# C++ Strings for Beginners: How They Work and How to Use Them

*A plain-English guide to `std::string`. Examples assume C++17 or newer unless a note says otherwise.*

## How to use this guide

Most string guides jump straight into a list of functions. That makes it easy to copy a line of code and hard to understand why it sometimes crashes. This guide goes the other way around:

- **Part 1** explains what a string actually is and how the computer stores it. Once you have that picture, most of the “weird rules” stop being weird.
- **Part 2** covers the 18 most useful string tools, grouped by what you want to do. Each one has the same four parts: what it does, an example, what to watch out for, and how fast it is.
- **Part 3** puts everything together in one worked example, then gives you a cheat sheet and a list of common mistakes.

If a word is unfamiliar, check the next section first.

## Words you will meet in this guide

- **Character**: one letter, digit, space or symbol, such as `A`, `7` or `!`. In C++ a single character has the type `char`.
- **String**: a sequence of characters, like `Hello`. In C++ the string type is `std::string`.
- **Index (or position)**: the number that says where a character sits in a string. The first character is at index `0`.
- **Function**: a named action you can ask the computer to carry out. A function that belongs to a type is called a **member function** and is used with a dot, like `name.size()`.
- **Argument**: a value you hand to a function inside the brackets. In `text.substr(2, 3)`, the arguments are `2` and `3`.
- **Return value**: what a function hands back to you. `size()` returns a number. Some functions return nothing, which C++ calls `void`.
- **Modify (or change in place)**: to alter the original string. The opposite is to make a **copy**, which leaves the original alone.
- **Literal**: a value typed directly into your code. `"Hello"` is a string literal and `'H'` is a character literal.
- **Unsigned number**: a number that can never be negative. String sizes and positions are unsigned.
- **`size_t` / `size_type`**: the unsigned number type used for sizes and positions. `std::string::size_type` is the exact type for strings.
- **`std::string::npos`**: a special value meaning “no position” or “not found”. Part 2 explains it under `find()`.
- **Exception**: an error C++ can raise while the program runs, such as `std::out_of_range`. If nothing handles it, the program stops.
- **Undefined behaviour**: C++ makes no promise about what happens. The program might crash, print nonsense, or appear to work. The “appears to work” case is the most dangerous, because the bug stays hidden.
- **Memory**: where the computer keeps your data while the program runs. To **allocate** is to ask for some. To **reallocate** is to move to a bigger spot.
- **Buffer**: a block of memory holding a row of items, such as the characters of a string.
- **Reference**: another name for an existing thing rather than a copy of it. `char&` is a reference to a character, so changing it changes the original.
- **`const`**: read-only. A `const` string cannot be changed.
- **Temporary**: a short-lived value that disappears at the end of the line of code that created it.
- **Standard library**: the toolbox that comes with C++. Its names start with `std::`.
- **Big-O, such as O(n)**: a short way of describing how the work grows as the string grows. Section 1.10 explains it in plain words so you don’t have to lean on the symbols.

## Part 1: How strings actually work

### 1.1 A string is a row of numbered lockers

Text is stored as a row of characters, one after another, like a row of numbered lockers where each locker holds exactly one character.

```text
 A    r    c    h    e    r
[0]  [1]  [2]  [3]  [4]  [5]
```

The numbers underneath are **indexes**. They start at `0` because an index really means “how many steps from the start”. The first character is zero steps away, the second is one step away, and so on.

This string has **six** characters, but its largest valid index is **5**. In general:

```cpp
// An index is valid when:
0 <= index && index < text.size()
```

Index `6` is one step past the end. There is no locker there that belongs to your text, and a lot of string bugs come from reaching for it.

### 1.2 What is inside each locker? A number

Computers only store numbers, so every character is stored as a small whole number. The agreed list of which number means which character is called a **character encoding**. For basic English text the list is **ASCII**, where `A` is 65, `B` is 66, `a` is 97 and the digit `7` is 55.

The type that holds one character is `char`. You can see the number behind a letter by converting it to an `int`:

```cpp
char letter = 'A';

std::cout << letter << '\n';       // A
std::cout << int(letter) << '\n';  // 65
```

Notice the quotes. Single quotes make **one character**. Double quotes make **text**:

```cpp
char first = 'A';            // one character
std::string word = "A";      // a string that happens to hold one character
```

They are different types and are not interchangeable. This difference causes a surprising number of beginner errors, and you will meet it again in `push_back()`.

### 1.3 A string is a manager plus a row of lockers

Your program’s memory is like a very long street of numbered mailboxes. A string needs a row of them side by side.

A `std::string` is really two things working together:

1. **A small record** that keeps three notes: where the row of characters is, how many characters are in use (the **size**), and how many lockers are available in total (the **capacity**).
2. **The row of characters itself**, called the **buffer**.

You never manage either part by hand. The string looks after them for you, which is the main reason to prefer `std::string` over older C-style text.

Because the characters sit side by side, two useful things follow:

- **Jumping to any position is instant.** To reach index 4, the computer starts at the beginning and moves 4 lockers along. That is one calculation, with no counting through the earlier characters.
- **Inserting or removing in the middle is slower.** If you add a character at the front, every character after it must shuffle along to make space, like a queue of people each stepping back one place.

### 1.4 The hidden “stop” marker

After the last character, a string keeps one extra locker holding a special value called the **null terminator**, written `'\0'`. It means “the text ends here”. It is **not** counted in `size()`, and you never put it there yourself.

Why does it exist? Older C code stores text as a bare row of characters with no record of the length, so the stop marker is the only way to know where the text ends. A string literal such as `"Hello"` in your code is exactly that kind of C-style text: six lockers holding `H`, `e`, `l`, `l`, `o` and the stop marker. `std::string` is a friendlier wrapper that remembers the length for you.

```cpp
std::string name = "Archer";   // copies the characters into a std::string
```

You can mostly forget the terminator. Remember it for two reasons:

- `size()` does not include it. `"Hello"` has size 5, not 6.
- `text[text.size()]` is legal to *read*, but it gives you the terminator, not a letter.

