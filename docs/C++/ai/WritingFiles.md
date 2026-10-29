# Writing files in C++: text, CSV, and JSON

Use `std::ofstream` to write files. Use its default mode to create or replace a file, and `std::ios::app` to add bytes to its end. Text and CSV support straightforward appends. Adding an element to an existing JSON array usually means reading the array, changing it in memory, and writing it back.

This guide uses **C++17** and compares three approaches:

| Approach | What it prioritises | When to use it |
| --- | --- | --- |
| Easiest | The least typing while showing the operation clearly | Learning and small experiments |
| Fastest approach to consider | Less formatting, fewer small writes, and less repeated work | Large files or frequent writes, after measuring |
| Best default | Correct formatting, checked errors, and maintainable code | Most real programs |

**There is no universally fastest implementation.** File size, formatting, memory use, storage, and the standard library all matter. The performance examples below are useful starting points, not benchmark results. For small files, the easiest checked approach is often the best choice.

## Contents

- [1. File modes and terminology](#1-file-modes-and-terminology)
- [2. Reusable helpers for checked writes](#2-reusable-helpers-for-checked-writes)
- [3. Text files](#3-text-files)
- [4. CSV files](#4-csv-files)
- [5. JSON files](#5-json-files)
- [6. Faster bulk output](#6-faster-bulk-output)
- [7. Safer replacement of important files](#7-safer-replacement-of-important-files)
- [8. Choosing an approach](#8-choosing-an-approach)

## 1. File modes and terminology

**Creating** writes a new file. **Rewriting** in this guide means replacing all its contents. **Updating** means changing selected existing data. **Appending** means preserving existing data and adding more at the end.

```cpp
#include <fstream>

int main() {
    std::ofstream file("notes.txt");
    file << "Hello!\n";
}
```

`std::ofstream` is an output file stream. `<<` sends characters or formatted values into it. Opening `notes.txt` this way creates it if missing and immediately discards its old contents if it exists. The default output mode has this behaviour; you do not have to call `clear()` or delete the file first. See the [C++ output-stream constructor](https://eel.is/c++draft/ofstream.cons) and [file-opening modes](https://eel.is/c++draft/filebuf.members).

| Code | Meaning with `std::ofstream` |
| --- | --- |
| `std::ofstream f("data.txt");` | Create or replace |
| `std::ofstream f("data.txt", std::ios::trunc);` | Explicitly create or replace |
| `std::ofstream f("data.txt", std::ios::app);` | Create if missing; otherwise preserve and append |
| `std::ofstream f("data.txt", std::ios::binary);` | Create or replace, without text-mode newline translation |
| `std::ofstream f("data.txt", std::ios::app \| std::ios::binary);` | Append exact bytes |

Combine flags using `|`. `ofstream` adds the output flag itself. `binary` controls translation; it does **not** turn text into a different file format or guarantee better performance. `app` moves to the end before every write. `ate` only seeks to the end when opening; it is not a substitute for `app`, and `ofstream` with `ate` alone still truncates. See [the C++ open-mode definitions](https://eel.is/c++draft/ios.openmode).

Paths such as `"notes.txt"` are relative to the program's **current working directory**, which may differ from the source-code directory. Streams create files, but do not create missing parent directories.

The stream closes automatically when it leaves scope. However, checking only that opening succeeded is insufficient: writing or closing can fail too. The shortest examples below omit those checks deliberately; the recommended examples use the helpers in section 2.

### Compiling the examples

For a complete program saved as `example.cpp`:

```sh
g++ -std=c++17 -O2 -Wall -Wextra -pedantic example.cpp -o example
./example
```

Blocks containing `main()` are standalone programs. Other C++ blocks are either helper definitions to put above `main()`, or snippets to put inside `main()`; their surrounding text identifies their dependencies. Choose one example at a time rather than pasting every block into one program.

## 2. Reusable helpers for checked writes

Copy these definitions above `main()` when an example uses `write_file()` or `open_output()`:

```cpp
#include <filesystem>
#include <fstream>
#include <string_view>

std::ofstream open_output(const std::filesystem::path& path,
                          std::ios::openmode mode) {
    std::ofstream out;
    out.exceptions(std::ios::failbit | std::ios::badbit);
    out.open(path, mode | std::ios::binary);
    return out;
}

void write_file(const std::filesystem::path& path,
                std::string_view bytes,
                std::ios::openmode mode = std::ios::trunc) {
    auto out = open_output(path, mode);
    out.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
    out.close();
}
```

The exception mask makes opening, writing, and explicit closing report errors by throwing. Calling `close()` here lets the caller observe a failure during the final buffer flush; automatic destruction still handles cleanup if an earlier operation throws. See [C++ stream error flags and exceptions](https://eel.is/c++draft/iostate.flags).

`write()` copies the supplied characters; it does not format numbers or add a newline. Its length has type `std::streamsize`, hence the cast. This small helper assumes the supplied string's size fits that type; use bounded chunks for exceptionally large data. See [C++ unformatted output](https://eel.is/c++draft/ostream.unformatted).

This helper writes exact bytes, so examples explicitly choose `\n` or `\r\n`. It directly truncates on replacement; section 7 covers preserving the old file until the new one is ready.

Use a `try`/`catch` in `main()` to display errors. This is a complete text-file example when placed after the helpers:

```cpp
#include <exception>
#include <iostream>

int main() {
    try {
        write_file("notes.txt", "Hello!\n");
    } catch (const std::exception& error) {
        std::cerr << "Could not save file: " << error.what() << '\n';
        return 1;
    }
}
```

All the recommended snippets below can run inside this same `try` block. Their functions propagate errors to it.

## 3. Text files

### Creating a text file: easiest

Include `<fstream>` and put this inside `main()`:

```cpp
std::ofstream("notes.txt") << "First line\nSecond line\n";
```

This temporary stream lives until the end of the statement, then closes. It is a short valid demonstration, but it gives you no convenient stream variable for checking errors or making later writes.

A named stream is only slightly longer and easier to extend:

```cpp
std::ofstream file("notes.txt");
file << "First line\nSecond line\n";
```

### Rewriting a text file: easiest

```cpp
std::ofstream("notes.txt") << "Replacement contents\n";
```

The operation is identical to creating a file. Running it twice leaves one copy of `Replacement contents`, because the second run replaces the first run's output.

### Appending to a text file: easiest

```cpp
std::ofstream("notes.txt", std::ios::app) << "Another line\n";
```

Running this twice adds two lines. Append mode does not insert separators for you. If an existing file ends with `Hello` and you append `World\n`, the result is `HelloWorld\n`. Either keep files newline-terminated from the start or inspect and repair the final separator before appending a new line.

### Text files: fastest approach to consider

Keep one stream open, use `\n` instead of flushing every line, and write existing blocks of text with `write()`. For many generated lines, accumulate bounded chunks as shown in section 6. Use `trunc` for creation/replacement and `app` for append.

For one small string, the short `<<` example is already reasonable. Building an extra large string solely to make a single write can cost more than it saves.

### Text files: best default

Use the checked helper from section 2. These snippets go inside `main()`:

```cpp
write_file("notes.txt", "Initial contents\n");
```

```cpp
write_file("notes.txt", "Replacement contents\n");
```

```cpp
write_file("notes.txt", "Another line\n", std::ios::app);
```

You keep short call sites while checking failures in one place.

### Updating part of an existing text file

For ordinary variable-length text, read it, change it in memory, then rewrite it. Writing at a byte offset overwrites bytes; it does not insert space or shift later text.

This example replaces the first occurrence of `old` with `replacement`. Put it inside `main()` after the section 2 helpers, and also include `<iterator>`, `<stdexcept>`, and `<string>`:

```cpp
std::string text;
{
    std::ifstream in("notes.txt", std::ios::binary);
    if (!in) throw std::runtime_error("Cannot open notes.txt");
    in.exceptions(std::ios::badbit);
    text.assign(std::istreambuf_iterator<char>(in),
                std::istreambuf_iterator<char>());
}

if (auto pos = text.find("old"); pos != std::string::npos) {
    text.replace(pos, 3, "replacement");
    write_file("notes.txt", text);
}
```

The input stream closes before the output stream opens. For a large file, process it in pieces into a separate file instead of loading everything into memory. For important data, use section 7's replacement pattern.

## 4. CSV files

A CSV file is text organised into records and comma-separated fields:

```csv
name,age
Alice,25
Bob,30
```

The file extension does not generate commas, headers, or escaping. Your code supplies them.

### Creating a CSV file: easiest

Include `<fstream>` and put this inside `main()`:

```cpp
std::ofstream("people.csv") << "name,age\nAlice,25\nBob,30\n";
```

This works for these fixed values. It uses LF line endings, which many CSV readers accept. The recommended helper below uses explicit CRLF record endings.

### Rewriting a CSV file: easiest

```cpp
std::ofstream("people.csv") << "name,age\nAlice,26\nBob,31\n";
```

This replaces the whole file, including its header. To change one existing row, first obtain the existing records, change the desired record, and serialize the resulting collection again.

### Appending to a CSV file: easiest

```cpp
std::ofstream("people.csv", std::ios::app) << "Charlie,22\n";
```

This assumes the existing file has the expected header and ends with a record separator. If it is missing, append mode creates it, but this snippet will contain only `Charlie,22` and no header. Avoid writing the header every time you append.

### Correctly writing CSV fields

Values can contain commas, quotes, and line breaks. Fields containing those characters need double quotes around them, and an embedded double quote becomes two double quotes. Backslash escaping is not CSV escaping. See [RFC 4180's CSV format rules](https://www.rfc-editor.org/rfc/rfc4180.html#section-2).

Examples:

| Actual value | Written CSV field |
| --- | --- |
| `Alice` | `Alice` or `"Alice"` |
| `Smith, Alice` | `"Smith, Alice"` |
| `Sam "Ace" Lee` | `"Sam ""Ace"" Lee"` |

For a small writer, always quoting string fields makes the rule easy to implement. Add these definitions after the section 2 helpers:

```cpp
#include <string>
#include <string_view>
#include <vector>

struct Person {
    std::string name;
    int age;
};

std::string csv_field(std::string_view value) {
    std::string result = "\"";
    for (char c : value) {
        if (c == '"') result += '"';
        result += c;
    }
    result += '"';
    return result;
}

std::string csv_row(const Person& person) {
    return csv_field(person.name) + ',' + std::to_string(person.age) + "\r\n";
}

void save_people(const std::filesystem::path& path,
                 const std::vector<Person>& people) {
    std::string csv = "name,age\r\n";
    for (const auto& person : people) csv += csv_row(person);
    write_file(path, csv);
}
```

These helpers handle CSV quoting and integer formatting. They use CRLF for record endings and binary mode so Windows does not translate an explicit `\r\n` into `\r\r\n`. Supply text in the encoding your reader expects, usually UTF-8. This writer supports commas, quotes, and embedded line breaks in names.

### CSV creation and rewriting: best default

With the preceding helpers, put this inside `main()`:

```cpp
std::vector<Person> people = {{"Alice", 25}, {"Smith, Bob", 30}};
save_people("people.csv", people);  // Create or replace.

people[0].age = 26;
save_people("people.csv", people);  // Rewrite with updated records.
```

This example already has the full collection in memory. If you only have an existing CSV file, use a CSV parser to load it first. Splitting every line on commas is insufficient because quoted commas and embedded newlines are legal. The writer above is not a CSV parser.

### CSV appending: best default

For a file your program owns, add this function after the CSV helpers. Also include `<stdexcept>`:

```cpp
void append_person(const std::filesystem::path& path, const Person& person) {
    const std::string row = csv_row(person);  // Format before opening output.
    const bool has_data = std::filesystem::exists(path)
                       && std::filesystem::file_size(path) != 0;

    std::string prefix;
    if (!has_data) {
        prefix = "name,age\r\n";
    } else {
        std::ifstream in(path, std::ios::binary);
        in.exceptions(std::ios::failbit | std::ios::badbit);
        in.seekg(-1, std::ios::end);
        char last;
        in.get(last);
        if (last == '\r') prefix = "\n";
        else if (last != '\n') prefix = "\r\n";
    }

    write_file(path, prefix + row, std::ios::app);
}
```

Call it inside `main()`:

```cpp
append_person("people.csv", {"Charlie \"CJ\" Smith", 22});
```

It writes a header for a missing or empty file, escapes the name, and supplies a missing final record separator. It assumes existing data is valid CSV with the same columns; it does not validate arbitrary input or repair an incomplete quoted record. It also assumes one writer. If several processes write the file, header detection and appending need coordination.

### CSV files: fastest approach to consider

For creation or replacement, serialize rows into bounded chunks and write those chunks through one open stream. For appending many rows, check the header and final separator once, then keep the append stream open for the entire batch. Calling `append_person()` separately for a million records repeatedly opens and inspects the file.

For example, with a `std::vector<Person> people` already populated, the section 2 helpers and `csv_row()` let you create or replace a CSV in chunks:

```cpp
auto out = open_output("people.csv", std::ios::trunc);
std::string chunk = "name,age\r\n";
chunk.reserve(64 * 1024);

for (const auto& person : people) {
    chunk += csv_row(person);
    if (chunk.size() >= 64 * 1024) {
        out.write(chunk.data(), static_cast<std::streamsize>(chunk.size()));
        chunk.clear();
    }
}

if (!chunk.empty()) {
    out.write(chunk.data(), static_cast<std::streamsize>(chunk.size()));
}
out.close();
```

To append a batch to an **already initialized, newline-terminated CSV**, change `std::ios::trunc` to `std::ios::app` and initialise `chunk` as an empty string. Keep the header out of the appended batch. For a missing, empty, or unterminated file, first apply the initialization/separator logic in `append_person()` once for the whole batch.

Keep CSV escaping even in performance-focused code. When writing many numeric fields, consider `std::to_chars()` to format into a reusable buffer; measure before replacing simpler formatting. See [C++ character conversions](https://eel.is/c++draft/charconv.to.chars).

For small and medium collections, `save_people()` is a good default. For large collections, use the chunking approach in section 6 to limit extra memory.

## 5. JSON files

A normal JSON file contains one JSON value, often an object or array:

```json
[
  {"name": "Alice", "age": 25},
  {"name": "Bob", "age": 30}
]
```

### JSON library setup

The C++17 standard library has file streams but no JSON parser or serializer. These examples use **nlohmann/json**, a convenient choice when ease of use matters. It is not a claim that this library is fastest for every workload.

For a simple local setup, obtain the single-header `json.hpp` from the [official integration instructions](https://json.nlohmann.me/integration/) and place it here:

```text
your-project/
  example.cpp
  third_party/
    nlohmann/
      json.hpp
```

Include it and add a short alias:

```cpp
#include <nlohmann/json.hpp>
using json = nlohmann::json;
```

Compile with the include directory:

```sh
g++ -std=c++17 -O2 -Wall -Wextra -pedantic -I third_party example.cpp -o example
```

No separate JSON library link flag is needed for this header-only setup.

### Creating a JSON file: easiest

For a fixed literal, the least typing needs no JSON library at all:

```cpp
#include <fstream>

int main() {
    std::ofstream("person.json") << R"({"name":"Alice","age":25})";
}
```

`R"(...)"` is a raw string literal: the double quotes inside it do not need backslashes. This writes the supplied text without checking whether it is valid JSON.

For values produced by your program, use the JSON library so strings are escaped correctly. With `<fstream>` and the JSON include/alias, put this inside `main()`:

```cpp
json person = {{"name", "Alice"}, {"age", 25}};
std::ofstream("person.json") << person;
```

Avoid building dynamic JSON by joining raw names into a string. Quotes, backslashes, and control characters need JSON escaping.

### Rewriting a JSON file: easiest

```cpp
json person = {{"name", "Alice"}, {"age", 26}};
std::ofstream("person.json") << person;
```

As with text and CSV, this replaces the entire file. It does not merge with an existing object. If you want to preserve other fields, parse the old object first, change the field, and serialize it again.

### Appending to a JSON array: easiest

Do **not** append a second object after the closing bracket of an existing array. For example, these are not one valid JSON document:

```text
[{"name":"Alice","age":25}]{"name":"Bob","age":30}
```

Instead, read the array, append in memory, and rewrite. With `<fstream>` and the JSON include/alias, this snippet goes inside `main()` and assumes `people.json` exists and contains an array:

```cpp
json people;
{
    std::ifstream in("people.json");
    in >> people;
}
people.push_back({{"name", "Bob"}, {"age", 30}});
std::ofstream("people.json") << people;
```

`push_back()` changes the in-memory array; it does not write to disk. See the [JSON array append API](https://json.nlohmann.me/api/basic_json/push_back/). The short example assumes successful file operations and valid input. Use the checked version below for real work.

### JSON creation and rewriting: best default

Use the section 2 helper and the JSON include/alias. Inside `main()`:

```cpp
json people = json::array({
    {{"name", "Alice"}, {"age", 25}},
    {{"name", "Bob"}, {"age", 30}}
});

write_file("people.json", people.dump(2) + '\n');
```

`dump(2)` produces a string with two-space indentation; `dump()` produces compact JSON. Both are valid. Serialization finishes before `write_file()` opens the destination, so a serialization failure does not truncate the old file. Pretty printing is helpful for files people edit; compact output uses fewer bytes. See [the `dump()` documentation](https://json.nlohmann.me/api/basic_json/dump/).

### Updating and appending JSON: best default

Add this reader after the section 2 helpers and the JSON include/alias. Also include `<stdexcept>`:

```cpp
json read_json(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) throw std::runtime_error("Cannot open JSON file");
    in.exceptions(std::ios::badbit);
    return json::parse(in);
}
```

`json::parse(in)` parses the document and rejects invalid JSON, including extra non-whitespace after the value. An empty file is invalid JSON; it is not an empty array. See [the parsing API](https://json.nlohmann.me/api/basic_json/parse/).

To update an existing object's age, put this inside `main()`:

```cpp
json person = read_json("person.json");
if (!person.is_object()) throw std::runtime_error("Expected a JSON object");
person["age"] = 26;
write_file("person.json", person.dump(2) + '\n');
```

To append an element to an array, add this function above `main()`:

```cpp
void append_json(const std::filesystem::path& path, const json& item) {
    json items = std::filesystem::exists(path)
               ? read_json(path)
               : json::array();

    if (!items.is_array()) {
        throw std::runtime_error("Expected a JSON array");
    }

    items.push_back(item);
    write_file(path, items.dump(2) + '\n');
}
```

Then call it inside `main()`:

```cpp
append_json("people.json", json{{"name", "Charlie"}, {"age", 22}});
```

This creates an array when the file is missing, rejects malformed existing data, checks the top-level type, and writes the updated array. It intentionally rejects an existing empty file rather than silently treating damaged data as no data. For important data, replace its final `write_file()` call with section 7's `replace_file()`.

### JSON files: fastest approach to consider

For creation and full replacement, compact serialization avoids indentation bytes. If the serialized string fits comfortably in memory, pass `dump()` to the bulk-writing helper:

```cpp
write_file("people.json", people.dump());
```

This assumes `people` is already defined. It allocates a string containing the whole document. A compact stream write avoids that full output-string allocation, and may suit a larger document better:

```cpp
auto out = open_output("people.json", std::ios::trunc);
out << people;
out.close();
```

The tradeoff is that serialization now happens after truncation; use a separate temporary file when the old document must survive a serialization failure. Measure both approaches for your data before calling either faster.

For repeated array appends, **read once, add all the items, and save once**. Repeatedly reading and rewriting the growing file can dominate the cost. With this representation, each save reads/writes data proportional to the document size; a sequence of single-item saves can grow quadratically in total work.

For example, using `read_json()` and `write_file()` inside `main()`:

```cpp
json additions = json::array({
    {{"name", "Charlie"}, {"age", 22}},
    {{"name", "Dana"}, {"age", 28}}
});

json people = read_json("people.json");
if (!people.is_array()) throw std::runtime_error("Expected a JSON array");
for (const auto& item : additions) people.push_back(item);
write_file("people.json", people.dump());
```

This assumes the file already exists; use the missing-file initialization from `append_json()` if needed. The same batch technique works when creating a new array: start with `json::array()` instead of reading the file.

An implementation specialised for a controlled array layout can edit the closing bracket and append near the end, but that needs careful validation, positioning, and failure recovery. It is a poor beginner default for arbitrary JSON.

### Frequent structured appends: JSON Lines

If you control the format and need frequent record appends, consider **JSON Lines**, conventionally `.jsonl`: one compact JSON value per line. It supports adding a record without reading and rewriting all prior records. It is a different file format from one JSON array; readers must support it. See [the JSON Lines format](https://jsonlines.org/).

With the section 2 helpers and JSON include/alias, put this inside `main()`:

```cpp
json event = {{"name", "Alice"}, {"age", 25}};
write_file("people.jsonl", event.dump() + '\n', std::ios::app);
```

After a second record, the file could look like:

```jsonl
{"age":25,"name":"Alice"}
{"age":30,"name":"Bob"}
```

Keep each record on one physical line: use `dump()`, not `dump(2)`. Newlines inside JSON strings are escaped by the serializer. Keep the file newline-terminated, as this writer does, so the next record starts on a new line. For an externally supplied file, validate its last record and separator before appending.

To create or rewrite JSON Lines, use `trunc` and write the complete set of serialized lines. For a batch of appends, keep a single append stream open and write multiple records before closing.

## 6. Faster bulk output

The main improvements to try across all three formats are:

1. Open the file once per batch.
2. Avoid flushing after every row or record.
3. Use `write()` for already-formatted blocks.
4. Use bounded chunks when accumulating everything would consume too much memory.
5. Measure formatting time as well as file-writing time.

`std::endl` writes a newline **and flushes the stream**. Use `\n` when you only need the newline; use flushing deliberately when data must become visible promptly. See [the C++ definition of `endl`](https://eel.is/c++draft/ostream.manip). Closing normally flushes the stream buffer, but neither a successful close nor `flush()` alone guarantees survival of a power failure.

Here is a complete bulk-text example using a reusable chunk. Changing `trunc` to `app` makes it append instead:

```cpp
#include <exception>
#include <fstream>
#include <iostream>
#include <string>

int main() {
    try {
        std::ofstream out;
        out.exceptions(std::ios::failbit | std::ios::badbit);
        out.open("numbers.txt", std::ios::binary | std::ios::trunc);

        constexpr std::size_t chunk_size = 64 * 1024;
        std::string chunk;
        chunk.reserve(chunk_size);

        for (int i = 0; i < 100000; ++i) {
            chunk += std::to_string(i);
            chunk += '\n';
            if (chunk.size() >= chunk_size) {
                out.write(chunk.data(),
                          static_cast<std::streamsize>(chunk.size()));
                chunk.clear();
            }
        }

        if (!chunk.empty()) {
            out.write(chunk.data(),
                      static_cast<std::streamsize>(chunk.size()));
        }
        out.close();
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
```

The 64 KiB threshold is a convenient starting point, not an optimal universal buffer size. The actual block can be slightly larger because the threshold is checked after adding a record. Standard file streams already buffer output, so extra chunking is something to test, not a requirement for every file.

For CSV, replace the number-generation step with `chunk += csv_row(person)` and write the header once. A single huge CSV field can still make a chunk huge; truly bounded processing must also handle oversized records in pieces.

For JSON, do not simply concatenate serialized objects into a `.json` file. Preserve the array/object syntax, use the JSON serializer, or deliberately choose JSON Lines.

`std::ios::sync_with_stdio(false)` controls synchronisation of the standard C++ streams with C standard I/O. It is not a general speed switch for independently opened `ofstream` objects. See [the C++ specification](https://eel.is/c++draft/ios.members.static).

When benchmarking, use a release build such as `-O2`, identical data and formatting, and include the final close in the timed work. Keep durability requirements the same: timing writes accepted into operating-system caches is a different measurement from timing durable storage.

## 7. Safer replacement of important files

Direct replacement truncates the old file before all the new data has been written. If the write fails, the result may be incomplete. This affects text, CSV, and JSON equally.

A common approach is to write a separate file in the same directory, close it successfully, then rename it over the destination. Here is a small example using the section 2 helper; also include `<stdexcept>`:

```cpp
void replace_file(const std::filesystem::path& path, std::string_view bytes) {
    auto temporary = path;
    temporary += ".tmp";

    if (std::filesystem::exists(temporary)) {
        throw std::runtime_error("Temporary file already exists");
    }

    write_file(temporary, bytes);
    std::filesystem::rename(temporary, path);
}
```

This example assumes a **trusted directory, one writer, and a reserved `.tmp` filename**. The existence check is not an exclusive-create operation and does not make it safe against competing writers. It refuses to overwrite a pre-existing temporary file; if saving or renaming fails, inspect the temporary file before retrying.

The [C++ filesystem rename operation](https://eel.is/c++draft/fs.op.rename) uses POSIX-style rename semantics, including replacement of an existing non-directory destination. Keeping the temporary file beside the destination avoids moving across filesystems. Atomic replacement depends on the filesystem/platform; open-file sharing rules and permissions can still cause failure. Do not delete the destination first as a fallback, because that removes the old file before replacement succeeds.

For this single-writer setting, calls inside `main()` can be:

```cpp
replace_file("notes.txt", "Replacement contents\n");
```

```cpp
replace_file("people.csv", "name,age\r\n\"Alice\",26\r\n");
```

```cpp
json person = {{"name", "Alice"}, {"age", 26}};
replace_file("person.json", person.dump(2) + '\n');
```

The CSV call uses fixed, already-escaped data. For dynamic CSV records, build the string with `csv_row()` and pass that string instead. You can also change `save_people()` to call `replace_file()` after constructing its CSV output.

For production use with competing writers, use a platform-supported exclusive temporary-file creation method and coordinate updates. Replacement also creates a new filesystem object, so consider permissions, ownership, and links if they matter. To guarantee recovery after power loss, use the platform's durability operations for the file and directory as appropriate; standard C++ stream flushing alone does not provide that guarantee.

## 8. Choosing an approach

| Format and operation | Easiest | Performance approach to try | Best default |
| --- | --- | --- | --- |
| Text: create | `ofstream(path) << text` | One open stream; bulk writes for existing blocks | Checked `write_file()` |
| Text: rewrite | Same as create | Stream transformed data to a separate file for large inputs | Checked rewrite; temporary replacement for important data |
| Text: append | `ofstream(path, ios::app) << text` | One open append stream per batch | Checked append with deliberate line separators |
| CSV: create | Write a fixed comma-separated string | Correctly escaped rows in chunks | `save_people()` |
| CSV: rewrite | Write the complete new CSV | Parse/update once, then serialize once | Rewrite structured records; preserve the old file when needed |
| CSV: append | Append a complete row | Inspect once, append a batch | `append_person()` for small single-writer workloads |
| JSON: create | A fixed raw JSON literal; library for dynamic values | Compact bulk output or compact streaming; measure both | Serialize with a library, then checked save |
| JSON: rewrite | Write a complete new document | Batch changes; serialize once | Parse, modify, and save; temporary replacement for important data |
| JSON: append an array item | Read, `push_back()`, rewrite | Read once and batch additions | Validate the array and save the updated document |
| JSON Lines: append | Compact JSON plus `\n` in append mode | Keep one append stream open | Useful when consumers support JSON Lines and records grow frequently |

For everyday code, start with the checked helpers and correct formatting. Choose temporary-file replacement when losing the previous version would matter. Optimise when measurements show file output is a bottleneck.
