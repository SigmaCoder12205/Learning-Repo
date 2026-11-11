#ifndef CPPBENCH_TELEMETRY_H
#define CPPBENCH_TELEMETRY_H
/* Compatibility aliases for instrumentation written before the bench rename. */
#include "bench_telemetry.h"
typedef bench_telemetry cppbench_telemetry;
#define cppbench_telemetry_enabled bench_telemetry_enabled
#define cppbench_telemetry_open bench_telemetry_open
#define cppbench_telemetry_string bench_telemetry_string
#define cppbench_telemetry_key bench_telemetry_key
#define cppbench_telemetry_value bench_telemetry_value
#define cppbench_telemetry_samples bench_telemetry_samples
#define cppbench_telemetry_close bench_telemetry_close
#endif
