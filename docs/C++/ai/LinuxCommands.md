# Basic Linux commands: a beginner's guide

*A practical guide to moving around, working with files, and joining commands together. Examples use a Linux terminal with Bash or Zsh. Most commands are available on a typical Linux installation; tools such as `less`, `man`, and `g++` may need installing separately.*

## Contents

1. [The picture to keep in your head](#1-the-picture-to-keep-in-your-head)
2. [How to read a command](#2-how-to-read-a-command)
3. [Paths: where files and folders live](#3-paths-where-files-and-folders-live)
4. [Moving around: `pwd`, `ls`, and `cd`](#4-moving-around-pwd-ls-and-cd)
5. [Creating files and folders](#5-creating-files-and-folders)
6. [Copying, moving, and deleting](#6-copying-moving-and-deleting)
7. [Reading and searching text](#7-reading-and-searching-text)
8. [Redirection and pipes](#8-redirection-and-pipes)
9. [Permissions and running programs](#9-permissions-and-running-programs)
10. [Processes and disk space](#10-processes-and-disk-space)
11. [Getting help and using the terminal](#11-getting-help-and-using-the-terminal)
12. [A worked example](#12-a-worked-example)
13. [Common mistakes, quick reference, and practice](#13-common-mistakes-quick-reference-and-practice)

## 1. The picture to keep in your head

Imagine your computer's files as a building full of rooms. Each **directory** is a room, and your shell has one room it currently considers your location. Commands can inspect that room, move to another room, or work on files elsewhere by giving their addresses.

The **terminal** is the application displaying text and accepting your typing. The **shell**, such as Bash or Zsh, interprets the commands you type and starts programs. Linux is the operating system underneath them.

Some commands, such as `cd`, are built into the shell. Others, such as `cp`, are separate programs. You use both in much the same way, but the shell also handles things such as quotes, variables, and pipes before or while running them.

Examples below show commands without a prompt. If another guide shows `$ ls`, the `$` usually represents the prompt; type only `ls`.

## 2. How to read a command

A common shape is:

```text
command [options] [arguments]
```

The square brackets mean “optional”; you do not type them.

```sh
ls -l /tmp
```

```text
ls       -l          /tmp
│        │           │
│        │           └─ argument: the directory to inspect
│        └───────────── option: request a detailed listing
└────────────────────── command: list directory contents
```

**Options** change a command's behaviour. **Arguments** supply things to work on, such as filenames. Many programs accept short options such as `-l` and long options such as `--help`, but each command defines its own options.

Several short options can often be combined:

```sh
ls -l -a
ls -la
```

Both request a detailed listing including hidden entries. Do not assume the same letter means the same thing in every command.

### Spaces, quotes, and capital letters

Spaces normally separate words. Quote a filename containing spaces:

```sh
mkdir "practice notes"
cd "practice notes"
```

Without quotes, `mkdir practice notes` creates **two** directories. Single quotes also group words; their difference from double quotes matters when using [variables](LinuxEnvironmentVariables.md#3-reading-values-and-using-quotes).

On typical Linux filesystems, `Notes.txt` and `notes.txt` are different names. Command names are also case-sensitive: use `ls`, not `LS`.

## 3. Paths: where files and folders live

Linux organises files into a tree beginning at `/`, called the **root directory**:

```text
/
├── home/
│   └── ada/
│       └── projects/
│           └── hello.cpp
├── tmp/
└── usr/
    └── bin/
```

An **absolute path** starts at `/`. A **relative path** starts from your current working directory.

If your current directory is `/home/ada`, these can refer to the same file:

```text
/home/ada/projects/hello.cpp    absolute
projects/hello.cpp             relative
./projects/hello.cpp           relative, explicitly starting here
```

| Symbol | Meaning | Example |
| --- | --- | --- |
| `/` | The root directory; also separates path components | `/home/ada` |
| `.` | The current directory | `./hello` |
| `..` | The parent directory, one level up | `../notes.txt` |
| `~` | Your home directory when expanded by the shell | `~/projects` |

`/` and your home directory are different places. Your home is commonly `/home/yourname`; `/` is the top of the entire tree.

**Watch out for.** In Bash and Zsh, quoting the tilde prevents its usual expansion: `"~/projects"` is not your home-directory path. Use `~/projects` or `"$HOME/projects"`.

## 4. Moving around: `pwd`, `ls`, and `cd`

### `pwd`: print the working directory

```sh
pwd
```

Example output:

```text
/home/ada/projects
```

It tells you where relative paths currently begin. It does not list files.

### `ls`: list files and directories

```sh
ls
ls -l
ls -a
ls -lh
ls /tmp
```

| Form | What it does |
| --- | --- |
| `ls` | Lists visible entries in the current directory |
| `ls -l` | Shows details such as permissions, owner, size, and modification time |
| `ls -a` | Includes names beginning with `.`, such as `.config` |
| `ls -lh` | Shows detailed output with readable sizes such as `4.0K` |
| `ls /tmp` | Lists a different directory without moving there |

A leading dot is the usual hidden-file convention. Hidden files still exist and can be opened normally.

### `cd`: change directory

```sh
cd /tmp
cd ..
cd ~
cd -
```

- `cd /tmp` moves to an absolute path.
- `cd ..` moves one level up.
- `cd ~`, or just `cd`, moves to your home directory.
- `cd -` returns to the previous working directory, when one is recorded.

`cd` changes your location; it does not move files. A relative command such as `cd projects` succeeds only if that directory exists under your current location.

## 5. Creating files and folders

For the file examples in sections 5–8, start in a new practice directory. Choose another name if `linux-practice` already exists:

```sh
cd ~
mkdir linux-practice
cd linux-practice
```

### `mkdir`: make a directory

```sh
mkdir notes
mkdir -p projects/demo/src
```

`mkdir notes` creates one directory. `-p` creates missing parent directories too, and accepts directories that already exist.

### `touch`: create an empty file or update timestamps

```sh
touch notes.txt
```

If `notes.txt` does not exist, this creates an empty file. If it already exists, `touch` normally updates its access and modification timestamps. **It does not empty an existing file.**

### `printf`: write text

```sh
printf 'First line\nSecond line\n' > notes.txt
```

`printf` formats and prints text. Here `\n` produces a newline, and `>` sends the output into a file. This replaces any previous contents of `notes.txt`; section 8 explains redirection in more detail.

## 6. Copying, moving, and deleting

### `cp`: copy a file or directory

```sh
cp notes.txt notes-copy.txt
cp notes.txt notes/
cp -r notes notes-backup
```

The first makes another file. The second copies into an existing directory. The third recursively copies a directory and its contents; here `notes-backup` is assumed not to exist yet.

If a destination file already exists, `cp` can overwrite it. Use `cp -i source destination` to ask before overwriting.

### `mv`: move or rename

```sh
mv notes-copy.txt draft.txt
mv draft.txt notes/
```

Renaming and moving use the same command. The first changes a name within the current directory; the second moves that file into `notes`. Use `mv -i` to ask before overwriting an existing destination file.

### `rm` and `rmdir`: remove things

```sh
rm -i notes/draft.txt
rmdir projects/demo/src
```

`rm -i` asks before deleting the file. `rmdir` removes a directory only if it is empty.

To remove a directory **and everything inside it**, `rm` needs recursive mode:

```sh
rm -ri notes-backup
```

`-r` means recursive, and `-i` requests confirmation. Check your current directory with `pwd` and inspect the target before deleting. **`rm` normally deletes directly rather than moving files to the desktop's Trash.**

### Wildcards: matching several names

```sh
ls *.txt
```

The shell expands `*.txt` into matching names in the current directory. `*` means any sequence of characters; `?` matches one character. Ordinary `*` does not include leading-dot names by default in Bash or Zsh.

If nothing matches, default Bash usually passes the pattern unchanged, while default Zsh reports `no matches found`. Quotes prevent expansion: `"*.txt"` is literal text. This is useful with `find` in the next section.

## 7. Reading and searching text

### `cat`, `less`, `head`, and `tail`

```sh
cat notes.txt
less notes.txt
head -n 5 notes.txt
tail -n 5 notes.txt
```

| Command | What it does |
| --- | --- |
| `cat notes.txt` | Prints the entire file; `cat` is short for concatenate |
| `less notes.txt` | Opens a scrollable view; press `q` to quit |
| `head -n 5 notes.txt` | Prints the first five lines |
| `tail -n 5 notes.txt` | Prints the last five lines |
| `tail -f notes.txt` | Prints the end and follows newly appended content; stop with `Ctrl+C` |

These commands display text; they do not edit it. For a large file, `less` is usually easier to read than printing everything with `cat`. In `less`, type `/word` and press Enter to search, then `n` for the next match.

### `grep`: search inside text

```sh
grep -n 'line' notes.txt
grep -in 'first' notes.txt
grep -F 'a.b' notes.txt
```

`grep` normally prints matching lines. `-n` adds line numbers, `-i` ignores letter case, and `-F` treats the search as fixed text rather than a regular expression. Without `-F`, characters such as `.` can have special pattern meanings.

With the example file, the first command prints:

```text
1:First line
2:Second line
```

### `find`: search for files by name

```sh
find . -type f -name '*.txt'
find . -type d -name 'notes'
```

`.` is the starting directory. `-type f` selects regular files; `-type d` selects directories. `find` searches below the starting point as well as directly inside it.

Quote `'*.txt'` so **`find`** receives the pattern. Otherwise, your shell may replace it with filenames before `find` starts.

## 8. Redirection and pipes

Think of a program as having an input tray, an output tray, and an error tray. The usual names are **standard input**, **standard output**, and **standard error**.

The shell can connect these streams to files or other commands:

| Operator | Meaning |
| --- | --- |
| `>` | Write standard output to a file, creating it or replacing its contents |
| `>>` | Append standard output to a file, creating it if missing |
| `<` | Read standard input from a file |
| `2>` | Write standard error to a file, replacing its contents |
| `\|` | Send one command's standard output into the next command's standard input |

```sh
printf 'apple\nbanana\napple\n' > fruit.txt
printf 'pear\n' >> fruit.txt
sort < fruit.txt
sort fruit.txt | uniq
wc -l < fruit.txt
```

`sort` orders lines. `uniq` removes adjacent repeated lines, so sorting first brings equal lines together. The pipeline produces:

```text
apple
banana
pear
```

`wc -l` counts newline characters; for this newline-terminated file it reports four lines. None of these reading commands changes `fruit.txt`.

You can save the pipeline's result:

```sh
sort fruit.txt | uniq > unique-fruit.txt
```

**Watch out for.** Do not write `sort fruit.txt > fruit.txt`. The shell opens and truncates the output file before `sort` reads it. Use a different output filename.

### Run the next command only after success

```sh
mkdir another-demo && cd another-demo
```

`&&` runs the command on the right only if the command on the left succeeds. A program normally reports success with exit status `0`; nonzero values indicate failure or another condition defined by the program. For example, `grep` returns `1` when it finds no matches.

## 9. Permissions and running programs

### Read a permission string

`ls -l` might show a file beginning with:

```text
-rw-r--r--
 │││││││││
 └┬┘└┬┘└┬┘
owner group others
```

The first character describes the file type: `-` means a regular file and `d` means a directory. The remaining characters form three groups: permissions for the owner, the file's group, and other users.

- `r`: read.
- `w`: write.
- `x`: execute a file, or search/traverse a directory.
- `-`: that permission is absent.

For a directory, reading lists names; writing allows changing entries, normally together with execute permission; execute permission lets you access entries by name. Deleting a file depends mainly on its containing directory's permissions, not whether the file itself is writable.

### `chmod`: change permissions

```sh
chmod u+x script.sh
./script.sh
```

This assumes `script.sh` already contains a valid script, usually with an interpreter line such as `#!/bin/sh`. `u+x` adds execute permission for the owner without changing the other permission bits. Execute permission alone does not turn arbitrary text into a valid program.

### Why `./program` has a dot and slash

```sh
g++ -std=c++17 hello.cpp -o hello
./hello
```

This assumes a source file named `hello.cpp` exists. `./hello` explicitly names the executable in the current directory. A bare name such as `hello` is resolved using the shell's command lookup, including the directories in [`PATH`](LinuxEnvironmentVariables.md#7-path-how-the-shell-finds-programs).

### `sudo`: run a command as another user

`sudo` commonly runs an authorised command as root, the administrator account. It may ask for your password; no characters normally appear while you type it.

Use it when an administrative task requires it and you understand the command. Ordinary work inside your own home directory generally does not need `sudo`. Adding it to a failing command can create root-owned files or change system files without fixing the underlying mistake.

## 10. Processes and disk space

A **process** is a running instance of a program. Its **PID** is a process ID.

| Command | What it shows or does |
| --- | --- |
| `ps -ef` | A snapshot of processes across the system |
| `top` | A live view of processes and resource use; press `q` to quit |
| `kill PID` | Sends a termination request to the process with that PID |
| `df -h` | Used and available space on mounted filesystems |
| `du -sh .` | A summary of disk space used by the current directory tree |

In `kill PID`, replace `PID` with the actual number from your process listing. The default signal requests termination; a program can handle or ignore it. Use `Ctrl+C` first when you just want to stop a command running in your terminal.

`df` answers “how full is this filesystem?” and `du` answers “how much space do these files use?” They measure different things, so their totals need not match exactly.

## 11. Getting help and using the terminal

```sh
man ls
ls --help
type cd
command -v g++
```

`man` opens a manual page; press `q` to leave and use `/word` to search. Many external commands accept `--help`, but that convention is not universal. `type` tells you how your shell resolves a name, such as whether it is a builtin, alias, or executable. `command -v` is useful for checking whether a command can be found.

| Key | Usual effect in an interactive Bash or Zsh session |
| --- | --- |
| Tab | Complete a command or filename; exact behaviour depends on configuration |
| Up / Down | Browse command history |
| `Ctrl+C` | Interrupt the foreground command |
| `Ctrl+D` | On an empty input line, signal end of input; may exit the shell |
| `Ctrl+L` | Clear/redraw the visible terminal screen |

`clear` also clears the display. It does not delete files or reliably erase scrollback/history. Shortcuts can differ if your shell uses another keybinding mode or a program handles the keys itself.

## 12. A worked example

Start from your home directory and use a new folder name if `linux-demo` already exists. This example is separate from the earlier practice files:

```sh
cd ~
mkdir linux-demo
cd linux-demo

# Create a file, then append another line.
printf 'Learn paths\nLearn pipes\nPractise C++\n' > tasks.txt
printf 'Learn permissions\n' >> tasks.txt

# Inspect and search it.
pwd
ls -lh
cat tasks.txt
grep -n 'Learn' tasks.txt

# Make a copy, move it, then save a filtered list.
mkdir backup
cp tasks.txt backup/
mv backup/tasks.txt backup/tasks-original.txt
grep 'Learn' tasks.txt > linux-tasks.txt
wc -l < linux-tasks.txt
```

The final command reports `3`. The resulting layout is:

```text
linux-demo/
├── tasks.txt
├── linux-tasks.txt
└── backup/
    └── tasks-original.txt
```

**Walk-through.** `cd` chooses where relative paths begin. `printf` creates the text, and `>>` preserves earlier lines when appending. `cp` creates an independent copy, `mv` renames it, and `grep` selects matching lines into a different file. `#` begins comments in these examples; interactive Zsh needs its `INTERACTIVE_COMMENTS` option enabled to accept comment lines, so skip those lines if necessary.

## 13. Common mistakes, quick reference, and practice

### Common mistakes

| The mistake | The fix |
| --- | --- |
| Using `cd file.txt` to open a file | `cd` needs a directory; use `less file.txt` to read text |
| Expecting `ls /tmp` to move into `/tmp` | Listing and changing directory are separate operations |
| Forgetting quotes around a path containing spaces | Use `"my notes.txt"` |
| Expecting `touch` to empty a file | It updates timestamps; `>` is the operator that truncates |
| Using `>` when you meant to add another line | Use `>>` to append |
| Expecting `rm` to use the desktop Trash | Treat it as direct deletion |
| Typing `program` when the file is in the current directory | Use `./program` |
| Treating “Permission denied” as a reason to always use `sudo` | Check the path, ownership, and required permissions first |

### Quick reference

| Job | Command |
| --- | --- |
| Where am I? | `pwd` |
| What is here, including hidden files? | `ls -la` |
| Go up / go home | `cd ..` / `cd` |
| Make a directory | `mkdir name` |
| Create an empty file if missing | `touch file.txt` |
| Copy / move a file | `cp source destination` / `mv source destination` |
| Remove a file with confirmation | `rm -i file.txt` |
| Remove an empty directory | `rmdir name` |
| Read a file | `less file.txt` |
| Search file contents | `grep -n 'word' file.txt` |
| Find filenames recursively | `find . -type f -name '*.txt'` |
| Count lines | `wc -l < file.txt` |
| Read a manual | `man command` |

### Practice ideas

1. Create a new practice directory with two subdirectories, then use `pwd` after each `cd`.
2. Create a file whose name contains spaces. Print it with `cat` using quotes.
3. Copy that file, rename the copy, and move it into a subdirectory.
4. Write five lines to a file, append a sixth, then search for one word with `grep -n`.
5. Use `find` to locate your text files and a pipeline to sort a list of repeated words into unique lines.

**Rule to remember:** know your current directory, quote paths that need it, and check the destination before overwriting or deleting files.
