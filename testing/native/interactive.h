#ifndef BENCH_INTERACTIVE_H
#define BENCH_INTERACTIVE_H
#include <stddef.h>
struct capture_result {
    int exit_code;
    int reason; /* 0 complete, 1 timeout, 2 output cap, 3 interrupt, 4 input cap */
    int launch_errno;
    int interrupt_signal;
};
/* Unmeasured, single-threaded trial. callback: 0 output bytes, 1 input bytes.
 * Return nonzero from callback to abort with an I/O error. */
int bench_capture(char *const argv[], char *const envp[], const char *cwd,
                  int terminal, double timeout, double output_limit,
                  int (*callback)(void *, int, const unsigned char *, size_t),
                  void *context, struct capture_result *result);
#endif
