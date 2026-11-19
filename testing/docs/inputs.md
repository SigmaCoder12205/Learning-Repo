# Input discovery, answer capture, and test cases

Version 0.12 adds input handling shared by native executables and Python workloads. Existing `--input FILE` replay remains supported.

## Answer once in your terminal

```bash
bench ./calculator
bench script.py -- --size 10
```

When stdin and stderr are terminals and no input was supplied, bench first runs an **extra, unmeasured trial**. It shows available source hints, displays the program's prompts, and lets you type answers using normal terminal line editing. Prompt wording can provide additional type or choice hints. Finish the whole session, including a quit command for programs that loop. Ctrl-D sends EOF; Ctrl-C cancels.

Bench then replays the captured bytes from the beginning for every warmup, measured run, and requested diagnostic. Human response time is excluded. This still measures startup, parsing, computation, output, and shutdown for each complete process invocation.

Use `--interactive` to request capture explicitly (stdin must be a terminal). `--interactive-timeout 180` changes the human-inclusive trial deadline from 120 seconds. `--output-limit` also bounds trial output, and recorded answers are limited to 1 MiB. A failed, interrupted, or timed-out trial produces a report and prevents measurements from starting. Reports stay on stdout; prompts and capture messages use stderr, including with `--format json`.

The trial runs the actual program, so its side effects happen once more. `--non-interactive` skips it. Outside a terminal, or with that flag, absent input still means `/dev/null` and immediate EOF. Suites never automatically prompt. Piping data into bench does not supply target stdin; use an explicit input option.

Capture is for ordinary line-oriented prompts. The trial uses a pseudo-terminal, but **measured runs use file-based stdin and captured output**, so programs that require raw keystrokes, curses, passwords without echo, or particular `isatty()` behavior are not supported by this replay workflow. Finish an EOF-sensitive program correctly: a target that ignores EOF can still loop until the output or time limit.

## Let bench infer expected types

```bash
bench script.py --discover-inputs
bench ./calculator --discover-inputs
bench ./build/program --input-source src/main.cpp --discover-inputs
bench script.py --discover-inputs --format json -- --size 10
```

Discovery reads source without executing or importing it. Bench inspects the Python script, sources passed with `--source`, or a sibling `.cpp`, `.cc`, `.cxx`, or `.c` file for an executable. `--input-source FILE` overrides discovery. Each source must be a regular UTF-8 file no larger than 256 KiB.

Recognized patterns include Python `input()`, common `int`/`float` conversions, simple argparse declarations and literal choices, C++ `cin`, `getline`, C `scanf` formats, direct `argv[N]` access, and possible character choices from switch cases. During capture, recognizable words and menus in live prompts provide further hints. Results show the likely type, example values, and their evidence.

These are bounded text heuristics, not a full language parser or an exhaustive specification. Dynamic argument parsers, imported code, multiline expressions, branches, loops, several values on one line, and validation rules can change what is actually accepted. A stripped executable alone does not reveal its input schema. Unknown inputs remain unknown; use explicit values when suggestions do not match the program. Numeric examples are not claims about valid ranges.

## Supply answers without a file

```bash
# Operation, two numbers, then quit:
bench ./calculator --input-line + --input-line 2 --input-line 3 --input-line q

# Exact literal bytes, with Bash newline quoting:
bench script.py --input-text $'Alice\n42\n'

# Target flags follow --; they are separate from stdin:
bench script.py --input-line hello -- --mode fast --size 100
```

`--input-line` appends a newline per answer; `--input-text` preserves its text exactly. Inline input is limited to 1 MiB. Choose one of file, text, lines, or explicit interactive capture. File replay can contain binary data and is not subject to the inline size limit. A temporary private replay file holds inline/recorded bytes until all invocations finish, then is removed.

## Compare different inputs

```bash
# Answer once, then test bounded inferred examples:
bench script.py --explore-inputs --max-cases 12

# Explicit answer choices, retaining a complete quit sequence:
bench ./calculator --input-line + --input-line 1 --input-line 2 --input-line q \
  --vary-input '2=["0","1","-1","100"]'

# Argument 2 means the value 10, not the flag --size:
bench script.py --non-interactive --vary-arg '2=["10","100","1000"]' -- --size 10

# Presets are available for explicit variations:
bench script.py --input-line 7 --vary-input '1=@integer'
```

`--explore-inputs` keeps your original case first, then changes one recognized field at a time using inferred examples. It needs usable example answers or existing target arguments. It skips changing standalone `q`, `quit`, and `exit` answers. Exploration stops at `--max-cases` (default 16, allowed 1–256); this is a sample, not all combinations or branch coverage. Later fields may not be reached before the cap. A changed answer can alter which prompts follow, so review failures and use explicit cases for different branches.

`--vary-input` addresses **1-based newline-delimited answer lines**, not semantic questions or whitespace-separated tokens. `--vary-arg` addresses **1-based arguments after `--`**. Repeating either flag creates a Cartesian product. Values are JSON strings; presets are `@integer`, `@number`, `@text`, `@character`, and `@boolean`. Explicit matrices exceeding the cap are rejected; they are not silently truncated. Exploration and explicit variations cannot be combined.

All generated plans are validated before measured cases start. Matrices require UTF-8 stdin of at most 1 MiB per case and at most 16 MiB combined; at most 64 positions may be varied, with 1–256 values per position. Discovery and any interactive trial precede case generation. Source compilation happens once for an entire matrix.

Each case has its own warmups, timings, memory measurements, exit status, correctness checks, and optional diagnostics. Ordinary case failures do not prevent the remaining cases from running; interruption stops the matrix. Text shows a compact comparison followed by case details, answers/arguments, and the first measured stdout preview. JSON and HTML preserve case metadata and per-run output previews (512 bytes). CSV retains the existing case-labelled metric rows. Failed warmups prevent measurements for that case.

The same `--expect-exit` and optional `--expect-output`, `--expect-text`, or `--expect-contains` apply to every generated case. `--check-output-stability` evaluates each case independently. See [correctness checks](correctness.md). For different expected outputs or distinct input sequences, define named suite entries instead. History, pins, saved comparisons, HTML, and checkpoint recovery support input matrices; comparison still checks each case's command and input hash. Use the same case ordering and values when comparing performance over time.

Reports now include bounded stdout previews on successful runs too. Matrix reports also contain answer/argument previews; local history and exported reports retain that information. Do not use sensitive answers in reports you intend to share.

## Inline answers in a saved suite

```json
{
  "version": 1,
  "defaults": {"runs": 5, "warmup": 1},
  "benchmarks": [
    {"name": "small", "python": "work.py", "input_lines": ["10"]},
    {"name": "large", "python": "work.py", "input_text": "1000\n"}
  ]
}
```

Suite entries accept `input_lines`, `input_text`, and optional `input_source`, as well as existing `input` files. Conflicting input methods are rejected during suite preflight. Interactive capture and generated variations are standalone CLI features; suite entries describe their inputs explicitly.