When an older function wants C-style text, `text.c_str()` hands it one.

### 1.5 Size, capacity and memory: the notebook picture

Three ideas get mixed up all the time, so keep them apart:

- **Size**: how many characters are in the string right now.
- **Capacity**: how many characters fit in the string’s current buffer before it has to find a bigger one.
- **Total memory used**: everything the string costs, including the small record, the buffer, and bookkeeping your compiler adds. You rarely need this number.

Think of a notebook. **Size** is the number of pages you have written on. **Capacity** is the number of pages the notebook has. Capacity can be bigger than size, never smaller.

When you add text and run out of pages, the string does the sensible but slightly costly thing:

1. It gets a bigger notebook (a new, larger buffer).
2. It copies every page across.
3. It throws the old notebook away.

This is called **reallocation**. For a big string it is real work, so strings buy a notebook that is noticeably bigger than they need (many implementations roughly double it). That way you can add many more characters before the next move.

You cannot predict exact capacities, because they depend on your compiler. Never write code that relies on a particular number.

There is one more trick, called **small string optimisation**. For short strings, many implementations keep the characters inside the small record itself, with no separate buffer at all. It is faster because there is nothing extra to fetch. The cutoff is usually somewhere around 15 to 22 characters, but it varies.

```cpp
#include <iostream>
#include <string>

int main() {
    std::string text = "Hello";

    std::cout << text.size() << '\n';      // 5, always
    std::cout << text.capacity() << '\n';  // At least 5; the exact number depends on your compiler
}
```

### 1.6 Strings are copied, not shared

When you assign one string to another, you get a separate copy:

```cpp
std::string a = "Hello";
std::string b = a;     // b gets its own copy of the characters

b[0] = 'J';

// a is still "Hello"
// b is now "Jello"
```

Changing `b` never changes `a`. That is easy to reason about, but copying a long string takes time. So when you hand a string to a function that only needs to look at it, pass it by **const reference**, which means “look at the original, but don’t copy it and don’t change it”:

```cpp
void printName(const std::string& name) {
    std::cout << name << '\n';
}
```

If you are not ready for references yet, ignore this tip. Everything still works. It only affects speed.

### 1.7 Strings can be changed in place

If you know Python or Java, this is a real difference. In those languages a string can never be edited, and every “change” quietly creates a brand-new string. A C++ `std::string` can be edited directly.

So string functions fall into groups, and it helps to know which group each one belongs to:

- **Functions that change the original string**: `push_back`, `pop_back`, `append`, `+=`, `insert`, `erase`, `clear`, and `reserve` (which changes the spare room, not the text).
- **Functions that leave the original alone**: `size`, `length`, `empty`, `capacity`, `find`, `rfind`, `substr` and `+`. They either answer a question or hand you a brand-new string.
- **Functions that give you direct access to a character**: `[]`, `at()`, `front()` and `back()`. Just reading the character changes nothing. Assigning to it changes the string.

### 1.8 The “can’t go negative” trap

`size()` returns an **unsigned** number, one that is never negative. When an unsigned number is asked to go below zero, it does not go negative. It wraps around to the largest value it can hold, like a car odometer rolling backwards from `000000` to `999999`.

```cpp
std::string text;                  // empty, so size() is 0

auto lastIndex = text.size() - 1;  // NOT -1. This becomes a gigantic number.
```

On a typical 64-bit computer that gigantic number is `18446744073709551615`. If you have ever seen it in a string tutorial, this is where it comes from: it is the value of `std::string::npos`, which is defined as “minus one, wrapped around”. That is why you compare against the name `npos` instead of typing the number. The name works on every computer.

The fix for the subtraction trap is to check before subtracting:

```cpp
if (!text.empty()) {
    auto lastIndex = text.size() - 1;
    std::cout << text[lastIndex] << '\n';
}
```

A related warning: if you loop with an `int i` against `text.size()`, your compiler may complain about comparing signed and unsigned numbers. Use `std::string::size_type` (or `size_t`) for indexes, or use a range-based loop that avoids indexes altogether.

### 1.9 A `char` is a byte, not always a letter

English letters, digits and common punctuation each fit in one `char`. Many other characters do not. Accented letters, Chinese or Japanese characters and emoji are usually stored using a system called **UTF-8**, which spends between one and four `char` lockers on a single visible character.

```cpp
// The two bytes that UTF-8 uses to store the letter é
std::string text = "\xC3\xA9";

std::cout << text.size() << '\n';  // 2, even though you see one letter
```

So `size()` counts lockers, not visible letters. The same goes for indexing, cutting with `substr()` and removing with `pop_back()`: they work on lockers, and cutting through the middle of a multi-locker character leaves broken text.

Every example in this guide is safe with plain English text. If your program must handle other languages or emoji, come back to this section and look into a Unicode-aware library when you get there.

### 1.10 Reading “speed” without the jargon

For each function, Part 2 tells you how its running time changes as the string gets longer. Programmers use a notation called Big-O for this. You don’t need the symbols, but you will see them in other guides, so here is the translation. Let **n** be the length of the string you are working on, and **m** the length of whatever you are adding or searching for.

- **Instant (written `O(1)`)**: the work is the same for 5 characters or 5 million.
- **Grows with the length (written `O(n)`)**: double the string and you roughly double the work, like reading a page: a longer page takes longer.
- **Grows with both lengths (written `O(n × m)`)**: the worst case for searching a long text for a long phrase.
- **Amortized instant (written “amortized `O(1)`”)**: usually instant, with an occasional slow step that is rare enough to average out. Adding to the end of a string works like this. Most additions are instant, and now and then the string has to reallocate, which means moving to a bigger notebook.

These costs are typical for common implementations. The C++ standard does not guarantee all of them for every version of every function, so treat them as a practical guide rather than a contract.

## Part 2: The functions

Each function below follows the same pattern: **What it does**, **Example**, **Watch out for** and **Speed**. They are grouped by purpose rather than alphabetically, because that matches how you will go looking for them.

