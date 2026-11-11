#define _GNU_SOURCE
#include "measure.h"
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile sig_atomic_t interrupted;
static void on_interrupt(int sig) { interrupted = sig; }

/* The Rust process only supervises this fresh C executable. Its own wait usage
 * is deliberately discarded: only the helper's target wait4 result is returned. */
int bench_measure(int helper, int request, int reply, int input, int output,
                     int error, struct measurement *result) {
    int fds[] = {request, reply, input, output, error};
    char numbers[5][32];
    char *argv[7] = {"bench-measure", NULL, NULL, NULL, NULL, NULL, NULL};
    for (size_t i = 0; i < 5; i++) {
        snprintf(numbers[i], sizeof numbers[i], "%d", fds[i]);
        argv[i + 1] = numbers[i];
    }
    /* Target environment is in the request, never injected into the loader/helper. */
    char *env[] = {"PATH=/usr/bin:/bin", "LANG=C", NULL};
    sigset_t blocked, oldmask;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGINT);
    sigaddset(&blocked, SIGTERM);
    if (sigprocmask(SIG_BLOCK, &blocked, &oldmask) < 0) return errno;
    struct sigaction action = {0}, oldint, oldterm;
    action.sa_handler = on_interrupt;
    sigemptyset(&action.sa_mask);
    interrupted = 0;
    if (sigaction(SIGINT, &action, &oldint) < 0) {
        int saved = errno; sigprocmask(SIG_SETMASK, &oldmask, NULL); return saved;
    }
    if (sigaction(SIGTERM, &action, &oldterm) < 0) {
        int saved = errno; sigaction(SIGINT, &oldint, NULL);
        sigprocmask(SIG_SETMASK, &oldmask, NULL); return saved;
    }
    pid_t parent = getpid();
    pid_t pid = fork();
    int saved = pid < 0 ? errno : 0;
    if (pid == 0) {
        if (prctl(PR_SET_PDEATHSIG, SIGKILL) < 0 || getppid() != parent || setsid() < 0) _exit(125);
        for (size_t i = 0; i < 5; i++) {
            int flags = fcntl(fds[i], F_GETFD);
            if (flags < 0 || fcntl(fds[i], F_SETFD, flags & ~FD_CLOEXEC) < 0) _exit(125);
        }
        /* INT/TERM stay blocked until the helper installs its target supervisor. */
        fexecve(helper, argv, env);
        struct measurement_reply failure = {0};
        failure.error = errno;
        (void)!pwrite(reply, &failure, sizeof failure, 0);
        _exit(125);
    }
    sigprocmask(SIG_SETMASK, &oldmask, NULL);
    if (pid > 0) {
        int status = 0;
        for (;;) {
            if (interrupted) kill(pid, interrupted);
            if (waitpid(pid, &status, 0) == pid) break;
            if (errno != EINTR) { saved = errno; break; }
        }
        if (!saved) {
            struct measurement_reply response;
            ssize_t n;
            do { n = pread(reply, &response, sizeof response, 0); } while (n < 0 && errno == EINTR);
            if (n != sizeof response) saved = n < 0 ? errno : EIO;
            else {
                *result = response.measurement;
                saved = response.error;
                if (!saved && interrupted) {
                    result->reason = 3;
                    result->interrupt_signal = interrupted;
                }
            }
        }
    }
    sigaction(SIGINT, &oldint, NULL);
    sigaction(SIGTERM, &oldterm, NULL);
    return saved;
}
