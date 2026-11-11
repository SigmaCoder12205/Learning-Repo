# Benchmarking Python

`bench` measures Python workloads with the same Rust/C process runner used for native executables. Install Python and any workload dependencies yourself; bench does not create environments or install packages.

```bash
bench workload.py
bench workload.py --runs 30 --warmup 3 --intermediate
bench workload.py --metrics timing,memory --format json --output results.json
bench workload.py --input input.txt --expect-output expected.txt
bench workload.py -- --size 100000 --mode batch
```

A positional path ending in `.py` automatically selects Python. Use `--python PATH` for extensionless scripts. A script needs to be readable, but does not need a shebang or executable permission. Use `--executable PATH` to explicitly launch a file through its own executable/shebang behavior instead.

## Interpreter and arguments

The default is `python3` from the target's PATH, including any `--env PATH=...` override. An activated virtual environment therefore works automatically. You can select an interpreter explicitly:

```bash
bench workload.py --interpreter .venv/bin/python
bench workload.py --interpreter python3.13 --python-arg=-O --python-arg=-B
bench --python scripts/workload --interpreter /usr/bin/python3 -- --size 10000
```

`--python-arg=OPTION` is repeatable and goes before the script path. Use it for interpreter options such as `-O`, `-B`, or `-Xdev`; execution-mode switches such as `-c` and `-m` change Python's command semantics and are not script benchmarking options. Everything after `--` goes to the script. Arguments are passed directly, without shell expansion or evaluation.

Relative script and interpreter paths resolve from the directory where you invoke bench. `--cwd` changes the workload's working directory. Interpreter symlinks are preserved so Python finds the correct virtual environment. See Python's [command-line documentation](https://docs.python.org/3/using/cmdline.html) and [virtual environment documentation](https://docs.python.org/3/library/venv.html).

## What the results measure

Each warmup and measured run starts a new interpreter process. Time includes interpreter startup, imports, script execution, and shutdown. Peak RSS includes Python, loaded libraries, and application memory; it is not the size of the `.py` file or just its Python objects. CPU usage and all other baseline metrics retain their usual process-level definitions. See [memory accounting](memory.md).

Warmups can prime filesystem caches but do not preserve imported modules, allocated objects, or JIT state between processes. For a very small operation, repeat useful work inside the script and use `--units N` to describe the work per invocation. This remains a full-process benchmark, not an isolated Python function microbenchmark. Imported code, installed packages, input data, and interpreter settings all affect results.

JSON records `python.script`, `python.interpreter`, `python.interpreter_args`, and the measurement scope. Provenance records SHA-256 hashes of the main script and interpreter executable. It does not hash every imported module or dependency. Comparisons warn when the main script changes and reject interpreter changes unless `--allow-mismatch` is supplied. Record your dependency versions alongside saved baselines.

Timeouts, output limits, expected output/exit status, input replay, checkpoints, reports, profiles, and diagnostic replays apply as for native programs. Python exceptions produce failed runs with captured stderr. Each diagnostic replay starts another interpreter. Native tools such as perf, Valgrind, and heaptrack observe interpreter/native activity; they do not automatically provide Python line-level timings or per-object allocation attribution. GPU collection retains the existing Intel/Linux tooling constraints.

## Mixed suites

```json
{
  "version": 1,
  "defaults": {"runs": 10, "warmup": 1},
  "benchmarks": [
    {"name": "native", "executable": "build/program", "args": ["10000"]},
    {"name": "python", "python": "workload.py", "interpreter": ".venv/bin/python",
     "python_args": ["-O"], "args": ["--size", "10000"]}
  ]
}
```

Scripts and relative interpreter paths containing a directory component resolve from the suite file's directory. Bare interpreter names are searched in the case's PATH. Use `./python` for an interpreter in the suite directory. Suites validate all script and interpreter paths before executing any case. Run the included standard-library example with `bench --suite examples/python-suite.json`.

## Application telemetry

Python can emit the same domain-specific JSON as C/C++ instrumentation. Only write it when bench requests a telemetry replay:

```python
import json
import os

destination = os.environ.get("BENCH_TELEMETRY_FILE")
if destination:
    with open(destination, "x", encoding="utf-8") as output:
        json.dump({
            "version": 1,
            "metrics": {
                "backend.request_ms": {"unit": "ms", "samples": measured_request_ms},
                "backend.requests": {"unit": "count", "value": request_count},
                "backend.errors": {"unit": "count", "value": error_count}
            }
        }, output)
```

Supply actual application measurements for those variables. `bench workload.py --backend --api` collects that replay separately from baseline statistics. Supported keys, units, limits, and subflags are documented in [domain profiles](profiles.md).