- **Measuring and checking:** `size()`, `length()`, `empty()`
- **Reading characters:** `[]`, `at()`, `front()`, `back()`
- **Searching:** `find()`, `rfind()`
- **Copying a piece:** `substr()`
- **Adding:** `+`, `+=` and `append()`, `push_back()`, `insert()`
- **Removing:** `pop_back()`, `erase()`, `clear()`
- **Managing memory:** `capacity()`, `reserve()`

Every example assumes these two lines at the top of the file:

```cpp
#include <iostream>
#include <string>
```

### Measuring and checking

#### `size()` and `length()`: how many characters?

**What it does.** Both return the number of characters currently in the string. They are two names for the same thing. Use `size()` to match other C++ containers, or `length()` if it reads more naturally. Spelling matters: it is `size()` with a lowercase s, not `Size()`.

**Example.**

```cpp
std::string text = "Basketball";

std::cout << text.size() << '\n';    // 10
std::cout << text.length() << '\n';  // 10
```

The result does not include the hidden terminator from section 1.4.

**Watch out for.**

- `size()` is not `sizeof`. `sizeof(text)` measures the small record from section 1.3, not the characters.
- The result is unsigned, so `text.size() - 1` on an empty string wraps around (section 1.8). Check `empty()` first.
- It counts `char` lockers, not visible letters (section 1.9).

**Speed.** Instant. The string already knows its own size.

#### `empty()`: is there nothing in it?

**What it does.** Returns `true` when the string has zero characters and `false` otherwise. It never changes the string.

**Example.**

```cpp
std::string text;

if (text.empty()) {
    std::cout << "There is no text.\n";
}
```

Writing `!text.empty()` (“not empty”) reads more clearly than `text.size() != 0`. Both are correct. The benefit is clearer wording, not extra speed.

**Watch out for.**

- **Empty is not the same as blank.** A string holding a single space is not empty:

```cpp
std::string a = "";
std::string b = " ";

std::cout << a.empty() << '\n';  // 1 (true)
std::cout << b.empty() << '\n';  // 0 (false)
```

- To reject whitespace-only input, ask “where is the first character that is *not* whitespace?” If there isn’t one, the text is blank (or empty):

```cpp
bool isBlank = text.find_first_not_of(" \t\n\r\f\v") == std::string::npos;
```

- Don’t confuse `empty()` with `clear()`. `empty()` *asks* whether the string is empty. `clear()` *makes* it empty.

**Speed.** Instant.

### Reading characters

#### `[]`: grab the character at a position

**What it does.** `text[i]` gives you the character at index `i`. It hands back the real character, not a copy, so you can read it or change it.

**Example.**

```cpp
std::string text = "Hello";

std::cout << text[1] << '\n';  // e

text[0] = 'Y';                 // changes the string itself
std::cout << text << '\n';     // Yello
```

If you want every character and don’t need the index, a range-based loop is simpler:

```cpp
std::string word = "hello";

for (char ch : word) {
    std::cout << ch << '\n';
}
```

To *change* characters while looping, use a reference (`char&`) so you edit the original instead of a copy:

```cpp
std::string word = "hello";

for (char& ch : word) {
    if (ch == 'l') {
        ch = 'L';
    }
}

// word is now "heLLo"
```

**Watch out for.**

- **`[]` does no safety check.** Asking for `text[100]` on a two-character string is undefined behaviour. It will not raise a friendly error. The program might crash, return garbage, or seem to work. If you want a safety check, use `at()`.
- **The index just past the end is special.** Reading `text[text.size()]` gives you the hidden terminator from section 1.4, not a letter. Writing anything else there is not allowed, and it does **not** make the string longer. To add a character, use `push_back()` or `+=`.
- **Indexes cannot be negative.** They are unsigned (section 1.8).

**Speed.** Instant.

#### `at()`: grab a character, with a safety check

**What it does.** Works like `[]`, but it checks the index first. If the index is too big, it raises an `std::out_of_range` exception instead of silently reading the wrong place. Like `[]`, it hands back the real character, so you can change it.

**Example.**

```cpp
std::string text = "Hello";

std::cout << text.at(1) << '\n';  // e

text.at(0) = 'Y';                 // text is now "Yello"
```

Handling the error uses `try` and `catch`. Read it as “try this; if that raises an `out_of_range` error, run the catch part instead of crashing”:

```cpp
#include <stdexcept>

std::string text = "Cat";

try {
    std::cout << text.at(3) << '\n';   // index 3 does not exist
} catch (const std::out_of_range&) {
    std::cout << "Invalid index.\n";
}
```

**Which one should you use?**

- Use `[]` when your code has already guaranteed the index is valid, such as a loop that stops at `size()`.
- Use `at()` while you are learning, or when the index comes from a calculation you don’t fully trust. A loud error is much easier to debug than a silent wrong answer.
- For input typed by a user, check the number yourself so you can show a helpful message. The negative check matters, because a negative `int` turns into a gigantic unsigned number (section 1.8):

```cpp
int index = -1;

if (index >= 0 &&
    static_cast<std::string::size_type>(index) < text.size()) {
    std::cout << text[static_cast<std::string::size_type>(index)] << '\n';
}
```

(`static_cast<...>(index)` means “convert this number to that type deliberately”.)

**Speed.** Instant when the index is valid. Raising an exception costs more, but it only happens when something has gone wrong.

#### `front()` and `back()`: the first and last character

**What it does.** `front()` is the first character, the same as `text[0]`. `back()` is the last real character, not the hidden terminator. Both hand back the real character, so you can change it.

**Example.**

```cpp
std::string text = "Hello";

std::cout << text.front() << '\n';  // H
std::cout << text.back() << '\n';   // o

text.back() = '!';                  // text is now "Hell!"
```

`back()` says what you mean more clearly than `text[text.size() - 1]`, and it avoids the subtraction trap from section 1.8.

**Watch out for.**

- **Both need at least one character.** Calling them on an empty string is undefined behaviour. They do not raise a neat error. Check first:

```cpp
if (!text.empty() && text.back() == '\n') {
    text.pop_back();   // remove a trailing newline
}
```

