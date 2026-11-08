#define _GNU_SOURCE
#include "measure.h"
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <sched.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t interrupted;
static void on_interrupt(int sig) { interrupted = sig; }
static double now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + (double)t.tv_nsec / 1e9;
}
static double size_of(int fd) {
    struct stat st;
    return fstat(fd, &st) == 0 ? (double)st.st_size : 0;
}
static double seconds(struct timeval t) {
    return (double)t.tv_sec + (double)t.tv_usec / 1e6;
}
static void kill_target(pid_t pid) {
    kill(-pid, SIGKILL);
    kill(pid, SIGKILL); /* Covers cancellation before the child creates its group. */
}
static void child_failure(int fd) {
    int saved = errno;
    /* One atomic pipe write; only async-signal-safe operations after fork. */
    (void)!write(fd, &saved, sizeof saved);
    _exit(127);
}

int cppbench_measure_target(const char *executable, char *const argv[], char *const envp[],
                     const char *cwd, int input, int output, int error,
                     double timeout, double output_limit, double units,
                     const unsigned *cpus, size_t cpu_count,
                     struct measurement *result) {
    memset(result, 0, sizeof *result);
    cpu_set_t affinity, allowed;
    CPU_ZERO(&affinity);
    if (cpu_count) {
        if (sched_getaffinity(0, sizeof allowed, &allowed) < 0) return errno;
        for (size_t i = 0; i < cpu_count; ++i) {
            if (cpus[i] >= CPU_SETSIZE || !CPU_ISSET(cpus[i], &allowed)) return EINVAL;
            CPU_SET(cpus[i], &affinity);
        }
    }
    int launch[2];
    if (pipe2(launch, O_CLOEXEC) < 0) return errno;
    sigset_t children, oldmask;
    sigemptyset(&children);
    sigaddset(&children, SIGCHLD);
    if (sigprocmask(SIG_BLOCK, &children, &oldmask) < 0) {
        int saved = errno; close(launch[0]); close(launch[1]); return saved;
    }
    struct sigaction action = {0}, oldint, oldterm;
    action.sa_handler = on_interrupt;
    sigemptyset(&action.sa_mask);
    interrupted = 0;
    if (sigaction(SIGINT, &action, &oldint) < 0) {
        int saved = errno;
        sigprocmask(SIG_SETMASK, &oldmask, NULL);
        close(launch[0]); close(launch[1]); return saved;
    }
    if (sigaction(SIGTERM, &action, &oldterm) < 0) {
        int saved = errno;
        sigaction(SIGINT, &oldint, NULL);
        sigprocmask(SIG_SETMASK, &oldmask, NULL);
        close(launch[0]); close(launch[1]); return saved;
    }
    /* The launcher blocks these across exec to preserve early cancellation. */
    sigdelset(&oldmask, SIGINT);
    sigdelset(&oldmask, SIGTERM);
    sigset_t runmask = oldmask;
    sigaddset(&runmask, SIGCHLD);
    sigprocmask(SIG_SETMASK, &runmask, NULL);
    double start = now();
    pid_t parent = getpid();
    pid_t pid = fork();
    int saved = pid < 0 ? errno : 0;
    if (pid == 0) {
        if (prctl(PR_SET_PDEATHSIG, SIGKILL) < 0 || getppid() != parent) _exit(127);
        close(launch[0]);
        sigaction(SIGINT, &oldint, NULL);
        sigaction(SIGTERM, &oldterm, NULL);
        struct sigaction default_pipe = {0};
        default_pipe.sa_handler = SIG_DFL;
        sigemptyset(&default_pipe.sa_mask);
        sigaction(SIGPIPE, &default_pipe, NULL);
        sigprocmask(SIG_SETMASK, &oldmask, NULL);
        if (cpu_count && sched_setaffinity(0, sizeof affinity, &affinity) < 0) child_failure(launch[1]);
        if (setsid() < 0 || chdir(cwd) < 0 ||
            dup2(input, STDIN_FILENO) < 0 || dup2(output, STDOUT_FILENO) < 0 ||
            dup2(error, STDERR_FILENO) < 0) child_failure(launch[1]);
        /* dup2(fd, fd) does not clear CLOEXEC when standard descriptors were closed. */
        if (fcntl(STDIN_FILENO, F_SETFD, 0) < 0 || fcntl(STDOUT_FILENO, F_SETFD, 0) < 0 ||
            fcntl(STDERR_FILENO, F_SETFD, 0) < 0) child_failure(launch[1]);
        execve(executable, argv, envp);
        child_failure(launch[1]);
    }
    close(launch[1]);
    if (pid < 0) {
        goto cleanup;
    }
    int status = 0;
    struct rusage usage = {0};
    for (;;) {
        pid_t done = wait4(pid, &status, WNOHANG, &usage);
        if (done == pid) break;
        if (done < 0 && errno != EINTR) { saved = errno; break; }
        if (interrupted) {
            result->reason = 3;
            result->interrupt_signal = interrupted;
        } else if (now() - start >= timeout) {
            result->reason = 1;
        } else if (size_of(output) + size_of(error) > output_limit) {
            result->reason = 2;
        }
        if (result->reason) {
            kill_target(pid);
            while (wait4(pid, &status, 0, &usage) < 0) {
                if (errno != EINTR) { saved = errno; break; }
            }
            break;
        }
        /* SIGCHLD wakes this immediately on exit; 5 ms is only the safety check cadence. */
        double remaining = timeout - (now() - start);
        if (remaining <= 0) continue;
        struct timespec interval = {0, (long)((remaining < .005 ? remaining : .005) * 1e9)};
        if (sigtimedwait(&children, NULL, &interval) < 0 && errno != EAGAIN && errno != EINTR) {
            saved = errno;
            kill_target(pid);
            while (wait4(pid, &status, 0, &usage) < 0 && errno == EINTR) {}
            break;
        }
    }
    double wall = now() - start;
    /* The leader is reaped; terminate remaining members of its process group. */
    kill(-pid, SIGKILL);
    if (saved) goto cleanup;
    ssize_t received;
    do { received = read(launch[0], &result->launch_errno, sizeof(int)); }
    while (received < 0 && errno == EINTR);
    if (received < 0) { saved = errno; goto cleanup; }
    result->exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : -WTERMSIG(status);
    double *m = result->metrics;
    m[0] = wall;
    m[1] = seconds(usage.ru_utime);
    m[2] = seconds(usage.ru_stime);
    m[3] = m[1] + m[2];
    m[4] = m[3] / wall * 100;
    m[5] = (double)usage.ru_maxrss;
    m[6] = (double)usage.ru_minflt;
    m[7] = (double)usage.ru_majflt;
    m[8] = (double)usage.ru_inblock;
    m[9] = (double)usage.ru_oublock;
    m[10] = (double)usage.ru_nvcsw;
    m[11] = (double)usage.ru_nivcsw;
    m[12] = size_of(output);
    m[13] = size_of(error);
    m[14] = units / wall;
    if (!result->reason && interrupted) {
        result->reason = 3;
        result->interrupt_signal = interrupted;
    }
    if (!result->reason && wall > timeout) result->reason = 1;
    if (!result->reason && m[12] + m[13] > output_limit) result->reason = 2;
cleanup:
    close(launch[0]);
    sigaction(SIGINT, &oldint, NULL);
    sigaction(SIGTERM, &oldterm, NULL);
    sigprocmask(SIG_SETMASK, &oldmask, NULL);
    return saved;
}
