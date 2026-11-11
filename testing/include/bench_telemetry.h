#ifndef BENCH_TELEMETRY_H
#define BENCH_TELEMETRY_H
/* Optional C11/C++ telemetry writer. One owner per process/replay; not thread-safe.
 * Buffer measurements in memory, join worker threads, then write once at shutdown.
 * No file is opened unless bench sets BENCH_TELEMETRY_FILE for a replay.
 * Durations are milliseconds. Keys/units must match bench --list-profiles.
 */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <stddef.h>

typedef struct { FILE *file; int first; int failed; } bench_telemetry;

static inline int bench_telemetry_enabled(void) {
    const char *path = getenv("BENCH_TELEMETRY_FILE");
    if (!path || !*path) path = getenv("CPPBENCH_TELEMETRY_FILE");
    return path && *path;
}
static inline bench_telemetry bench_telemetry_open(void) {
    bench_telemetry t = {NULL, 1, 0};
    const char *path = getenv("BENCH_TELEMETRY_FILE");
    if (!path || !*path) path = getenv("CPPBENCH_TELEMETRY_FILE");
    if (!path || !*path) return t;
    t.file = fopen(path, "wx");
    if (!t.file) { t.failed = 1; return t; }
    if (fputs("{\"version\":1,\"metrics\":{", t.file) < 0) t.failed = 1;
    return t;
}
static inline void bench_telemetry_string(FILE *f, const char *text) {
    fputc('"', f);
    for (const unsigned char *p = (const unsigned char *)text; *p; ++p) {
        if (*p == '"' || *p == '\\') { fputc('\\', f); fputc(*p, f); }
        else if (*p < 32) fprintf(f, "\\u%04x", (unsigned)*p);
        else fputc(*p, f);
    }
    fputc('"', f);
}
static inline void bench_telemetry_key(bench_telemetry *t, const char *key, const char *unit) {
    if (!t->first) fputc(',', t->file);
    t->first = 0;
    bench_telemetry_string(t->file, key);
    fputs(":{\"unit\":", t->file);
    bench_telemetry_string(t->file, unit);
}
static inline void bench_telemetry_value(bench_telemetry *t, const char *key, const char *unit, double value) {
    if (!t->file || t->failed) return;
    if (!isfinite(value) || value < 0) { t->failed = 1; return; }
    bench_telemetry_key(t, key, unit);
    fprintf(t->file, ",\"value\":%.17g}", value);
}
static inline void bench_telemetry_samples(bench_telemetry *t, const char *key, const char *unit, const double *values, size_t count) {
    if (!t->file || t->failed) return;
    if (!count || count > 100000 || !values) { t->failed = 1; return; }
    for (size_t i = 0; i < count; ++i) {
        if (!isfinite(values[i]) || values[i] < 0) { t->failed = 1; return; }
    }
    bench_telemetry_key(t, key, unit);
    fputs(",\"samples\":[", t->file);
    for (size_t i = 0; i < count; ++i) fprintf(t->file, "%s%.17g", i ? "," : "", values[i]);
    fputs("]}", t->file);
}
static inline int bench_telemetry_close(bench_telemetry *t) {
    if (!t->file) return t->failed ? -1 : 0;
    /* Invalid data leaves malformed JSON so bench cannot mistake it for a clean report. */
    if (t->failed) fputs(",INVALID", t->file);
    fputs("}}\n", t->file);
    if (ferror(t->file)) t->failed = 1;
    if (fclose(t->file)) t->failed = 1;
    t->file = NULL;
    return t->failed ? -1 : 0;
}
#endif