The `&&` means “and”. If the left side is false, C++ never looks at the right side, so `back()` is never called on an empty string. The order of the two checks matters.

- **They only look at one character.** To check a whole beginning or ending, such as `//` or `.csv`, C++20 gives you `starts_with()` and `ends_with()`:

```cpp
if (text.starts_with("//")) {
    // the line begins with a comment marker
}

if (filename.ends_with(".csv")) {
    // it is a CSV file
}
```

**Speed.** Instant.

### Searching

#### `find()`: where does this text first appear?

**What it does.** Looks for a smaller piece of text (or a single character) inside a string, starting from the left. It returns the index where the **first** match begins. If there is no match, it returns `std::string::npos`.

**Example.**

```cpp
std::string text = "red blue red";

auto position = text.find("red");

std::cout << position << '\n';   // 0
```

(`auto` just means “let the compiler work out the type”. Here it is `std::string::size_type`. You already met it in section 1.8.)

You can tell `find()` where to start searching by adding a second argument:

```cpp
auto second = text.find("red", 1);   // start looking at index 1

std::cout << second << '\n';         // 9
```

The answer is always measured from the **start of the whole string**, not from where you began searching.

**What is `npos`?** Every number from `0` upwards is a real position, so “not found” needs a value that can never be a real position. C++ uses the largest unsigned number there is (section 1.8) and gives it the name `std::string::npos`. Always compare against the name, never the number:

```cpp
auto position = text.find("green");

if (position == std::string::npos) {
    std::cout << "Not found.\n";
}
```

**Watch out for.**

- **Never use the result as a yes/no value.** This looks natural and is wrong:

```cpp
// WRONG
if (text.find("red")) {
    // ...
}
```

A match at index `0` counts as “false”, because zero means false. And `npos` is not zero, so a failed search counts as “true”. You get the opposite of what you wanted. Write it properly:

```cpp
if (text.find("red") != std::string::npos) {
    std::cout << "Found it.\n";
}
```

- **Check for `npos` before doing arithmetic with the result.** `position + 1` when `position` is `npos` wraps around to zero (section 1.8) and quietly gives you nonsense.
- **Searching is case-sensitive and literal.** `find("hello")` will not match `Hello`. The `.` in `find(".")` means an actual dot, not “any character”. `find()` is not a pattern search.

**Finding every match.** Search again each time, starting just after the previous match:

```cpp
std::string text = "banana";
std::string target = "ana";

if (!target.empty()) {
    auto position = text.find(target);

    while (position != std::string::npos) {
        std::cout << position << '\n';

        position = text.find(target, position + 1);
    }
}

// Prints 1, then 3
```

Moving forward by one step lets matches overlap (the `ana` at index 1 and the `ana` at index 3 share the letter `a`). Moving forward by `target.size()` would skip the overlap and find only index 1. The `!target.empty()` guard is there because an empty target matches everywhere, and a loop that moves forward by zero would never finish.

**If you only need yes or no,** newer C++ has tidier tools:

```cpp
text.contains("ana");     // C++23
text.starts_with("ba");   // C++20
text.ends_with("na");     // C++20
```

**Speed.** Searching for one character grows with the length of the string. Searching for a longer phrase can in the worst case grow with both lengths (n × m), though real searches are usually much faster. Don’t rely on any particular speed guarantee.

#### `rfind()`: where does this text last appear?

**What it does.** The same as `find()`, but it searches from the **right** and gives you the last match.

**Example.**

```cpp
std::string text = "red blue red";

auto first = text.find("red");   // 0
auto last = text.rfind("red");   // 9
```

The answer is still a normal index counted from the left, so `9`, not “three from the end”. If nothing matches, you get `std::string::npos`, and the same safety rules apply.

An optional second argument limits the **latest index where a match may start**:

```cpp
std::string text = "one two one two";

auto position = text.rfind("one", 7);

std::cout << position << '\n';   // 0
```

The matches start at index 0 and index 8. Index 8 is later than the limit of 7, so only the one at 0 qualifies.

**A good use: finding the last separator.**

```cpp
std::string filename = "archive.backup.zip";

auto dot = filename.rfind('.');

if (dot != std::string::npos) {
    std::string extension = filename.substr(dot + 1);

    std::cout << extension << '\n';   // zip
}
```

For real file paths, `std::filesystem::path::extension()` is the better tool, because it handles path rules for you.

**Watch out for.** Don’t confuse `rfind()` with `find_last_of()`:

```cpp
std::string text = "abc";

text.rfind("ba");         // npos: the exact text "ba" does not appear
text.find_last_of("ba");  // 1: the last character that is either b or a
```

`rfind()` looks for a **whole sequence**. `find_last_of()` looks for **any one character from a set**.

**Sister functions you will see again.** These four search by character set rather than by phrase. All of them return `npos` when nothing matches:

- `find_first_of(set)`: the first character that is *in* the set.
- `find_first_not_of(set)`: the first character that is *not* in the set.
- `find_last_of(set)`: the last character that is in the set.
- `find_last_not_of(set)`: the last character that is not in the set.

**Speed.** Same as `find()`: grows with the length of the string, and a long phrase can be slower in the worst case.

### Copying a piece

#### `substr()`: copy a section into a new string

**What it does.** `text.substr(start, count)` copies `count` characters beginning at index `start` into a **new** string. The second argument is a **count**, not an end position. Spelling matters: it is `substr`, not `strName(startIndex, length)`.

**Example.**

```cpp
std::string text = "Basketball";

std::string part = text.substr(0, 6);

// part is "Basket"
// text is still "Basketball"
```

```cpp
std::string text = "ABCDEFG";

std::string part = text.substr(2, 3);

// "CDE": start at index 2 and take 3 characters
```

Leave out the count to copy everything from the start position to the end:

```cpp
std::string part = text.substr(3);   // "DEFG"
```

The result is its own string. Changing it never changes the original:

```cpp
std::string original = "Hello";
std::string copy = original.substr(0, 2);

copy[0] = 'Y';

// original is "Hello"
// copy is "Ye"
```

**Edge cases.**

