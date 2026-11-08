#ifndef CPPBENCH_MEASURE_H
#define CPPBENCH_MEASURE_H
#include <stddef.h>

/* Layout is mirrored by #[repr(C)] Measurement in src/runner.rs. */
struct measurement {
    double metrics[15];
    int exit_code;
    int reason;       /* 0: complete, 1: timeout, 2: output limit, 3: interrupted */
    int launch_errno;
    int interrupt_signal;
};

/* Synchronous, single-threaded API. argv/envp must be NULL terminated.
 * All files and strings remain owned by the caller. Returns errno on failure.
 * Installs/restores temporary signal handlers and a SIGCHLD mask. */
int cppbench_measure_target(const char *executable, char *const argv[], char *const envp[],
                     const char *cwd, int input, int output, int error,
                     double timeout, double output_limit, double units,
                     const unsigned *cpus, size_t cpu_count,
                     struct measurement *result);
struct measurement_reply {
    int error;
    struct measurement measurement;
};
int cppbench_measure(int helper, int request, int reply, int input, int output,
                     int error, struct measurement *result);
#endif
