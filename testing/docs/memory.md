# Understanding memory measurements

cppbench reports **peak resident set size (RSS)** in KiB internally and readable KiB/MiB in text. It comes from Linux `wait4` / `ru_maxrss`. It is not a count of C variables, heap allocations, or executable sections.

## Why `size` and RSS differ

GNU `size` describes sections of the executable file. For example:

```text
text   data  bss  dec
2815   672   16   3503
```

This means 3,503 bytes in the reported section categories (about 3.42 KiB), not 3,503 bytes of total process RAM. Local automatic variables use the stack; the executable's BSS is not the stack. Dynamic runtime mappings, the loader, stack pages, and libc's internal buffers are not all represented by this section total. Resident pages are accounted at page granularity, not at the byte granularity of C scalar types.

A dynamic program may therefore have a few KiB of executable sections and use MiB of resident memory. RSS counts resident shared-library pages mapped into the process; it is not the process's uniquely owned physical memory. It also does not include every byte of a mapped library's on-disk file, only resident pages accounted by the kernel.

A `.cpp` file built with `g++` can link the C++ runtime even when its source uses C-style stdio. Inspect a trusted local binary with `ldd ./program` or inspect its dynamic dependencies with `readelf -d ./program`. If the source is valid C, compiling it as C (`cc -x c -O2 source.cpp -o program-c`) can change the runtime dependencies. This does not imply its process RSS should equal its section size.

The calculator case used to investigate this issue had 3,503 bytes of sections but linked libstdc++, libm, libgcc_s, and libc. A separate minimal C launcher also measured roughly 4 MiB RSS. Building the same source as C in a temporary directory measured roughly 1.7–1.8 MiB on that machine. These are observations for those builds, not portable memory requirements.

## The launcher bug fixed in 0.9

Earlier versions forked the target directly from the Rust report process. Linux preserves resource high-water accounting across `exec`. The child could inherit the Rust process's resident mappings before replacing them, causing its reported peak to reflect cppbench's memory instead of its own workload. The effect grows when the report process holds large allocations; subtracting a constant baseline cannot correct this maximum reliably.

Version 0.9 starts a separate, small C executable in a fresh address space, then forks and measures the target from there. The Rust process's usage and the first launcher's inherited peak are discarded. Only the C helper's `wait4` measurement of its target child is reported. The helper is embedded in the installed binary and executed using a sealed `memfd`; no executable needs to be installed beside cppbench and `/tmp` does not need to allow execution. Systems that prohibit executable memory files cannot use this backend; it does not silently fall back to the old contaminated path.

Timing starts in the helper immediately before the target fork and stops when the target is reaped. Creating/loading the helper and transporting its result are excluded from the reported target wall/CPU time, but do increase the total duration of the cppbench command. Launch overhead for the target, runtime startup, and shutdown remain included. The target's environment is passed privately in the request and does not influence the helper's loader.

## Exact scope and limitations

- The reported value is the kernel's peak for the **target child lifetime**, including its small C pre-exec setup. It is not an exact post-exec-only measurement; a very small static target can still be limited by this launch footprint.
- The target's libraries, stack, heap, and other resident mappings are included. cppbench does not claim byte-exact application-owned or heap-only memory.
- Usage can include children the target waited for. Linux peak RSS is a maximum, not a simultaneous sum across an entire process tree. Background descendants that are not waited for are not reliably represented.
- Values reflect kernel RSS accounting and its granularity. They are not PSS, USS, cgroup memory, virtual address space, or allocation volume.
- Peak RSS remains a high-water value after memory is freed. cppbench uses the kernel result rather than periodic RSS polling, so short-lived allocations are not intentionally missed by a sampling interval.

Use `--heaptrack` when you need allocation counts and heap-oriented diagnostics. Those are separate instrumented replays and remain different from baseline process RSS.

Reports now record `measurement_backend: "isolated-c-wait4-v1"` and a `memory_accounting` description. Comparing these with older baselines is marked incompatible unless `--allow-mismatch` is explicitly supplied; regenerating baselines is recommended because launch-related timing, faults, and RSS can all change.

## Verification

Regression tests hold an additional 64 MiB of touched memory in the Rust parent and verify that target peak RSS stays stable. Another test allocates, touches, and frees 64 MiB in a short-lived target and compares cppbench's peak with the target's own `getrusage` value. Existing tests cover signals, timeouts, descendant cleanup, source compilation, environment/arguments, CPU affinity, diagnostics, and file replay.

References: [GNU size documentation](https://sourceware.org/binutils/docs/binutils/size.html), [Linux getrusage](https://man7.org/linux/man-pages/man2/getrusage.2.html), and [Linux exec resource high-water handling](https://github.com/torvalds/linux/blob/master/fs/exec.c).