```cpp
std::string text = "Hello";

text.substr(2, 100);   // "llo": a too-large count is trimmed to what is left
text.substr(5);        // "": starting exactly at the end is allowed
text.substr(6);        // raises std::out_of_range: starting past the end is not
text.substr(0, 0);     // ""
```

**A good use: cutting text out after a search.**

```cpp
std::string filename = "report.txt";

auto dot = filename.find('.');

if (dot != std::string::npos) {
    std::string extension = filename.substr(dot + 1);   // "txt"
}
```

**A more advanced option: `std::string_view`.** A *view* is a window onto text that already exists. It copies nothing, so it is instant, but it does not own the characters. The original string must stay alive and must not be reallocated while the view is in use.

```cpp
#include <string_view>

std::string text = "Basketball";

std::string_view part = std::string_view(text).substr(0, 6);   // "Basket", no copy
```

The next line looks similar but is a bug, because `text.substr(0, 6)` makes a temporary string that is destroyed at the end of the line, leaving the view pointing at nothing:

```cpp
// WRONG
std::string_view part = text.substr(0, 6);
```

If that feels like too much for now, ignore views and use `substr()`. It is always safe.

**Speed.** Grows with the number of characters copied. Copying 6 characters is quick, and copying 6 million takes longer.

### Adding

#### `+`: join strings into a new string

**What it does.** Joins pieces of text together and gives you a **new** string. The strings you joined are left untouched.

**Example.**

```cpp
std::string first = "Hello";
std::string second = "world";

std::string result = first + " " + second;   // "Hello world"

// first is still "Hello"
// second is still "world"
```

**Watch out for.**

- **`+` does not change the string on its left.** This line builds a new string and then throws it away:

```cpp
std::string text = "Hello";

text + "!";                  // builds "Hello!" and discards it
std::cout << text << '\n';   // Hello

text += "!";                 // this one really changes text
```

- **You cannot add two string literals.** Both sides are C-style text (section 1.4), and C-style text has no `+`:

```cpp
// WRONG
// auto text = "Hello" + "world";
```

Fix it by writing the literals side by side with nothing between them, or by making one side a `std::string`:

```cpp
std::string a = "Hello" "world";             // joined by the compiler
std::string b = std::string("Hello") + "world";
```

- **Numbers do not turn into digits by themselves.** Convert them first with `std::to_string()`:

```cpp
int score = 42;

std::string message = "Score: " + std::to_string(score);   // "Score: 42"
```

The tempting version `"Score: " + 2` compiles but does something strange. Adding a number to a literal means “skip that many lockers”, so you end up with the text ` ore:  ` instead of `Score: 2`.

- **Rebuilding a string in a loop is slow.** This copies everything collected so far on every pass:

```cpp
for (const auto& piece : pieces) {
    result = result + piece;   // slow for many pieces
}
```

If you add one character at a time this way, the copying adds up as 1 + 2 + 3 + … + n, which programmers call **quadratic** growth: ten times as many pieces means roughly a hundred times as much work. Use `+=` or `append()` for building a string bit by bit.

**Speed.** Grows with the combined length of both strings, because a new string of that size has to be made and filled.

#### `+=` and `append()`: add onto the end of the original

**What it does.** Both add text to the end of an existing string and **change that string**.

**Example.**

```cpp
std::string text = "Hello";

text += " world";      // "Hello world"
text += '!';           // "Hello world!"
text.append(" Bye");   // "Hello world! Bye"
```

For everyday adding, `+=` is the one to reach for. `append()` earns its place because it has extra forms:

```cpp
std::string text = "Hi";
std::string source = "ABCDE";

text.append(" there");       // adds " there"
text.append(3, '!');         // adds "!!!"
text.append(source, 1, 3);   // adds "BCD": 3 characters of source, starting at index 1
```

`append()` hands back the string it just changed, so you can chain calls:

```cpp
std::string text;

text.append("Hello").append(" ").append("world");   // "Hello world"
```

The three-argument form also avoids making a temporary copy. `result.append(source, 10, 20)` does the same job as `result += source.substr(10, 20)` without the extra string in between.

**Watch out for.**

- **`+=` works with a string, a literal or a single character, but not a number.** `text += 5` adds the character with code 5, which you cannot see, not the digit `5`. Use `std::to_string()`.
- **C-style text stops at the terminator.** This matters only if your text contains a zero character:

```cpp
std::string a;
a.append("A\0B");
std::cout << a.size() << '\n';   // 1, because it stops at the \0

std::string b;
b.append("A\0B", 3);             // "take exactly 3 characters"
std::cout << b.size() << '\n';   // 3
```

**Speed.** If the string has spare room (section 1.5), the work grows only with the length of the text you add. If it has to reallocate first, it also copies the old contents. Because strings grow in big steps, adding to the end is cheap on average.

#### `push_back()`: add one character

**What it does.** Adds a single character to the end. It changes the string and returns nothing.

**Example.**

```cpp
std::string text = "Hello";

text.push_back('!');   // "Hello!"
```

It takes a **character**, so it needs single quotes. This is the single-quote versus double-quote difference from section 1.2:

```cpp
text.push_back('A');     // correct: one character

// WRONG: "A" is text, not a character
// text.push_back("A");
```

It shines when you build a string one character at a time:

```cpp
std::string input = "a1b2c3";
std::string digits;

for (char ch : input) {
    if (ch >= '0' && ch <= '9') {
        digits.push_back(ch);
    }
}

// digits is "123"
```

(`ch >= '0' && ch <= '9'` works because the codes for the digits sit next to each other in the character list.)

`digits += '4'` does exactly the same thing as `digits.push_back('4')`. Use whichever reads better to you.

**Watch out for.** It never converts numbers into written digits. `text.push_back(65)` adds the character with code 65, which is `A`, not the text `65`. For the text, use `text += std::to_string(65)`.

**Speed.** Amortized instant (section 1.10). Most calls take no time at all. Now and then the string runs out of room and has to reallocate, which is slower, but that is rare enough to average out.

#### `insert()`: put text into the middle

**What it does.** Puts new text **before** the character currently at the index you give. It changes the string.

