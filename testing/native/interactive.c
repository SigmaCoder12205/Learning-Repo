#define _GNU_SOURCE
#include "interactive.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pty.h>
#include <signal.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t interrupted;
static void on_interrupt(int signal) { interrupted = signal; }
static double now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + (double)t.tv_nsec / 1e9;
}
static void terminate(pid_t pid) { kill(-pid, SIGKILL); kill(pid, SIGKILL); }
static void child_error(int fd) {
    int saved = errno;
    (void)!write(fd, &saved, sizeof saved);
    _exit(127);
}

int bench_capture(char *const argv[], char *const envp[], const char *cwd,
                  int terminal, double timeout, double output_limit,
                  int (*callback)(void *, int, const unsigned char *, size_t),
                  void *context, struct capture_result *result) {
    memset(result, 0, sizeof *result);
    int master = -1, slave = -1, launch[2];
    struct termios term;
    if (tcgetattr(terminal, &term) < 0) return errno;
    /* Host line editing/echo remain intact; suppress a second echo from the PTY. */
    term.c_lflag &= ~(ECHO | ECHONL);
    term.c_lflag |= ICANON | ISIG;
    struct winsize size = {0};
    (void)ioctl(terminal, TIOCGWINSZ, &size);
    if (openpty(&master, &slave, NULL, &term, &size) < 0) return errno;
    if (pipe2(launch, O_CLOEXEC) < 0) { int e=errno; close(master); close(slave); return e; }
    int saved = 0;
    sigset_t blocked, oldmask;
    sigemptyset(&blocked); sigaddset(&blocked, SIGINT); sigaddset(&blocked, SIGTERM);
    if (sigprocmask(SIG_BLOCK, &blocked, &oldmask) < 0) { saved=errno; goto files; }
    struct sigaction action = {0}, oldint, oldterm;
    action.sa_handler = on_interrupt;
    sigemptyset(&action.sa_mask);
    interrupted = 0;
    if (sigaction(SIGINT, &action, &oldint) < 0) { saved=errno; goto mask; }
    if (sigaction(SIGTERM, &action, &oldterm) < 0) { saved=errno; sigaction(SIGINT,&oldint,NULL); goto mask; }
    double start = now();
    pid_t parent = getpid();
    pid_t pid = fork();
    if (pid < 0) { saved=errno; goto handlers; }
    if (pid == 0) {
        close(master); close(launch[0]);
        if (prctl(PR_SET_PDEATHSIG, SIGKILL) < 0 || getppid()!=parent || setsid()<0) child_error(launch[1]);
        if (ioctl(slave, TIOCSCTTY, 0)<0 || chdir(cwd)<0) child_error(launch[1]);
        for (int fd=0; fd<3; ++fd) if (dup2(slave,fd)<0) child_error(launch[1]);
        if (slave>2) close(slave);
        struct sigaction normal = {0}; normal.sa_handler=SIG_DFL; sigemptyset(&normal.sa_mask);
        sigaction(SIGINT,&normal,NULL); sigaction(SIGTERM,&normal,NULL);
        sigprocmask(SIG_SETMASK,&oldmask,NULL);
        execve(argv[0],argv,envp);
        child_error(launch[1]);
    }
    close(slave); slave=-1;
    close(launch[1]); launch[1]=-1;
    sigprocmask(SIG_SETMASK,&oldmask,NULL);
    if (fcntl(master,F_SETFL,fcntl(master,F_GETFL)|O_NONBLOCK)<0) { saved=errno; terminate(pid); }
    unsigned char pending[4096], output[4096];
    size_t used=0, sent=0, input_bytes=0;
    double output_bytes=0;
    int eof=0, status=0, done=0;
    while (!saved) {
        pid_t waited=waitpid(pid,&status,WNOHANG);
        if (waited==pid) { done=1; kill(-pid,SIGKILL); break; }
        if (waited<0 && errno!=EINTR) { saved=errno; break; }
        if (interrupted) { result->reason=3; result->interrupt_signal=interrupted; break; }
        if (now()-start >= timeout) { result->reason=1; break; }
        struct pollfd fds[2] = {{master,POLLIN | (sent<used ? POLLOUT : 0),0},
            {(!eof && sent==used) ? terminal : -1,POLLIN,0}};
        int ready=poll(fds,2,20);
        if (ready<0) { if (errno==EINTR) continue; saved=errno; break; }
        if (fds[0].revents & (POLLIN|POLLHUP)) {
            ssize_t n=read(master,output,sizeof output);
            if (n>0) {
                output_bytes+=(double)n;
                if (output_bytes>output_limit) { result->reason=2; break; }
                if (callback(context,0,output,(size_t)n)) { saved=EIO; break; }
            } else if (n<0 && errno!=EAGAIN && errno!=EIO && errno!=EINTR) { saved=errno; break; }
        }
        if (fds[0].revents & POLLOUT) {
            ssize_t n=write(master,pending+sent,used-sent);
            if (n>0) sent+=(size_t)n;
            else if (n<0 && errno!=EAGAIN && errno!=EINTR && errno!=EIO) { saved=errno; break; }
        }
        if (fds[1].revents & (POLLIN|POLLHUP)) {
            ssize_t n=read(terminal,pending,sizeof pending);
            if (n<0) { if (errno==EINTR) continue; saved=errno; break; }
            sent=0;
            if (!n) {
                eof=1; pending[0]=term.c_cc[VEOF]; pending[1]=term.c_cc[VEOF]; used=2;
            } else {
                used=(size_t)n; input_bytes+=used;
                if (input_bytes>1048576) { result->reason=4; break; }
                if (callback(context,1,pending,used)) { saved=EIO; break; }
            }
        }
    }
    if (!done) {
        terminate(pid);
        while (waitpid(pid,&status,0)<0) { if (errno!=EINTR) { if (!saved) saved=errno; break; } }
    }
    /* Drain the finite final output after terminating any remaining group members. */
    if (!saved && !result->reason) {
        for (;;) {
            ssize_t n=read(master,output,sizeof output);
            if (n<=0) { if (n<0 && errno==EINTR) continue; break; }
            output_bytes+=(double)n;
            if (output_bytes>output_limit) { result->reason=2; break; }
            if (callback(context,0,output,(size_t)n)) { saved=EIO; break; }
        }
    }
    if (interrupted) { result->reason=3; result->interrupt_signal=interrupted; }
    result->exit_code=WIFEXITED(status)?WEXITSTATUS(status):-WTERMSIG(status);
    ssize_t received;
    do { received=read(launch[0],&result->launch_errno,sizeof(int)); } while (received<0 && errno==EINTR);
    if (received<0 && !saved) saved=errno;
handlers:
    sigaction(SIGINT,&oldint,NULL); sigaction(SIGTERM,&oldterm,NULL);
mask:
    sigprocmask(SIG_SETMASK,&oldmask,NULL);
files:
    if (master>=0) close(master);
    if (slave>=0) close(slave);
    close(launch[0]); if (launch[1]>=0) close(launch[1]);
    return saved;
}
