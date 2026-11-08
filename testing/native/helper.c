#define _GNU_SOURCE
#include "measure.h"
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

struct reader { unsigned char *p, *end; int error; };
static uint32_t number(struct reader *r) {
    if (r->end - r->p < 4) { r->error = EINVAL; return 0; }
    uint32_t v = (uint32_t)r->p[0] | (uint32_t)r->p[1] << 8 |
                 (uint32_t)r->p[2] << 16 | (uint32_t)r->p[3] << 24;
    r->p += 4;
    return v;
}
static double real(struct reader *r) {
    uint64_t bits = number(r);
    bits |= (uint64_t)number(r) << 32;
    double value;
    memcpy(&value, &bits, sizeof value);
    if (!isfinite(value) || value <= 0) r->error = EINVAL;
    return value;
}
static char *string(struct reader *r) {
    uint32_t n = number(r);
    if (!n || n > (size_t)(r->end - r->p) || r->p[n-1] != 0 || memchr(r->p, 0, n-1)) {
        r->error = EINVAL; return NULL;
    }
    char *value = (char *)r->p;
    r->p += n;
    return value;
}
static int run(int request, int input, int output, int error, struct measurement *result) {
    struct stat st;
    if (fstat(request, &st) < 0) return errno;
    if (st.st_size < 32 || st.st_size > 16 * 1024 * 1024) return E2BIG;
    unsigned char *data = malloc((size_t)st.st_size);
    if (!data) return ENOMEM;
    size_t offset = 0;
    while (offset < (size_t)st.st_size) {
        ssize_t n = pread(request, data + offset, (size_t)st.st_size - offset, (off_t)offset);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) { int saved = n < 0 ? errno : EIO; free(data); return saved; }
        offset += (size_t)n;
    }
    struct reader r = {data, data + st.st_size, 0};
    if (number(&r) != 1) { free(data); return EINVAL; }
    double timeout = real(&r), limit = real(&r), units = real(&r);
    uint32_t argc = number(&r), envc = number(&r), cpu_count = number(&r);
    if (!argc || argc > 1048576 || envc > 1048576 || cpu_count > 1024 || r.error) {
        free(data); return EINVAL;
    }
    unsigned cpus[1024];
    for (uint32_t i = 0; i < cpu_count; i++) cpus[i] = number(&r);
    char *cwd = string(&r);
    char **argv = calloc((size_t)argc + 1, sizeof *argv);
    char **envp = calloc((size_t)envc + 1, sizeof *envp);
    if (!argv || !envp) { free(argv); free(envp); free(data); return ENOMEM; }
    for (uint32_t i = 0; i < argc && !r.error; i++) argv[i] = string(&r);
    for (uint32_t i = 0; i < envc && !r.error; i++) envp[i] = string(&r);
    int saved = r.error || r.p != r.end ? EINVAL :
        cppbench_measure_target(argv[0], argv, envp, cwd, input, output, error,
                                timeout, limit, units, cpus, cpu_count, result);
    free(argv); free(envp); free(data);
    return saved;
}
int main(int argc, char **argv) {
    if (argc != 6) return 125;
    int fd[5];
    for (int i = 0; i < 5; i++) {
        char *end;
        long n = strtol(argv[i+1], &end, 10);
        if (*end || n < 0 || n > 2147483647) return 125;
        fd[i] = (int)n;
        if (fcntl(fd[i], F_SETFD, FD_CLOEXEC) < 0) return 125;
    }
    struct measurement_reply reply = {0};
    reply.error = run(fd[0], fd[2], fd[3], fd[4], &reply.measurement);
    return pwrite(fd[1], &reply, sizeof reply, 0) == sizeof reply ? 0 : 125;
}