**Example.**

```cpp
std::string text = "Hello world";

text.insert(6, "beautiful ");   // "Hello beautiful world"
```

Index 6 is the `w`, so the new text lands in front of it.

Other forms:

```cpp
std::string text = "Hello";

text.insert(0, "Say: ");         // at the start: "Say: Hello"
text.insert(text.size(), "!");   // at the very end: "Say: Hello!"
text.insert(0, 3, '*');          // three asterisks at the start: "***Say: Hello!"
```

Valid positions run from `0` to `size()`. A position past the end raises `std::out_of_range`.

**Watch out for.**

- **Inserting does not overwrite anything.** The old characters shuffle along to make room:

```cpp
std::string text = "ABCDE";

text.insert(2, "XX");   // "ABXXCDE"
```

To *replace* characters, use `replace(start, count, newText)`, which removes `count` characters at `start` and puts the new text in their place:

```cpp
std::string text = "ABCDE";

text.replace(2, 2, "XX");   // "ABXXE"
```

To replace a single character, assigning is simpler: `text[2] = 'X';`

- **Inserting near the front is slow on long strings.** Every character after the insertion point has to shuffle along (section 1.3). Doing it over and over, like `result.insert(0, piece)` in a loop, makes the total work grow quickly. If you want a reversed string, build it normally and then call `std::reverse(text.begin(), text.end())` from `<algorithm>`. That works on plain English text. Reversing text with multi-byte characters needs more care (section 1.9).

**Speed.** Grows with the number of characters that have to move (everything after the insertion point) plus the length of the new text. Inserting at the end is cheap. Inserting at the front of a long string is the slowest case.

### Removing

#### `pop_back()`: remove the last character

**What it does.** Removes the final character. It changes the string and returns nothing.

**Example.**

```cpp
std::string text = "Hello!";

text.pop_back();   // "Hello"
```

Because it returns nothing, you cannot grab the removed character from it:

```cpp
// WRONG
// char removed = text.pop_back();
```

If you want to keep the character, read it with `back()` first:

```cpp
if (!text.empty()) {
    char removed = text.back();
    text.pop_back();

    std::cout << removed << '\n';
}
```

A common use is **undoing** the last thing you added. This appears in *backtracking*, a technique where you try one option, then undo it and try the next:

```cpp
current.push_back('A');   // try adding an A

// ... explore what happens with that choice ...

current.pop_back();       // undo it, back to the earlier text
```

**Watch out for.**

- **Never call it on an empty string.** That is undefined behaviour. Check `empty()` first.
- **It removes one character, not several.** To drop a known number of characters from the end, `resize(newSize)` chops the string down to the size you give it:

```cpp
std::string filename = "report.txt";

if (filename.size() >= 4) {
    filename.resize(filename.size() - 4);   // "report"
}
```

This removes four characters whatever they are. If their content matters, check the ending first (for example with `ends_with()`).

- **It removes one `char`, not one visible letter.** With multi-byte characters (section 1.9), one `pop_back()` can leave a broken character behind.

**Speed.** Instant.

#### `erase()`: remove a section

**What it does.** `text.erase(start, count)` removes `count` characters beginning at index `start`. The second argument is how many to remove, not an end position. Spelling matters: it is `erase`, not `earse`. It changes the string.

**Example.**

```cpp
std::string text = "Hello beautiful world";

text.erase(6, 10);   // "Hello world"
```

```cpp
std::string text = "ABCDEFG";

text.erase(2, 3);    // "ABFG": removed C, D and E
```

Leave out the count to remove everything from `start` to the end:

```cpp
std::string text = "report.txt";

text.erase(6);       // "report"
```

**Edge cases.**

```cpp
std::string text = "Hello";

text.erase(text.size(), 3);   // allowed: nothing there to remove
text.erase(2, 100);           // allowed: removes everything from index 2 onwards

// text.erase(6);             // raises std::out_of_range: starts past the end
```

**A good use: remove known text.** Search first, check the result, then erase:

```cpp
std::string text = "Hello amazing world";
std::string target = "amazing ";

auto position = text.find(target);

if (position != std::string::npos) {
    text.erase(position, target.size());   // "Hello world"
}
```

**Watch out for.**

- **Erasing inside an index loop skips characters.** After an erase, the next character slides into the index you just handled, and then `++i` steps right past it:

```cpp
// Bug-prone: may skip characters
for (std::string::size_type i = 0; i < text.size(); ++i) {
    if (text[i] == ' ') {
        text.erase(i, 1);
    }
}
```

To remove every copy of one character, C++20 has a one-line tool:

```cpp
std::erase(text, ' ');
```

In C++17, use the *erase-remove* pattern. `std::remove` shuffles the characters you want to keep to the front and tells you where the leftover junk begins. Then `erase` chops the junk off the end in one go:

```cpp
#include <algorithm>

text.erase(std::remove(text.begin(), text.end(), ' '), text.end());
```

Both versions are faster as well as safer, because they don’t shuffle the whole tail for every single removed character.

**Speed.** Grows with the number of characters after the removed section, because they all slide left to close the gap. Removing from the very end is cheap.

#### `clear()`: remove everything

**What it does.** Empties the string. It changes the string and returns nothing. The string is still perfectly usable afterwards.

**Example.**

```cpp
std::string text = "Hello";

text.clear();

std::cout << text.size() << '\n';    // 0
std::cout << text.empty() << '\n';   // 1 (true)

text = "New contents";               // still usable
```

**A good use: reusing one string again and again.**

```cpp
std::string buffer;
buffer.reserve(1000);

for (int i = 0; i < 10; ++i) {
    buffer.clear();

    buffer += "Item ";
    buffer += std::to_string(i);

    std::cout << buffer << '\n';
}
```

In practice, common implementations keep the string’s capacity after `clear()`, so the notebook from section 1.5 is not thrown away. That makes reuse cheap.

**Watch out for.**

- **`clear()` is not a memory-release command.** To *request* that spare room be given back, follow it with `shrink_to_fit()`. It is only a request, and the compiler is allowed to ignore it:

