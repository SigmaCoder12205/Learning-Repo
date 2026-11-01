# Processes in C++ on Linux and other POSIX systems

*A practical guide to process lifetime, PIDs, `fork()`, `exec()`, `wait()`, and related tools. The examples use C++17 and a POSIX system such as Linux. Windows has a different process API; these examples do not compile there unchanged.*

## Contents

1. [Programs, processes, and threads](#1-programs-processes-and-threads)
2. [What belongs to a process?](#2-what-belongs-to-a-process)
3. [PIDs and parent processes](#3-pids-and-parent-processes)
4. [Creating a child with `fork()`](#4-creating-a-child-with-fork)
5. [Finishing and collecting a child with `waitpid()`](#5-finishing-and-collecting-a-child-with-waitpid)
6. [Running another program with `exec()`](#6-running-another-program-with-exec)
7. [A complete `fork()` + `exec()` + `waitpid()` example](#7-a-complete-fork--exec--waitpid-example)
8. [File descriptors, redirection, and pipes](#8-file-descriptors-redirection-and-pipes)
9. [Signals, process groups, and job control](#9-signals-process-groups-and-job-control)
10. [Threads, `posix_spawn()`, and `std::system()`](#10-threads-posix_spawn-and-stdsystem)
11. [Observing processes and common mistakes](#11-observing-processes-and-common-mistakes)
12. [Practice and references](#12-practice-and-references)

## 1. Programs, processes, and threads

A **program** is an executable file: instructions and data stored on disk. A **process** is one running instance of a program, with its own identity and resources. Starting the same executable three times normally creates three different processes. When you type `./main`, your shell arranges for a process to execute that file. The shell is itself a process.

```text
executable file: ./main
                   │
             run it three times
                   │
                   ├── process PID 4210
                   ├── process PID 4211
                   └── process PID 4212
```

The kernel creates or replaces a process's address space, maps executable code and required libraries, prepares its stack and arguments, and schedules its threads to run. The code does not have to be copied into one dedicated block of physical RAM: virtual memory maps pages to physical memory or other backing as needed. The first C++ statement in `main()` runs after runtime initialization has done some work.

A **thread** is an execution path *inside* a process. Threads in one process share its address space and many resources; separate processes normally have isolated address spaces. The scheduler can interleave runnable threads, so the output order of two processes is generally unpredictable. A process can have more than one thread, but a single-threaded program starts with one.

### What the kernel does while a process runs

An executable does not hold the CPU continuously. The kernel's scheduler gives CPU time to runnable threads. When a thread waits for input, a timer, or a child, it can **block** so other work runs. Later it becomes runnable again. A **context switch** saves one thread's CPU state and restores another's. These are simplified states: Linux exposes more detailed states through tools such as `ps`.

```text
created → runnable ⇄ running → blocked (waiting for an event)
             ↑                    │
             └──── event ready ───┘
running → exited → status collected by parent
```

A C++ call such as `fork()` or `waitpid()` enters the operating system through a POSIX library interface backed by kernel services. `fork()` may make another process runnable; `waitpid()` may block the caller. An `exec` call replaces the program being run in the *same* process. The exact order in which parent and child next run is chosen by the scheduler.

For command-line arguments, see [Command-line arguments](CommandLineArguments.md). For the memory layout behind stacks and heaps, see [Memory](Memory.md).

## 2. What belongs to a process?

It helps to separate **memory**, **execution state**, and **kernel-managed resources**:

| Part | What it means |
| --- | --- |
| Virtual address space | The process's view of addresses, including mapped code, globals, heap, stacks, and shared-library mappings. It is not necessarily all resident in RAM. |
| Threads and CPU state | Each thread has registers, a program counter, and a stack. The program counter identifies the next machine instruction, not a C++ source line. |
| File descriptors | Small integers referring to open files, terminals, sockets, or pipes. `0`, `1`, and `2` conventionally mean standard input, output, and error. |
| Environment and arguments | Text values supplied when a program starts. The environment can be changed for future child programs. |
| Identity and permissions | A PID, parent PID, user/group credentials, and other security attributes. |
| Working directory and signal state | A base directory for relative paths; rules for handling or blocking signals. |

The kernel enforces normal process isolation: a pointer in one process does not let it directly read another process's private memory. Processes can deliberately exchange data through pipes, sockets, shared memory, files, and other interprocess communication (IPC) mechanisms.

## 3. PIDs and parent processes

A **process ID** (PID) identifies a process within a PID namespace. `getpid()` returns your process's PID; `getppid()` returns its current parent's PID. A PID is temporary: after a process has been reaped, the number can be reused. Do not treat a saved PID as a permanent identity. PID namespaces (often used by containers) can also give one process different PID numbers from different views.

Save this as `show_pid.cpp`:

```cpp
#include <iostream>
#include <sys/types.h>
#include <unistd.h>

int main() {
    std::cout << "PID: " << getpid() << '\n';
    std::cout << "parent PID: " << getppid() << '\n';
}
```

```sh
g++ -std=c++17 -Wall -Wextra show_pid.cpp -o show_pid
./show_pid
```

The numbers change between runs. If a shell starts the executable directly, the parent is normally that shell. Wrappers, IDEs, and terminals can create a different process tree. A parent can have many children; a process has one current parent. If its original parent exits first, the system reparents it, so `getppid()` can change.

`pid_t` is the POSIX type used for PIDs. It is supplied through POSIX headers, not by standard C++ alone.

## 4. Creating a child with `fork()`

`fork()` creates a new child process. Both processes continue at the instruction *after* the call, but the result differs:

| Return from `fork()` | Which process sees it? | Meaning |
| --- | --- | --- |
| `-1` | Parent only | Creation failed; no child was created. Check `errno`. |
| `0` | Child only | This is the child branch. Use `getpid()` to get its actual PID. |
| Positive PID | Parent only | This is the parent branch; the value is the child's PID. |

```text
before fork(): one process
          │
       fork()
       /    \
parent:     child:
pid > 0     pid == 0
```

At the fork point, parent and child have separate virtual address spaces with initially equal contents. If the child changes an ordinary local or global variable, the parent's copy does not change. On Linux, **copy-on-write** usually lets them share physical memory pages until one writes to a page; `fork()` need not immediately copy all bytes. This does not mean that every resource is independent: the child receives file descriptors that refer to the same underlying *open file descriptions*, so the processes can share a file offset and file status flags. Their output can interleave. The child also inherits an environment and working directory, while receiving its own PID and pending-signal state. [Linux `fork(2)`](https://man7.org/linux/man-pages/man2/fork.2.html) lists the precise inheritance rules.

This complete example shows independent memory and the two return values. It assumes a single-threaded process:

```cpp
// fork_demo.cpp
#include <cerrno>
#include <cstdio>
#include <iostream>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

int main() {
    int value = 10;
    std::cout << "Before fork, PID " << getpid() << '\n' << std::flush;

    pid_t child = fork();
    if (child == -1) {
        std::perror("fork");
        return 1;
    }

    if (child == 0) {
        value = 20;
        std::cout << "Child " << getpid() << ": value = " << value << '\n'
                  << std::flush;
        _exit(42); // End this child without running copied C++ exit handlers.
    }

    int status = 0;
    pid_t result;
    do {
        result = waitpid(child, &status, 0);
    } while (result == -1 && errno == EINTR);

    if (result == -1) {
        std::perror("waitpid");
        return 1;
    }

    std::cout << "Parent " << getpid() << ": value = " << value << '\n';
    if (WIFEXITED(status)) {
        std::cout << "Child exit code: " << WEXITSTATUS(status) << '\n';
    }
}
```

```sh
g++ -std=c++17 -Wall -Wextra fork_demo.cpp -o fork_demo
./fork_demo
```

The child prints `20`; the parent prints `10`. The PID values vary. `std::flush` before `fork()` matters: buffered output copied into both processes might otherwise appear twice when both later flush it. The child flushes its own line before `_exit()`, which does not flush C++ streams. For a larger C++ program, avoid using complex C++ library state between `fork()` and `exec()`; section 10 explains why.

## 5. Finishing and collecting a child with `waitpid()`

When a child terminates, the kernel retains a small record of its termination status until its parent collects it. During that interval the child is a **zombie**: it is no longer executing and does not retain its normal address space, but its PID and status remain. A zombie is not automatically permanent. Reaping it with a wait call removes the record; if a parent exits, an adopter process normally takes responsibility. A long-running parent that never reaps children can accumulate zombies.

`wait(&status)` waits for **any one** child. `waitpid(child_pid, &status, 0)` waits for a **particular** child, which is preferable when you know its PID. Both return the child's PID on success, or `-1` on error. `waitpid()` may fail with `EINTR` if interrupted by a signal; retry as the example does. `ECHILD` means there is no matching unwaited-for child. A wait call does not work on arbitrary unrelated processes. [Linux `wait(2)`](https://man7.org/linux/man-pages/man2/waitpid.2.html) documents these cases.

The `status` integer is an encoded result, **not** simply the child's exit code. Use the macros from `<sys/wait.h>`:

| Check | Meaning |
| --- | --- |
| `WIFEXITED(status)` | Child ended normally (`return` from `main`, `exit`, or `_exit`). |
| `WEXITSTATUS(status)` | Its exit code; use only if `WIFEXITED` is true. POSIX exposes the low eight bits, so use `0` through `255`. |
| `WIFSIGNALED(status)` | Child was terminated by a signal. |
| `WTERMSIG(status)` | Signal number; use only if `WIFSIGNALED` is true. |

Code `0` conventionally means success; nonzero means some kind of failure. A signal termination has no normal exit code. If you create several children, keep their PIDs and wait for each one, or loop over `wait()` until no children remain. `waitpid(pid, &status, WNOHANG)` returns `0` if the child has not yet changed state, letting the parent continue other work. A successful `waitpid()` reaps a terminated child; it does not mean that a single nonblocking call will wait for future termination.

`SIGCHLD` normally informs a parent when a child changes state, but it is a notification, not a substitute for collecting the status. Signal disposition and platform rules can change whether a zombie is retained, so explicit waiting is the clearest approach for these examples.

## 6. Running another program with `exec()`

There is no single POSIX function literally named `exec()`. The **exec family** (`execl`, `execv`, `execlp`, `execvp`, `execve`, and others) replaces the *current process image* with a new program. A successful call does not return to the old C++ code. It does **not** create another process: the PID remains the same. If it returns `-1`, loading failed and the old program is still running at the next statement. [Linux `exec(3)`](https://man7.org/linux/man-pages/man3/exec.3.html) and [`execve(2)`](https://man7.org/linux/man-pages/man2/execve.2.html) explain the family and the replacement rules.

| Variant | Arguments | How executable is found | Environment |
| --- | --- | --- | --- |
| `execl(path, arg0, ..., null)` | Separate arguments (`l` = list) | Use a path you give it | Inherited |
| `execv(path, argv)` | Null-terminated array (`v` = vector) | Use a path you give it | Inherited |
| `execlp(file, arg0, ..., null)` / `execvp(file, argv)` | List / array | Search `PATH` when `file` has no slash (`p`) | Inherited |
| `execve(path, argv, envp)` | Arrays | Use a path you give it | Explicit `envp` array |

`arg0` conventionally names the new program and becomes its `argv[0]`; arguments after it become `argv[1]`, `argv[2]`, and so on. For `execl*`, the final null pointer marks the end of the variable-length argument list. For `execv*`, the array's final element is `nullptr`. The `p` variants search using `PATH`; without `p`, provide a usable path such as `/bin/ls` or `./worker`. Neither form automatically interprets shell syntax such as `|`, `>`, `*`, or `$HOME`. See [environment variables](LinuxEnvironmentVariables.md) for `PATH` and inheritance.

After a successful exec, the new program has fresh code, globals, heap, stack, and arguments. Some process attributes persist, including PID, working directory, and open file descriptors unless their **close-on-exec** flag is set. Caught signal handlers are reset; ignored signal dispositions generally remain ignored. The new program can therefore inherit resources and behavior you did not intend if you do not manage them.

## 7. A complete `fork()` + `exec()` + `waitpid()` example

The usual pattern for launching an external program while keeping the parent alive is:

```text
parent ──fork()──┬── parent waits for its child
                └── child execs another program (same child PID)
```

Save this as `run_ls.cpp`:

```cpp
#include <cerrno>
#include <cstdio>
#include <iostream>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

int main() {
    // Flush C++ output before duplicating the process.
    std::cout << "Starting ls -l\n" << std::flush;

    pid_t child = fork();
    if (child == -1) {
        std::perror("fork");
        return 1;
    }

    if (child == 0) {
        execl("/bin/ls", "ls", "-l", static_cast<char*>(nullptr));

        // Reached only if exec failed. write() avoids buffered C++ output here.
        constexpr char message[] = "Could not execute ls\n";
        (void)write(STDERR_FILENO, message, sizeof(message) - 1);
        _exit(127);
    }

    int status = 0;
    pid_t result;
    do {
        result = waitpid(child, &status, 0);
    } while (result == -1 && errno == EINTR);

    if (result == -1) {
        std::perror("waitpid");
        return 1;
    }
    if (WIFEXITED(status)) {
        std::cout << "Child exited with code " << WEXITSTATUS(status) << '\n';
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status)) {
        std::cout << "Child was terminated by signal " << WTERMSIG(status) << '\n';
        return 1;
    }
    return 1;
}
```

```sh
g++ -std=c++17 -Wall -Wextra run_ls.cpp -o run_ls
./run_ls
```

The parent receives the child's PID from `fork()`. The child turns into `ls`; `ls` inherits the same PID and writes to the terminal through the inherited standard-output descriptor. `/bin/ls` is a common location on Linux and macOS; substitute the actual path if needed. The parent waits for **that** child and interprets its status. If `execl()` fails, `_exit(127)` ends the child without accidentally continuing through parent code or running duplicated exit handlers. The message deliberately uses only `write()` in that branch; a real launcher may arrange a dedicated error pipe to report the precise `errno` to the parent. `127` is a conventional launcher failure code, not a unique proof that `exec` failed: the executed program could also exit with `127`.

## 8. File descriptors, redirection, and pipes

After `fork()`, the child inherits copies of the parent's open file descriptors. The descriptor numbers are separate entries, but they can refer to the same underlying open file description and thus share a file offset. This is why two processes writing through inherited descriptors can interfere or interleave. After `exec()`, descriptors normally remain open unless marked close-on-exec (`FD_CLOEXEC`). For new descriptors in programs with threads, use an atomic close-on-exec creation option where available (`O_CLOEXEC`, for example) to avoid a race with another thread launching a process.

`dup2(old_fd, new_fd)` makes `new_fd` refer to the same open file description as `old_fd`. It is commonly used in a child before `exec()` to redirect one of the standard streams:

```cpp
// In a child, after opening output_fd and before exec:
if (dup2(output_fd, STDOUT_FILENO) == -1) {
    _exit(126);
}
close(output_fd); // The duplicated stdout descriptor remains open.
// exec...; the new program writes standard output to that file.
```

That snippet assumes `output_fd` is a different descriptor from `STDOUT_FILENO` and was opened successfully. A full launcher must also handle open, close, and exec errors. `dup2()` performs the replacement atomically. [Linux `dup(2)`](https://man7.org/linux/man-pages/man2/dup.2.html) has the details.

A **pipe** is a one-way byte stream made with `pipe(fd)`: `fd[0]` is for reading, `fd[1]` for writing. Create it before `fork()` so both processes inherit the endpoints. Close unused ends in *both* processes. A reader sees end-of-file only after **every** write descriptor referring to the pipe has closed. Leaving an extra write end open can make a reader wait forever. A writer may block when the pipe is full. Reads can return fewer bytes than requested, so production code loops as needed. [Linux `pipe(7)`](https://man7.org/linux/man-pages/man7/pipe.7.html) describes these rules.

```text
parent: pipe(fd) ── fork()
          │             │
          │          child closes fd[0], writes to fd[1], closes fd[1]
          └───────── parent closes fd[1], reads fd[0] until EOF, closes fd[0]
```

For a shell-style pipeline, one child can `dup2()` a pipe's write end onto stdout, and another can `dup2()` its read end onto stdin before each child calls `exec()`. Pipes, sockets, shared memory, and files are ways to exchange data; `waitpid()` coordinates termination, not the data itself.

## 9. Signals, process groups, and job control

A **signal** is an asynchronous notification. Examples include `SIGINT` (often from Ctrl+C), `SIGTERM` (a request to terminate), `SIGKILL` (cannot be caught or ignored), `SIGCHLD` (child state changed), and `SIGPIPE` (writing to a pipe with no readers). `kill(pid, SIGTERM)` sends a signal; despite its name, `kill()` does not always terminate the target. Signal handlers have strict rules about which functions they can safely call. [Linux `signal(7)`](https://man7.org/linux/man-pages/man7/signal.7.html) provides the full model.

Shells use **process groups** to manage jobs. A foreground job can contain several processes (for example, a pipeline), and Ctrl+C is typically delivered to the foreground *process group*, not merely one PID. A session groups process groups and is associated with terminal control. This is why a shell needs more than `fork()` and `waitpid()` to implement job control. `getpgrp()`, `setpgid()`, and `tcsetpgrp()` are related POSIX interfaces.

Sending a signal is not the same as waiting for a child. If your program created the child, still collect its status. Also remember that PIDs can be reused: signaling a PID long after saving it can affect a different process.

## 10. Threads, `posix_spawn()`, and `std::system()`

`fork()` is easiest to reason about in a single-threaded process. If a process has several threads, only the thread that called `fork()` exists in the child. Memory still contains the state left by the other threads, including possibly locked mutexes. Before `exec()`, the child must restrict itself to async-signal-safe operations; calling complex C++ code, `std::cout`, memory allocators, or code that locks a mutex can deadlock or be unsafe. The short child branch in the `run_ls.cpp` example uses `execl()`, `write()`, and `_exit()` for this reason. POSIX lists `execl()` as async-signal-safe, but does not make the same guarantee for the PATH-searching `execlp()` and `execvp()` variants. Libraries and `pthread_atfork()` handlers make the full situation more subtle; plan carefully if a multithreaded application launches processes. [Linux `fork(2)`](https://man7.org/linux/man-pages/man2/fork.2.html), [`pthread_atfork(3)`](https://man7.org/linux/man-pages/man3/pthread_atfork.3.html), and [`signal-safety(7)`](https://man7.org/linux/man-pages/man7/signal-safety.7.html) discuss the restrictions.

`posix_spawn()` and `posix_spawnp()` provide a POSIX way to create a child and execute a program in one interface, with file actions for redirection. They can simplify launching a program from a larger application; they still return a child PID whose status you normally collect with `waitpid()`. Check their return value directly: unlike many POSIX calls, they return an **error number**, not `-1` with `errno` set. Depending on the platform and failure stage, an exec failure can instead appear as a child that exits with code `127`. [Linux `posix_spawn(3)`](https://man7.org/linux/man-pages/man3/posix_spawn.3.html) explains the details.

`std::system("command")` is the simple C++ library option for a command string. It invokes a command processor (usually a shell on POSIX), so shell expansion and quoting apply. Its integer result is not necessarily the command's plain exit code. Do not build a shell command by concatenating untrusted text: shell metacharacters can change what runs. When you already have an executable and separate argument values, an exec variant or `posix_spawn()` lets you pass them without shell parsing.

Windows has `CreateProcess` and different handle and waiting APIs. `std::thread` is standard C++ for threads, but the C++ standard library has no direct `fork()` or `exec()` equivalent.

## 11. Observing processes and common mistakes

These Linux commands help you inspect a running example (replace numbers with real PIDs):

```sh
ps -o pid,ppid,stat,comm -p 1234
ps -ef
pstree -p
top
```

`STAT` can show `Z` for a zombie. A very short-lived process may finish before you inspect it. `pstree` may not be installed; `ps` is more commonly available. The `/proc/PID/` directory on Linux exposes process details such as command line and open descriptors; it is Linux-specific. See [Linux commands](LinuxCommands.md) for related terminal commands.

| Mistake | Better model or action |
| --- | --- |
| Thinking `fork()` restarts `main()` | Both processes resume *after* `fork()`; only their return values differ. |
| Thinking a child sees the parent's later variable updates | Ordinary memory is separate after the fork. Use IPC for communication. |
| Calling `wait()` and assuming it collected a particular child | Use `waitpid(child_pid, ...)` when that child matters. |
| Treating `status` as an exit code | Check `WIFEXITED()` before `WEXITSTATUS()`, or `WIFSIGNALED()` before `WTERMSIG()`. |
| Assuming successful `exec()` returns | Put post-exec code only in the failure path. |
| Forgetting to end the child when exec fails | Report the failure and call `_exit()`; do not fall through into parent logic. |
| Forgetting pipe ends or inherited descriptors | Close unused ends and set close-on-exec where appropriate. |
| Expecting output from parent and child in a fixed order | Synchronize explicitly; scheduling order is unspecified. |
| Assuming all processes use the POSIX API | Windows needs its own process API. |

## 12. Practice and references

1. Modify `fork_demo.cpp` so the parent changes `value` to `30` after `fork()`. Confirm the child's value remains `20`.
2. Change `run_ls.cpp` to run `/bin/pwd` with `execl()`. Try a nonexistent executable and inspect the failure code.
3. Start two children, keep both returned PIDs, and collect each with `waitpid()`. Give them different exit codes.
4. Create a pipe before `fork()`. Have the child write a short message and close its write end; have the parent read until EOF, then reap the child.
5. Run `ps -o pid,ppid,stat,comm` while a child sleeps, and compare its PID and PPID with `getpid()` and `getppid()`.

For exact interface details and platform-specific behavior, use the local manual pages (`man 2 fork`, `man 2 wait`, `man 3 exec`, `man 3 posix_spawn`) or the linked Linux manual pages above. POSIX interfaces are shared by many Unix-like systems, but details such as `/proc`, feature flags, and signal behavior can differ.
