# Command-line arguments in C++: `argc` and `argv`

*A beginner's guide to passing values to a program from the terminal. The commands below use a Linux or macOS shell and a C++17 compiler.*

## 1. The idea: a delivery slip for your program

Imagine starting a program is like sending a worker on a job. The **program name** says which worker to send, and the words after it are instructions written on a delivery slip:

```text
./greet Ada 3
   │     │  │
   │     │  └─ argument: how many greetings?
   │     └──── argument: whom to greet?
   └────────── program to run
```

In C++, `main` receives that slip through two parameters:

```cpp
int main(int argc, char* argv[])
```

- **`argc`** means *argument count*: the number of entries on the slip, **including the program name**. For the command above, it is `3`.
- **`argv`** means *argument vector*: an indexed list of the entries as text. For the command above, `argv[0]` is typically `"./greet"`, `argv[1]` is `"Ada"`, and `argv[2]` is `"3"`.

The names `argc` and `argv` are conventional; their types and positions are what matter. The terminal's shell splits the command into arguments before your program starts. Your program then reads them from `main`.

## 2. See what the program actually receives

Save this complete program as `show_args.cpp`:

```cpp
#include <iostream>

int main(int argc, char* argv[]) {
    std::cout << "argc = " << argc << '\n';

    for (int i = 0; i < argc; ++i) {
        std::cout << "argv[" << i << "] = " << argv[i] << '\n';
    }

    return 0;
}
```

Compile it, then run it from the same directory:

```sh
g++ -std=c++17 -Wall -Wextra show_args.cpp -o show_args
./show_args red blue
```

You should see:

```text
argc = 3
argv[0] = ./show_args
argv[1] = red
argv[2] = blue
```

`g++` turns the source file into an executable named `show_args`. The `./` in `./show_args` means “run the file in the current directory.” Anything after the executable name is passed to it as an argument. The `for` loop starts at index `0` and stops **before** `argc`, so it prints every valid entry.

On Windows with PowerShell and `g++`, compile with `g++ -std=c++17 show_args.cpp -o show_args.exe`, then run `.\show_args.exe red blue`. PowerShell uses `.\` to run an executable in the current directory.

Try it with no extra arguments:

```sh
./show_args
```

```text
argc = 1
argv[0] = ./show_args
```

Even though you supplied no *extra* arguments, the program name still occupies `argv[0]`. In the usual terminal case, the number of values you supplied after the program name is `argc - 1`.

## 3. How `argv` stores text

The type `char* argv[]` means “an array of pointers to characters.” Each pointer leads to a **C-style string**: characters ending with a special `\0` marker. This is why `argv[i]` can be printed as text.

```text
argv[0] ──> './show_args\0'
argv[1] ──> 'red\0'
argv[2] ──> 'blue\0'
```

`char* argv[]` and `char** argv` are equivalent ways to write this parameter in `main`. Both give you access to `argv[0]`, `argv[1]`, and so on. Use `i < argc` when reading entries: `argv[argc]` is a null pointer marking the end, **not** another string to print. There is no valid `argv[1]` to read when `argc` is only `1`.

If you want the convenience of `std::string`, make one from an entry:

```cpp
#include <string>

// Inside main, after checking that argc >= 2:
std::string name = argv[1];
```

`argv[0]` usually contains the name or path used to start the program, but it is not a reliable way to find the executable's full path. Its exact text depends on how the program was launched.

## 4. Quotes, spaces, and the shell

The **shell** groups text into arguments. Spaces normally separate them; quotes make several words one argument:

```sh
./show_args red apple
```

```text
argc = 3
argv[0] = ./show_args
argv[1] = red
argv[2] = apple
```

```sh
./show_args "red apple"
```

```text
argc = 2
argv[0] = ./show_args
argv[1] = red apple
```

The quote characters group the words; they are not part of `argv[1]`. Single quotes also group words in a Linux or macOS shell:

```sh
./show_args 'red apple' blue
```

Here `argv[1]` is `"red apple"`, `argv[2]` is `"blue"`, and `argc` is `3`. The shell can also expand things such as `*` and variables before the program receives its arguments, so the text typed in the terminal is not always identical to the text in `argv`.

## 5. A useful program: required and optional arguments

Suppose `greet` needs a name and can optionally take a number of times to greet. Save this as `greet.cpp`:

```cpp
#include <iostream>
#include <stdexcept>
#include <string>

int main(int argc, char* argv[]) {
    // The program name counts, so 2 means one user-supplied argument.
    if (argc != 2 && argc != 3) {
        std::cerr << "Usage: ./greet NAME [TIMES]\n";
        return 1;
    }

    std::string name = argv[1];
    int times = 1;  // Default when TIMES is absent.

    if (argc == 3) {
        std::string input = argv[2];
        std::size_t charactersRead = 0;

        try {
            times = std::stoi(input, &charactersRead);
        } catch (const std::invalid_argument&) {
            std::cerr << "TIMES must be a positive integer.\n";
            return 1;
        } catch (const std::out_of_range&) {
            std::cerr << "TIMES is too large.\n";
            return 1;
        }

        if (charactersRead != input.size() || times <= 0) {
            std::cerr << "TIMES must be a positive integer.\n";
            return 1;
        }
    }

    for (int i = 0; i < times; ++i) {
        std::cout << "Hello, " << name << "!\n";
    }

    return 0;
}
```

Compile and try the different cases:

```sh
g++ -std=c++17 -Wall -Wextra greet.cpp -o greet
./greet Ada
```

```text
Hello, Ada!
```

```sh
./greet "Ada Lovelace" 3
```

```text
Hello, Ada Lovelace!
Hello, Ada Lovelace!
Hello, Ada Lovelace!
```

The program checks `argc` *before* reading `argv[1]` or `argv[2]`. The optional `TIMES` arrives as text, even when you type `3`, so `std::stoi` converts it to an `int`. The `charactersRead` check rejects inputs such as `3cats`, which `std::stoi` would otherwise partly convert. The two `catch` blocks handle text that is not a number or is outside the range of `int`.

```sh
./greet
# Usage: ./greet NAME [TIMES]

./greet Ada three
# TIMES must be a positive integer.
```

The square brackets in `NAME [TIMES]` mean that `TIMES` is optional; you do **not** type the brackets. `std::cerr` prints errors, and `return 1` reports failure to the shell. `return 0` reports success.

## 6. Arguments versus input while the program runs

Arguments are supplied **when you launch** the program:

```sh
./greet Ada 2
```

If you want the program to ask a question *after* it starts, use input such as `std::cin` instead:

```cpp
#include <iostream>
#include <string>

int main() {
    std::string name;
    std::cout << "Your name? ";
    std::getline(std::cin, name);
    std::cout << "Hello, " << name << "!\n";
}
```

That program waits for the user to type a name. A program may use both methods, but they solve different timing needs.

## Quick reference

| Expression | Meaning | Example for `./greet Ada 3` |
| --- | --- | --- |
| `argc` | Number of entries, including the program name | `3` |
| `argv[0]` | Program name or invocation text | Usually `"./greet"` |
| `argv[1]` | First argument after the program name | `"Ada"` |
| `argv[2]` | Second argument after the program name | `"3"` (text, not an `int`) |
| `argv[argc]` | Null pointer ending the array | Not a printable argument |

**Rule to remember:** check `argc` before using an `argv` index, and convert argument text before treating it as a number.