```cpp
text.clear();
text.shrink_to_fit();
```

- **`clear()` does not securely wipe anything.** The old characters may still sit in memory. If a program holds a password, emptying the string is not the same as erasing the bytes.

**Speed.** In practice, instant. It does not walk over the old characters.

### Managing memory

#### `capacity()`: how much room is there?

**What it does.** Returns how many characters the current buffer can hold before the string has to reallocate. Capacity is never smaller than size (section 1.5). It does not change anything.

**Example.**

```cpp
std::string text = "Hello";

std::cout << text.size() << '\n';       // 5
std::cout << text.capacity() << '\n';   // at least 5; exact value depends on your compiler
```

Capacity is handy when you are curious about how a string grows. This program prints a message each time the string has to move to a bigger buffer:

```cpp
std::string text;
auto previousCapacity = text.capacity();

for (int i = 0; i < 100; ++i) {
    text.push_back('x');

    if (text.capacity() != previousCapacity) {
        std::cout << "Capacity changed to " << text.capacity() << '\n';

        previousCapacity = text.capacity();
    }
}
```

Run it and you will see only a handful of messages for 100 additions. That is the “buy a much bigger notebook” habit at work. The exact numbers depend on your compiler.

**Watch out for.**

- **Don’t expect a particular number.** Capacity might be 5, 15, 22 or something else, and it can change between compilers.
- **Capacity is not size.** Never use it to decide whether an index is valid. Spare room is not characters:

```cpp
std::string text = "Hi";
text.reserve(100);

// WRONG: there are still only 2 characters
// text[50] = 'X';
```

- **It counts characters, not total memory used** (section 1.5).

**Speed.** Instant.

#### `reserve()`: ask for room in advance

**What it does.** `text.reserve(n)` asks the string to make room for at least `n` characters **now**, so it won’t have to keep reallocating later. It does not add any characters and does not change the text or the size. Spelling matters: it needs a number inside the brackets.

**Example.**

```cpp
std::string result;
result.reserve(1000);
```

```cpp
std::string text = "Hi";
text.reserve(100);

std::cout << text << '\n';          // Hi
std::cout << text.size() << '\n';   // 2, unchanged
```

**A good use: you roughly know the final length.**

```cpp
std::string first = "Archer";
std::string last = "Lee";

std::string fullName;
fullName.reserve(first.size() + 1 + last.size());   // first name, a space, last name

fullName += first;
fullName += ' ';
fullName += last;
```

For short names like this it makes no noticeable difference. It starts to matter when you build a large piece of output from many small pieces.

**`reserve()` versus `resize()`.** They sound alike and do different jobs. In notebook terms, `reserve()` buys extra pages without writing on them, and `resize()` actually fills pages:

```cpp
std::string a;
a.reserve(5);        // still empty, but with room for 5 characters

std::string b;
b.resize(5, '?');    // really contains "?????"
```

**Watch out for.**

- **Don’t reserve one extra locker before every addition.**

```cpp
// Avoid this pattern
for (char ch : input) {
    result.reserve(result.size() + 1);
    result.push_back(ch);
}
```

Repeated tiny reservations can defeat the string’s own habit of growing in big steps. Reserve once with a sensible estimate, or let the string grow by itself.

- **Asking for less than the current capacity does nothing** in C++20 and later. (Before C++20 it was a non-binding request to shrink.)
- **Silly requests raise errors.** Asking for more than the string could ever hold raises `std::length_error`, and running out of memory raises `std::bad_alloc`.

**Speed.** Instant if there is already enough room. Otherwise the string has to move to a bigger buffer, which grows with its current length. During that move the old and new buffers briefly exist side by side, so it briefly needs extra memory.

## Part 3: Putting it together

### A worked example

This program turns the messy text `   player:Archer_Lee;   ` into the clean message `Hello, Archer Lee!`. Read the comments first, then the walk-through underneath.

```cpp
#include <iostream>
#include <string>

int main() {
    std::string input = "   player:Archer_Lee;   ";

    // Step 1: remove spaces from the front
    auto first = input.find_first_not_of(' ');

    if (first == std::string::npos) {
        std::cout << "Input is blank.\n";
        return 0;
    }

    input.erase(0, first);

    // Step 2: remove spaces from the back
    auto last = input.find_last_not_of(' ');
    input.erase(last + 1);

    // Step 3: find the separator
    auto colon = input.find(':');

    if (colon == std::string::npos) {
        std::cout << "Missing ':' separator.\n";
        return 0;
    }

    // Step 4: copy everything after the colon into a new string
    std::string name = input.substr(colon + 1);

    // Step 5: remove an optional semicolon at the end
    if (!name.empty() && name.back() == ';') {
        name.pop_back();
    }

    if (name.empty()) {
        std::cout << "Missing player name.\n";
        return 0;
    }

    // Step 6: replace underscores with spaces
    for (char& ch : name) {
        if (ch == '_') {
            ch = ' ';
        }
    }

    // Step 7: build the message
    std::string message;
    message.reserve(7 + name.size() + 1);   // "Hello, " is 7 characters, plus the name, plus "!"

    message.append("Hello, ");
    message += name;
    message.push_back('!');

    std::cout << message << '\n';   // Hello, Archer Lee!
}
```

**Walk-through.**

1. `find_first_not_of(' ')` finds the first character that is not a space. If there isn’t one, the input is blank, so the program stops early. Then `erase(0, first)` removes the leading spaces.
2. `find_last_not_of(' ')` finds the last real character. Everything after it is trailing space, so `erase(last + 1)` removes it all.
3. `find(':')` locates the separator. The result is checked against `npos` before it is used.
4. `substr(colon + 1)` copies everything after the colon into a brand-new string. `input` itself is untouched.
5. `back()` peeks at the final character, but only after `!name.empty()` has proved there is one. `pop_back()` then removes the semicolon.
6. `for (char& ch : name)` uses a reference so that changing `ch` changes the real string.
7. `reserve()` prepares room once. `append()`, `+=` and `push_back()` then build the message with no wasted copying.

