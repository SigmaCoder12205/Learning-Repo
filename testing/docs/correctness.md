# Output correctness, repeatability, and focused suites

Version 0.13 adds inline output assertions, output repeatability checks, and suite case selection. They work with native executables and Python workloads. Existing file-based checks, input capture, metrics, histories, and report formats remain available.

## Assert an answer without an expected-output file

```bash
# Exact output, including the final newline (Bash quoting):
bench /bin/cat --input-line hello --expect-text $'hello\n'

# For a temperature converter that prints prompts before its answer:
bench ./temp_conversion \
  --input-line C --input-line 100 --input-line F \
  --expect-contains $'Answer: 212\n' \
  --runs 5 --warmup 1 --timeout 1 --output-limit 0.1
```

`--expect-text` compares exact stdout bytes, including whitespace and final newlines. It accepts up to 1 MiB of literal text, subject to the operating system's command-line size limits. It cannot be combined with `--expect-output FILE`; the existing file option remains suitable for binary or larger expected output. An empty exact expectation (`--expect-text=''`) requires empty stdout.

`--expect-contains TEXT` requires a literal, case-sensitive substring anywhere in stdout. Repeat it to require several substrings; all must occur, in any order. It can accompany either exact-check option. Accepts at most 16 nonempty literals of up to 4096 bytes each. It does not interpret regular expressions or numeric tolerances. For example, `Answer: 212` alone also matches `Answer: 2120`; include a newline when checking the complete answer line.

Checks scan full captured stdout, including matches across internal buffer boundaries, rather than just the 512-byte preview. Comparison and hashing occur after measured execution with bounded working memory. Reports include `output_check` on checked invocations: status, actual/expected byte counts, first differing byte (zero-based), and missing substring indices (one-based, in flag order). Text/HTML explain mismatches; the existing `output_mismatch` run status and exit code 1 apply. A failed warmup prevents measured runs. Timeout, output limit, and unexpected exit retain their original failure reasons and skip output checking.

Checks apply to warmups, measured runs, and diagnostic replays that preserve target stdout. Compiler invocations, Python startup calibration, tool probes, and profiler analysis commands are exempt. Heaptrack recording remains exempt because its launcher can mix messages into stdout; normal baseline output is still checked. Interactive input capture is an unmeasured setup trial; assertions apply to its subsequent file-based replays.

Exact checks require the correct expected answer. A passing substring assertion only establishes that the text occurred; it does not prove the whole output or algorithm correct.

## Detect changing output across measured runs

```bash
bench examples/workload.py --non-interactive \
  --check-output-stability --runs 5 --warmup 2 -- --size 1000
```

`--check-output-stability` requires `--runs` of at least 2. It fingerprints full stdout using SHA-256 for successful measured runs and compares each with the first. It excludes warmups, stderr, diagnostics, and failed output/exit checks. Each suite or generated input case has its own reference: different inputs may legitimately produce different answers.

Reports show `output_stability.status` (`stable`, `unstable`, or `insufficient_samples`), checked-run count, distinct-output count, reference run, and differing run numbers. Two identical successful samples establish stability only for those observations. Constantly wrong output can still be stable. Programs intentionally printing timestamps, random values, or process IDs will often fail this check.

An unstable or insufficient result makes `measurement_passed` false and exit code 1, prevents saving it as a successful baseline, and skips subsequent diagnostics. Individual successful timing/resource samples remain in the report and summaries; their statuses are not rewritten as execution failures. Text, JSON, and HTML expose the repeatability result. CSV remains a metric-row export; check the exit code and use JSON for the aggregate stability result. Partial checkpoint recovery recomputes stability from captured sample fingerprints.

Hashing is outside measured time but adds work between invocations and may affect caches. It does not measure stderr stability, internal state, numerical accuracy, or all possible nondeterminism.

## List or run selected suite cases

```bash
bench --suite examples/correctness-suite.json --list-cases
bench --suite examples/correctness-suite.json --case native-echo
bench --suite examples/correctness-suite.json --case python-hash --case native-echo
bench --suite examples/correctness-suite.json --list-cases --format json
```

`--list-cases` validates the suite and prints case names without running targets or creating benchmark history/checkpoints. Text names are quoted/escaped; JSON provides a `cases` array. It supports text/JSON and optional `--output FILE` and `--case` filters. Executables, interpreter paths, and other required files must exist because listing performs the normal preflight.

Repeat `--case NAME` for exact, case-sensitive names. Selected cases run once each in their original suite order; repeating a name does not duplicate it. Unknown names are errors before execution. All suite entries are validated, including unselected entries, so selection cannot hide malformed configuration. Both options require `--suite` and do not filter generated input matrices or recovered reports.

Use the same selection when saving and comparing a baseline. Existing comparison rules reject missing suite cases; filtering a run does not silently discard extra cases from a full-suite baseline.

Suite entries/defaults support the new checks:

```json
{
  "version": 1,
  "defaults": {"runs": 3, "warmup": 1, "check_output_stability": true},
  "benchmarks": [
    {
      "name": "echo",
      "executable": "/bin/cat",
      "input_lines": ["hello"],
      "expect_text": "hello\n",
      "expect_contains": ["hello"]
    }
  ]
}
```

For programs like a temperature converter, give each named conversion its own answers and expected text. Generated matrices still share their assertions across cases, so a single fixed expected answer usually does not suit a matrix with different conversions.

## Saved results and compatibility

History, checkpoint recovery, and HTML retain output checks and repeatability information. Exact expectations and substring literals are included in JSON reports; use appropriate values for reports you intend to share.

Provenance now records expectation hashes. Equivalent exact text and expected files have the same hash; substring hashes are sorted and deduplicated for compatibility. Changed expectations or repeatability settings cause comparison incompatibility unless `--allow-mismatch` is explicitly supplied. Older baselines with no checks remain compatible with unchecked runs. Older baselines using output checks lack the new hashes; regenerate them or inspect the difference before overriding. Failed measurements cannot be made valid with that override.

The timed execution backend and report schema version 3 are unchanged. Type inference also received targeted fixes: `print(...)` no longer implies `int(...)`, prompt words use boundaries (`message` is not `age`), bare yes/no prompts suggest yes/no, and scanf discovery reads the format belonging to the call rather than an earlier printed string. Input hints remain heuristics; see [input workflows](inputs.md).
