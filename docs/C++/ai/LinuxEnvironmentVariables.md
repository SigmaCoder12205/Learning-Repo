# Environment variables in Linux: a beginner's guide

*A practical guide to named settings, `export`, `PATH`, and making values available to programs. Examples use Bash or Zsh on Linux; Fish and other shells can use different syntax. The final C++ example uses C++17.*

## Contents

1. [The idea: settings passed to a program](#1-the-idea-settings-passed-to-a-program)
2. [Shell variables versus environment variables](#2-shell-variables-versus-environment-variables)
3. [Reading values and using quotes](#3-reading-values-and-using-quotes)
4. [Common environment variables](#4-common-environment-variables)
5. [Inheritance: who receives a change?](#5-inheritance-who-receives-a-change)
6. [Setting a value for one command](#6-setting-a-value-for-one-command)
7. [`PATH`: how the shell finds programs](#7-path-how-the-shell-finds-programs)
8. [Keeping settings across terminal sessions](#8-keeping-settings-across-terminal-sessions)
9. [Empty values, missing values, and defaults](#9-empty-values-missing-values-and-defaults)
10. [Scripts and `.env` files](#10-scripts-and-env-files)
11. [Reading an environment variable in C++](#11-reading-an-environment-variable-in-c)
12. [A worked example](#12-a-worked-example)
13. [Common mistakes, quick reference, and practice](#13-common-mistakes-quick-reference-and-practice)

## 1. The idea: settings passed to a program

Imagine giving a worker a small card of settings before they start a job:

```text
APP_MODE=development
APP_GREETING=Hello
```

An **environment variable** is a named text value in a process's environment. Programs can read these values to choose settings without putting those settings directly in their source code.

For example, a program might read `APP_MODE` to decide whether to print extra debugging information. Merely setting the name does nothing unless a program has been written to use it.

**An environment variable is not automatically a system-wide variable.** Each process has its own environment. When a process launches another program, it normally passes an environment to it, often based on its own current values. Section 5 explains this inheritance.

If you need a refresher on the terminal, paths, or commands, start with [Basic Linux commands](LinuxCommands.md).

## 2. Shell variables versus environment variables

### A shell variable: a value the shell knows

```sh
APP_MESSAGE="Hello from Linux"
printf '%s\n' "$APP_MESSAGE"
```

Output:

```text
Hello from Linux
```

This creates a shell variable. For a fresh name that has not already been exported, it is available in the current shell but is not automatically passed to external programs.

**There must be no spaces around `=` in an assignment.**

```sh
APP_MODE=development
```

Writing `APP_MODE = development` makes the shell try to run a command named `APP_MODE` instead.

### `export`: include a variable in child programs' environments

```sh
export APP_MESSAGE
printenv APP_MESSAGE
```

`export` marks the variable for inclusion in the environment passed to subsequently launched programs. `printenv` is a separate program, so its output demonstrates that it received the exported value.

You can assign and export in one command:

```sh
export APP_MODE=development
```

| Operation | Current shell can use it? | Subsequently launched programs receive it? |
| --- | --- | --- |
| `APP_MODE=development`, starting with an unexported name | Yes | No |
| `export APP_MODE` | Yes | Yes |
| `export APP_MODE=development` | Yes | Yes |
| `unset APP_MODE` | No; the variable is removed | No value is passed under that name |

Once a name is exported, assigning a new value normally keeps it exported. You do not need to repeat `export` after each assignment.

Uppercase names with underscores are conventional for environment variables. Names are case-sensitive: `APP_MODE` and `app_mode` are different. For ordinary shell assignments, use letters, digits, and underscores, with no leading digit.

## 3. Reading values and using quotes

Use `$NAME` or `${NAME}` to expand a variable's value:

```sh
APP_NAME=demo
printf '%s\n' "$APP_NAME"
printf '%s\n' "${APP_NAME}_backup"
```

Output:

```text
demo
demo_backup
```

Braces make the name's boundary clear. `$APP_NAME_backup` refers to a variable named `APP_NAME_backup`, which is a different name.

### Single quotes versus double quotes

```sh
APP_NAME=demo
printf '%s\n' "$APP_NAME"
printf '%s\n' '$APP_NAME'
```

Output:

```text
demo
$APP_NAME
```

- **Double quotes** allow variable expansion while keeping the result together as one argument.
- **Single quotes** preserve literal text, including `$`.

Use double quotes around values when passing them as arguments:

```sh
NOTES_DIR="$HOME/practice notes"
mkdir -p "$NOTES_DIR"
```

In Bash, leaving `$NOTES_DIR` unquoted can split its value into several arguments and expand wildcard characters. Default Zsh handles some unquoted expansions differently; quoting still gives the clear, portable behaviour wanted here.

The variable's name is `HOME`, not `$HOME`. The `$` requests its value. Write `export APP_MODE=development`, not `export $APP_MODE=development`.

### See one value or the exported environment

```sh
printenv HOME
printenv
env
```

`printenv HOME` prints one environment value. With no names, `printenv` lists its received environment; `env` without a command also lists environment entries. They do not show ordinary unexported shell variables. Use `printf '%s\n' "$NAME"` to display a shell variable's value.

Environment listings can contain tokens or passwords. Check the output before sharing it in logs, screenshots, or bug reports.

## 4. Common environment variables

| Name | Usual meaning | Example value or use |
| --- | --- | --- |
| `HOME` | The current user's home directory | `/home/ada` |
| `USER` | A username supplied by the login/session environment | `ada` |
| `PATH` | Colon-separated directories used when searching for external commands | `/usr/local/bin:/usr/bin:/bin` |
| `PWD` | The current directory as maintained by the shell | `/home/ada/projects` |
| `SHELL` | Usually the user's configured login shell | `/usr/bin/zsh` |
| `EDITOR` | A preferred editor recognised by programs that consult it | `nano` |
| `LANG` | A default locale setting, affecting language and formatting | `en_AU.UTF-8`, if installed |
| `TMPDIR` | A preferred temporary directory for programs that honour it | Often unset; defaults depend on the program |

These are conventions. Some variables may be unset, and individual programs choose which ones they honour. `EDITOR` does not force every application to use the same editor.

**Watch out for.** `SHELL` does not reliably identify the shell interpreting your current commands. Starting Bash from Zsh can leave `SHELL` pointing to Zsh. In an interactive terminal, `ps -p $$ -o comm=` can help identify the current shell process. `$$` is a special shell parameter holding its PID.

Likewise, changing `USER` does not change your actual account identity; use `id -un` to ask for your effective username. Changing `HOME` does not move your files, and assigning `PWD` is not a replacement for `cd`.

## 5. Inheritance: who receives a change?

Think of a child process receiving a copy of its parent's settings card:

```text
terminal shell: APP_MODE=development, exported
│
├── program started now: receives development
│
└── child shell: receives development
    └── program started there: normally receives development too
```

```sh
export APP_MODE=development
sh -c 'printf "%s\n" "$APP_MODE"'
```

This prints `development`. The single quotes stop the **parent** shell from expanding `$APP_MODE`; the new `sh` reads the inherited value itself.

A child can change its own copy without changing the parent:

```sh
sh -c 'APP_MODE=production; printf "%s\n" "$APP_MODE"'
printf '%s\n' "$APP_MODE"
```

Output:

```text
production
development
```

The important consequences are:

- Exporting a value affects programs launched afterward from that shell.
- Already-running programs do not automatically receive the change.
- Another existing terminal does not automatically receive it either.
- Running a child script cannot ordinarily change its parent shell's variables.
- Closing the shell loses values that have not been arranged to load again.

Desktop applications, services, and scheduled jobs may start from different parents. An `export` typed in one terminal does not automatically configure them.

## 6. Setting a value for one command

Put an assignment before an external command to supply a value only for that invocation:

```sh
APP_MODE=testing printenv APP_MODE
```

Output:

```text
testing
```

The current shell's previous `APP_MODE` value is unchanged. This form does not require a separate `export`.

You can pass several settings:

```sh
APP_MODE=testing APP_COLOUR=blue env
```

`env` also supports supplying values and removing one for a command:

```sh
env APP_MODE=testing printenv APP_MODE
env -u APP_MODE sh -c 'printf "%s\n" "${APP_MODE-unset}"'
```

The second command prints `unset` without removing the variable from your current shell.

### A subtle trap: expansion happens before the command starts

```sh
APP_MODE=development
APP_MODE=testing printf '%s\n' "$APP_MODE"
```

This prints `development`: the shell expands `"$APP_MODE"` while preparing the command, before the temporary assignment takes effect for the command. To have a program read the temporary environment value, use `printenv APP_MODE` or launch a child shell as above.

## 7. `PATH`: how the shell finds programs

`PATH` is a list of directory paths separated by colons:

```text
/home/ada/.local/bin:/usr/local/bin:/usr/bin:/bin
         first            second      third fourth
```

When resolving a bare command name, the shell also considers things such as aliases, functions, and builtins. For an external program found through `PATH`, earlier directories have priority.

```sh
printf '%s\n' "$PATH"
command -v g++
type ls
```

`command -v g++` shows how the command resolves if it is available. If the shell cannot find a requested command, it usually reports `command not found`.

### Add a directory without losing the existing list

```sh
export PATH="$HOME/.local/bin:$PATH"
```

This **prepends** your local directory. To give existing directories priority, append it instead:

```sh
export PATH="$PATH:$HOME/.local/bin"
```

Choose one placement; these examples are alternatives. The directory must exist and contain suitable executable files for the addition to be useful. Repeating a simple prepend adds duplicate entries; a fresh session avoids changes accumulated while experimenting.

**Watch out for.** `export PATH="$HOME/.local/bin"` replaces the whole search list. Common commands may stop resolving because their directories are no longer listed.

An empty `PATH` component, such as the final component in `/usr/bin:`, means the current directory. Prefer explicitly running local programs with `./program` rather than adding `.` or an empty component to `PATH`.

A command containing `/`, such as `./hello` or `/usr/bin/printenv`, supplies a path directly and bypasses the `PATH` search for that executable.

## 8. Keeping settings across terminal sessions

An `export` typed at the prompt lasts in that shell and can be inherited by its children. To recreate a setting in later shells, put the assignment in an appropriate startup file.

### Interactive terminal settings

For a typical interactive terminal, edit the file for the shell you actually use:

| Shell | Common per-user interactive startup file |
| --- | --- |
| Bash | `~/.bashrc`, read by interactive non-login Bash shells |
| Zsh | `${ZDOTDIR:-$HOME}/.zshrc`, normally `~/.zshrc` |

Add lines such as:

```sh
export EDITOR=nano
export APP_MODE=development
```

Use an editor you have installed. These example values become available to programs started from shells that load the file. They do not reconfigure already-running applications.

After saving, open a new terminal. To apply the file in your current shell, use the matching command:

```sh
# Bash:
source ~/.bashrc
```

```sh
# Zsh:
source "${ZDOTDIR:-$HOME}/.zshrc"
```

Choose the command for your shell; do not load a Bash startup file into Zsh or vice versa. `source` runs the **whole file**, so any other commands or repeated `PATH` additions inside it run again too. In interactive Zsh, skip example comment lines beginning with `#` unless interactive comments are enabled.

### Login shells and other launch contexts

Startup files are not interchangeable:

- **Bash login shells** read `/etc/profile`, then the first readable file among `~/.bash_profile`, `~/.bash_login`, and `~/.profile`. They do not automatically read `~/.bashrc`; a login file often sources it explicitly.
- **Zsh login shells** also read a login file, normally `~/.zprofile`. Interactive Zsh reads `.zshrc` whether or not it is a login shell. Zsh's user startup-file location can be changed through `ZDOTDIR`.
- **Services and desktop sessions** have their own launch and configuration rules. `.bashrc` and `.zshrc` are not universal configuration files for every Linux process.

For ordinary terminal use, begin with the interactive file for your shell. If a setting is missing elsewhere, first identify how that application is launched.

## 9. Empty values, missing values, and defaults

An empty variable exists with a zero-length value. An unset variable does not exist:

```sh
export APP_NOTE=""
printenv APP_NOTE
unset APP_NOTE
```

Before `unset`, `printenv` prints a blank line. After `unset`, `printenv APP_NOTE` prints nothing and returns a nonzero status. Merely printing `"$APP_NOTE"` normally looks blank in both cases; shells configured to reject unset variables can instead report an error.

### Use a fallback value

```sh
unset APP_COLOUR
printf '%s\n' "${APP_COLOUR:-blue}"
```

Output:

```text
blue
```

| Expansion | Meaning |
| --- | --- |
| `${NAME:-fallback}` | Use the fallback if `NAME` is unset **or empty** |
| `${NAME-fallback}` | Use the fallback only if `NAME` is unset |
| `${NAME+x}` | Produce `x` if `NAME` is set, even if its value is empty |

These expansions do not assign the fallback to the variable. They choose text to use in that particular expansion.

Remove a variable with `unset NAME`, without `$`:

```sh
unset APP_COLOUR
```

## 10. Scripts and `.env` files

### Executing versus sourcing a script

Suppose `settings.sh` contains:

```sh
export APP_MODE=development
```

Running it in a new shell changes that child, not your current shell:

```sh
sh settings.sh
```

To execute its assignments in the current shell, source it:

```sh
. ./settings.sh
```

For Bash and Zsh, `source ./settings.sh` is another spelling. Sourcing executes shell code and can change variables, directories, and other shell state, so only source files you trust and that are compatible with your shell.

### A `.env` file is not loaded automatically by Linux

A project might contain:

```dotenv
APP_MODE=development
APP_PORT=8080
```

The filename `.env` has no automatic meaning to the Linux kernel or a normal shell session. A framework, library, or launch tool must load it. Different `.env` loaders also support different quoting and expansion rules.

In an ordinary shell, merely sourcing those two assignments creates shell variables for fresh names; without `export`, they are not automatically passed to programs. If a project deliberately provides a **shell-compatible settings script**, explicit `export` lines make its intent clear.

Keep secret-bearing configuration out of version control. Environment variables are a way to pass configuration, not a guarantee that a secret cannot be exposed by logs, diagnostics, or other processes with sufficient access.

## 11. Reading an environment variable in C++

Shell expansion and a program reading its environment are separate operations. A C++ program can read its environment directly with `std::getenv` from `<cstdlib>`.

Save this complete program as `show_env.cpp`:

```cpp
#include <cstdlib>
#include <iostream>

int main() {
    const char* mode = std::getenv("APP_MODE");

    if (mode == nullptr) {
        std::cout << "APP_MODE is not set.\n";
    } else if (mode[0] == '\0') {
        std::cout << "APP_MODE is set but empty.\n";
    } else {
        std::cout << "APP_MODE = " << mode << '\n';
    }
}
```

Compile and run it from the directory containing the source:

```sh
g++ -std=c++17 -Wall -Wextra show_env.cpp -o show_env
env -u APP_MODE ./show_env
APP_MODE=testing ./show_env
APP_MODE= ./show_env
```

Output from the three runs:

```text
APP_MODE is not set.
APP_MODE = testing
APP_MODE is set but empty.
```

**Walk-through.** `std::getenv` receives the name `"APP_MODE"`, without `$`. It returns a null pointer if the variable is absent; otherwise it returns a pointer to a null-terminated string. Check for `nullptr` before reading the string, and do not modify the returned characters. The program treats an empty string separately from a missing variable.

All environment values arrive as text. If `APP_PORT` contains `8080`, your program must still validate and convert it before treating it as an integer.

### Environment variables versus command-line arguments

```sh
APP_MODE=testing ./show_env extra
```

`APP_MODE` is supplied through the environment. `extra` is a command-line argument; the program above ignores it. An application may support either method or both, and it decides which takes priority. See [Command-line arguments in C++](CommandLineArguments.md) for `argc` and `argv`.

## 12. A worked example

This uses only the current shell and its children. Run the commands in order:

```sh
unset DEMO_MESSAGE
DEMO_MESSAGE="Hello from the shell"

printf '%s\n' "$DEMO_MESSAGE"
sh -c 'printf "%s\n" "${DEMO_MESSAGE-not inherited}"'

export DEMO_MESSAGE
sh -c 'printf "%s\n" "$DEMO_MESSAGE"'

DEMO_MESSAGE="One command only" printenv DEMO_MESSAGE
printf '%s\n' "$DEMO_MESSAGE"

unset DEMO_MESSAGE
printf '%s\n' "${DEMO_MESSAGE:-No message set}"
```

Output:

```text
Hello from the shell
not inherited
Hello from the shell
One command only
Hello from the shell
No message set
```

**Walk-through.** The first assignment is local to the shell because the name starts unexported. `export` makes the value available to later child processes. The one-command assignment overrides it for `printenv` alone. `unset` removes it, so the final expansion uses its fallback.

## 13. Common mistakes, quick reference, and practice

### Common mistakes

| The mistake | The fix |
| --- | --- |
| `APP_MODE = development` | Remove spaces around `=` |
| `export $APP_MODE=development` | Use the variable's name: `export APP_MODE=development` |
| Using single quotes and expecting expansion | Use `"$APP_MODE"`; `'$APP_MODE'` is literal text |
| Expecting an ordinary shell variable to reach a child program | Export it or use a one-command assignment |
| Expecting a child script to update its parent shell | Source a trusted shell-compatible script when that is the intent |
| Assuming `export` configures the entire system | Changes apply to this shell's future descendants unless another launcher loads them |
| Replacing `PATH` while trying to extend it | Preserve `"$PATH"` when adding a directory |
| Assuming `SHELL` always identifies the current shell | Check the running shell process |
| Creating `.env` and expecting Linux to load it | Use the loader documented by your application |
| Expecting a number in an environment variable to be a numeric type | Read, validate, and convert the text |

### Quick reference

| Job | Syntax |
| --- | --- |
| Assign a shell variable | `APP_MODE=development` |
| Assign and export | `export APP_MODE=development` |
| Read a shell value | `printf '%s\n' "$APP_MODE"` |
| Read an exported value | `printenv APP_MODE` |
| List the exported environment | `printenv` |
| Set a value for one program | `APP_MODE=testing ./program` |
| Remove a variable | `unset APP_MODE` |
| Remove it only for one program | `env -u APP_MODE ./program` |
| Use a default for an empty or missing value | `"${APP_MODE:-development}"` |
| Put text directly after a variable | `"${APP_MODE}_settings"` |
| Prepend an executable directory | `export PATH="$HOME/.local/bin:$PATH"` |
| Load a trusted settings script into this shell | `. ./settings.sh` |

### Practice ideas

1. Create `PRACTICE_NAME` without exporting it. Compare shell expansion with `printenv PRACTICE_NAME`, then export it and try again.
2. Give one `printenv` invocation a temporary value and show that the parent shell's value remains unchanged.
3. Compare `${PRACTICE_NAME:-Guest}` and `${PRACTICE_NAME-Guest}` when the variable is empty and when it is unset.
4. Inspect `PATH` and use `command -v` to locate a program you already use.
5. Compile `show_env.cpp` and test missing, empty, and nonempty values.

**Rule to remember:** assignment gives the shell a value; exporting makes it available to future child programs; a startup file recreates it in later shells that read that file.