Notice the habits that keep this safe: every search result is checked against `npos` before it is used as a position, `empty()` is checked before `back()` and `pop_back()`, and a reference is used when the goal is to change characters in place.

### Coming from Python or Java?

If you already know one of those languages, this table maps the everyday jobs across:

| Task | C++ | Python | Java |
| --- | --- | --- | --- |
| Number of characters | `text.size()` | `len(text)` | `text.length()` |
| Is it empty? | `text.empty()` | `len(text) == 0` | `text.isEmpty()` |
| Character at a position | `text[i]` or `text.at(i)` | `text[i]` | `text.charAt(i)` |
| Last character | `text.back()` | `text[-1]` | `text.charAt(text.length() - 1)` |
| Copy a piece | `text.substr(start, count)` | `text[start:end]` | `text.substring(start, end)` |
| Find the first match | `text.find("x")` | `text.find("x")` | `text.indexOf("x")` |
| Find the last match | `text.rfind("x")` | `text.rfind("x")` | `text.lastIndexOf("x")` |
| “Not found” result | `std::string::npos` | `-1` | `-1` |
| Add to the end | `text += "x"` (changes the string) | `text += "x"` (makes a new string) | `text += "x"` (makes a new string) |
| Can you edit characters in place? | Yes | No | No |

Three differences catch people out:

- **`substr()` takes a count, not an end position.** Python slices and Java’s `substring()` both take an end position.
- **There are no negative indexes in C++.** `text[-1]` is a bug, not “the last character”. Use `back()`.
- **`[]` never raises an error for a bad index.** Python and Java both do. The C++ equivalent of their behaviour is `at()`.

### Cheat sheet

| Function | Changes the string? | What you get back | What goes wrong if misused | Speed in plain words |
| --- | --- | --- | --- | --- |
| `size()`, `length()` | No | The number of characters | Nothing (but the number is unsigned) | Instant |
| `empty()` | No | `true` or `false` | Nothing | Instant |
| `capacity()` | No | The room available | Nothing | Instant |
| `reserve(n)` | Only the spare room | Nothing | An error if `n` is absurdly large | Instant if there is room, otherwise grows with length |
| `a + b` | No, makes a new string | The new string | Compile error if both sides are literals | Grows with combined length |
| `+=`, `append()` | Yes | The same string | Nothing | Grows with the added length |
| `[]` | Only if you assign | The character | Undefined behaviour if out of range | Instant |
| `at()` | Only if you assign | The character | `std::out_of_range` exception | Instant |
| `front()`, `back()` | Only if you assign | The character | Undefined behaviour if empty | Instant |
| `substr(start, count)` | No, makes a new string | The new string | `std::out_of_range` if `start` is past the end | Grows with characters copied |
| `find()`, `rfind()` | No | An index, or `npos` | Using the result without checking `npos` | Grows with length |
| `push_back(c)` | Yes | Nothing | Passing text instead of a character | Amortized instant |
| `pop_back()` | Yes | Nothing | Undefined behaviour if empty | Instant |
| `insert()` | Yes | The same string | `std::out_of_range` if the index is past the end | Grows with characters moved |
| `erase()` | Yes | The same string | `std::out_of_range` if `start` is past the end | Grows with characters moved |
| `clear()` | Yes | Nothing | Nothing | Instant in practice |

### Common mistakes

| The mistake | The fix |
| --- | --- |
| `text.Size()` | It is lowercase: `text.size()` |
| `text.earse(2, 3)` | It is spelled `erase` |
| `text(2, 3)` to copy a piece | Use `text.substr(2, 3)` |
| `text.reserve()` with nothing inside | Give it a number: `text.reserve(100)` |
| Comparing a search result with `18446744073709551615` | Compare with `std::string::npos` |
| `if (text.find("x"))` | Write `if (text.find("x") != std::string::npos)` |
| `text.size() - 1` on a string that might be empty | Check `!text.empty()` first |
| `"Hello" + "world"` | Make one side a `std::string` |
| `text.push_back("A")` | Use single quotes: `text.push_back('A')` |
| Expecting `[]` to raise an error | Use `at()` if you want a checked access |
| `text + "!"` and expecting `text` to change | Use `text += "!"` |
| Using `capacity()` as the number of characters | Use `size()`. Capacity is spare room, not characters |
| Treating `capacity()` as total memory used | It counts characters only |
| Assuming `size()` counts visible letters | It counts `char` lockers (section 1.9) |

### Five habits that prevent most string bugs

1. **Use `size()` for “how many” and `capacity()` for planning.** Reserved room does not create characters.
2. **Check every `find()` and `rfind()` result against `std::string::npos`** before using it as a position or doing arithmetic with it.
3. **Check `empty()` before `front()`, `back()`, `pop_back()` or `size() - 1`.**
4. **Don’t hold on to a reference or a view across a change that might reallocate.** If you saved a reference and then added a lot of text, the string may have moved to a new notebook, and the reference still points at the old one that was thrown away:

```cpp
std::string text = "Hi";
char& first = text[0];   // refers to a locker in the current buffer

text += "a very long piece of text that probably forces a reallocation";

// DANGEROUS: first may now point at the old, discarded buffer
// first = 'J';
```

After changing a string, fetch references again with `text[0]` rather than trusting old ones.

5. **Remember that a `char` is a byte.** Indexing, cutting and removing work on bytes, which is fine for plain English text and risky for anything else.

### Practice ideas

The fastest way to make this stick is to write small programs. Try these in order:

1. Count the vowels in a string, using a loop and `[]`.
2. Check whether a word is a palindrome (reads the same backwards), using `front()` and `back()` or two indexes.
3. Remove every space from a sentence, first with a loop and `push_back()`, then with the erase-remove pattern.
4. Split `key=value` into two strings, using `find()` and `substr()`.
5. Write a function that trims spaces from both ends of a string, using `find_first_not_of()`, `find_last_not_of()` and `erase()`.

Once you are comfortable, good next topics are `std::getline` (reading a line of input into a string), `std::stoi` (turning text into a number), `std::string_view`, and `std::stringstream` (building and taking apart text like a stream).
