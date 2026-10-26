# `std::vector` in C++: a beginner's guide

*Examples use C++17 or newer. For short snippets, put any `#include` lines at the top of your source file and the other lines inside a function. The worked example near the end is a complete program. If a snippet uses `std::cout` to print, include `<iostream>`.*

## 1. What is a vector?

A **vector** is a list of values that can grow or shrink while your program runs. All its values have the same **type**. Each value in the vector is called an **element**.

```cpp
#include <vector>

std::vector<int> scores{10, 20, 30};
```

Read this as “a vector of `int` values named `scores`.” `<vector>` supplies the type. `std::` means it comes from the C++ standard library. The angle brackets choose the element type, and the braces give the starting elements.

Each element has an **index**, starting at zero:

```text
Element:  10   20   30
Index:     0    1    2
```

So `scores[0]` is `10`, and `scores[2]` is `30`. The vector's **size** is 3. Index 3 does not hold an element yet.

A vector remembers its size, manages its own memory, supports fast access by index, and works with standard library algorithms. It is a good first choice for most ordinary lists of values.

## 2. A few words used below

- A **member function** is an action called with a dot, such as `scores.push_back(40)`. The value in parentheses is an **argument**.
- A **return value** is what a function gives back. `scores.size()` gives back a number; `scores.pop_back()` gives back nothing.
- A **reference** is another name for an existing value. In `int& item`, changing `item` changes the original `int`.
- A **pointer** stores a memory address, such as the address of an element. If that element moves, an old pointer to it is no longer safe to use.
- An **iterator** marks a position in a collection. `v.begin()` marks a vector's first element; `v.end()` marks the position just past its last element. That last position is a boundary, not an element.
- An **allocation** asks the computer for a block of memory. A **reallocation** replaces that block with a new one, often because the old one is full.
- An **exception** is an error a function reports while the program runs. `at()` can throw `std::out_of_range`; an uncaught exception normally ends the program.
- **Undefined behavior** means C++ makes no promise about the result. An invalid `v[index]` might appear to work, crash, or corrupt data.

## 3. Creating vectors

```cpp
std::vector<int> empty;            // No elements
std::vector<int> values{1, 2, 3};  // Elements 1, 2, 3
std::vector<int> zeros(4);         // Four ints, each 0
std::vector<int> sevens(4, 7);     // Four ints, each 7
```

Braces here list the *elements*. Parentheses give a *number of elements*, optionally followed by the value to put in each one. This is easy to miss:

```cpp
std::vector<int> a{4, 7};  // Two elements: 4, 7
std::vector<int> b(4, 7);  // Four elements: 7, 7, 7, 7
```

The element type need not be `int`. For example, `std::vector<std::string>` holds strings; include `<string>` as well as `<vector>`.

Unlike a fixed-size array, a vector can use a count known only while the program is running:

```cpp
int numberOfPlayers = 5;
std::vector<int> points(numberOfPlayers); // Five zero-initialized ints
```

## 4. How size, capacity, and memory work

Imagine a row of boxes. **Size** counts boxes that contain elements. **Capacity** counts boxes the vector has room for before it must get a bigger row.

```text
size = 3, capacity = 5

[10][20][30][  ][  ]
 0   1   2   extra room
```

The empty boxes are *not* elements. You may read indices 0, 1, and 2, but not index 3. Capacity is always at least size; its exact value is chosen by the implementation.

```cpp
std::vector<int> v{10, 20, 30};
v.reserve(5);    // Ask for capacity of at least 5; size is still 3
v.push_back(40); // Now size is 4
```

Elements are stored **contiguously**: side by side in memory. This makes indexing fast and looping through a vector usually efficient.

When a vector runs out of capacity, it generally allocates a larger block, moves or copies its existing elements into it, and releases the old block. The standard does not promise an exact growth factor. Spare capacity makes it possible to add many elements without reallocating each time.

The vector object also needs a little bookkeeping memory. Roughly, its element storage uses `capacity() * sizeof(T)` bytes for a `std::vector<T>`, plus allocation overhead. This excludes memory separately owned by an element, such as a long string. For example, if an `int` takes 4 bytes and capacity is 8, the element storage alone is about 32 bytes.

### `reserve()` and `resize()` are different

```cpp
std::vector<int> v{1, 2, 3};

v.reserve(10); // At least 10 slots of capacity; size remains 3
// v[7] = 99;  // Wrong: element 7 does not exist

v.resize(5);  // Size becomes 5: 1, 2, 3, 0, 0
v[4] = 99;    // Valid: element 4 now exists
```

Use `reserve(n)` when you expect to **add** about `n` elements and want to avoid repeated reallocations. Use `resize(n)` when you want the vector to **contain** `n` elements now. Calling `reserve()` before every addition can make growth slower; normally call it once if you have a useful estimate.

## 5. Reading elements

### `[]` and `at()`: get an element by index

```cpp
std::vector<int> v{10, 20, 30};

int first = v[0];   // 10
v[1] = 25;          // Vector is now 10, 25, 30
int last = v.at(2); // 30
```

Both take constant time: neither needs to scan earlier elements. The difference is what happens for a bad index:

- `v[3]` does **not** check the index. Here it is invalid and causes undefined behavior.
- `v.at(3)` checks and throws `std::out_of_range`. The check has a small cost, but the operation is still constant time.

For a nonempty vector, valid indices go from `0` to `v.size() - 1`. An empty vector has no valid index. `size()` returns an unsigned number, so do not use `v.size() - 1` when `v` is empty.

### `front()` and `back()`: first and last

```cpp
std::vector<int> v{10, 20, 30};
v.front() = 11; // Changes the first element
v.back() = 31;  // Changes the last element
```

They take no index argument and each takes constant time. Check `!v.empty()` first: calling either on an empty vector is invalid.

### `data()`: a pointer to the elements

`v.data()` gives a pointer to the contiguous element storage. It is mainly useful when another function expects a pointer to an array. Pass `v.size()` separately if that function needs the length. Avoid keeping the pointer after changing the vector: a reallocation can make it point to old memory.

## 6. Checking size and available room

```cpp
std::vector<int> v{10, 20, 30};

v.size();     // 3: number of elements
v.empty();    // false: is size zero?
v.capacity(); // At least 3: room before growth
```

Each takes constant time. `empty()` returns a `bool`, either `true` or `false`. Use `if (v.empty())` when you just need to know whether the vector has any elements.

## 7. Adding elements

### `push_back()` and `emplace_back()`: add at the end

```cpp
std::vector<int> v{10, 20};
v.push_back(30);    // 10, 20, 30
v.emplace_back(40); // 10, 20, 30, 40
```

`push_back(value)` adds a value. `emplace_back(arguments...)` constructs an element from its arguments at the end. The distinction is clearer with a type that has a constructor:

```cpp
#include <string>

std::vector<std::string> words;
words.push_back(std::string(3, 'x')); // Add "xxx"
words.emplace_back(3, 'x');           // Construct "xxx" in the vector
```

`emplace_back()` can avoid a temporary object, but it is not automatically faster. Use the clearer expression for the job.

Most end additions are quick. If the vector is full, it must allocate new storage and move or copy the old elements. That one addition can take time proportional to the size. Across many additions, the **average work per addition** stays constant; this is called *amortized constant time*.

### `insert()` and `emplace()`: add at a position

An iterator marks where to insert. `v.begin()` is the start; `v.begin() + 1` is one position later.

```cpp
std::vector<int> v{10, 30};
v.insert(v.begin() + 1, 20); // 10, 20, 30
v.emplace(v.begin(), 5);     // 5, 10, 20, 30
```

`insert(position, value)` adds a value before that position. `emplace(position, arguments...)` constructs one there. Both may shift later elements right and may reallocate. Adding near the front or middle takes time proportional to the number of elements shifted.

## 8. Removing and replacing elements

### `pop_back()`: remove the last element

```cpp
std::vector<int> v{10, 20, 30};
v.pop_back(); // 10, 20
```

It returns nothing, so save `v.back()` first if you need the value. Calling it on an empty vector is invalid. It normally takes constant time.

### `erase()`: remove at a position or over a range

```cpp
std::vector<int> v{10, 20, 30, 40};
v.erase(v.begin() + 1);          // 10, 30, 40
v.erase(v.begin() + 1, v.end()); // 10
```

The two-argument form removes from the first iterator **up to but not including** the second. Erasing shifts later elements left, so erasing near the front is slower than `pop_back()`. `erase()` returns an iterator to the element now following the removed part.

### `clear()`, `assign()`, and `resize()`

```cpp
std::vector<int> v{10, 20, 30};

v.clear();       // No elements; size is 0
v.assign(3, 7);  // Replace contents: 7, 7, 7
v.resize(5);     // 7, 7, 7, 0, 0
v.resize(2);     // 7, 7
```

`clear()` removes every element but keeps allocated capacity for future additions. `assign(count, value)` replaces all elements with `count` copies of `value`. `resize(count)` keeps existing elements that fit, creates new ones when growing, and removes elements from the end when shrinking. For `int`, newly created elements are zero-initialized in this example.

### `shrink_to_fit()`: ask to release spare memory

`v.shrink_to_fit()` asks the vector to reduce unused capacity. The implementation may ignore the request. If it moves elements into smaller storage, old pointers, references, and iterators become invalid. Most programs do not need to call this.

## 9. Looping through a vector

A **range-based loop** visits each element in order:

```cpp
std::vector<int> v{10, 20, 30};

for (int value : v) {
    std::cout << value << ' '; // Prints 10 20 30
}
```

Here `value` is a **copy** of each `int`. Changing it does not change the vector. Use a reference (`&`) to modify the real elements:

```cpp
for (int& value : v) {
    value *= 2; // Vector becomes 20, 40, 60
}
```

For a large element you only want to read, `const auto& value` avoids a copy and prevents changes. `auto` asks C++ to infer the type; `const` means read-only.

If you need the index too, use an index loop:

```cpp
for (std::size_t i = 0; i < v.size(); ++i) {
    std::cout << i << ": " << v[i] << '\n';
}
```

`std::size_t` is an unsigned integer type used for sizes and indices. The `<` condition works even when the vector is empty. Avoid `i <= v.size()`: it tries to access one position past the end.

## 10. Algorithms that work with vectors

The `begin()` and `end()` functions give iterators that describe the whole vector. Many standard algorithms take those two positions. Include `<algorithm>` to use these functions.

```cpp
#include <algorithm>

std::vector<int> v{4, 1, 3, 1};
std::sort(v.begin(), v.end());    // 1, 1, 3, 4
std::reverse(v.begin(), v.end()); // 4, 3, 1, 1
```

`std::sort` orders elements, usually with the smallest first. `std::reverse` reverses their current order; it does not sort them.

`std::find` searches for a value. It returns an iterator to the first match, or `v.end()` if it finds none:

```cpp
auto found = std::find(v.begin(), v.end(), 3);
if (found != v.end()) {
    std::cout << "Found 3\n";
}
```

There is no `v.find(3)` member function for a vector. `std::find` checks elements one by one, so it can take time proportional to the vector's size.

To remove **all** copies of a value in C++17, use the erase-remove pattern:

```cpp
v.erase(std::remove(v.begin(), v.end(), 1), v.end());
```

`std::remove` moves values you want to keep toward the front and returns the new logical end. It does **not** shrink the vector. `erase` removes the leftover elements. In C++20, `std::erase(v, 1)` is shorter.

## 11. Time costs, in plain English

Big-O describes how work grows as the vector gets larger. `O(1)` is roughly the same work regardless of size. `O(n)` may touch each of `n` elements. `O(n log n)` grows faster than `O(n)` but much slower than `O(n²)`. These are growth rates, not stopwatch measurements.

Here **n** is the current size. “End” means the back of the vector.

| Operation | Usual time | Why |
| --- | --- | --- |
| `[]`, `at()`, `front()`, `back()`, `data()` | `O(1)` | Go straight to a known position. |
| `size()`, `empty()`, `capacity()`, `begin()`, `end()` | `O(1)` | Read stored information or a position. |
| `push_back()`, `emplace_back()` | Amortized `O(1)`; one growth can be `O(n)` | Usually use spare capacity; sometimes move all elements. |
| `pop_back()` | `O(1)` | Remove only the final element. |
| `insert()`, `emplace()`, `erase()` | `O(n)` near the front or middle | Shift the elements after the position. |
| `clear()` | `O(n)` | Destroy all elements. |
| `reserve(count)` | `O(1)` if nothing changes; `O(n)` if it grows | Growing moves or copies existing elements. |
| `resize(count)` | Up to `O(n + count)` | May create or destroy elements and may reallocate. |
| `assign(count, value)` | `O(n + count)` | Replace old elements with new ones. |
| Copy a vector | `O(n)` | Copy every element into separate storage. |
| `std::find` | `O(n)` | Might inspect every element. |
| `std::reverse` | `O(n)` | Swap elements from the ends inward. |
| `std::sort` | `O(n log n)` | Compare and rearrange elements. |

The cost of each element matters too. Moving a large object, copying a string, or running a destructor can take extra work. The table describes collection operations, not exact running times.

Compared with a fixed-size array, a vector carries bookkeeping and may reserve extra element storage. That extra room helps make repeated additions cheap. Compared with a linked list, a vector usually uses less memory per element and is often faster to loop through because elements are next to each other.

Memory use changes differently from size:

| Operation | What usually happens to allocated storage |
| --- | --- |
| `push_back()` or `insert()` | Reuses spare capacity if there is enough; otherwise allocates a larger block. During growth, the old and new blocks can briefly coexist. |
| `reserve(count)` | May allocate a larger block, but never creates extra elements. |
| `resize(count)` | Creates or removes elements; growing past capacity may allocate a larger block. |
| `pop_back()`, `erase()`, or `clear()` | Removes elements but keeps capacity, ready to reuse. |
| Copying a vector | Allocates separate storage for the copy's elements. |
| `shrink_to_fit()` | May reduce capacity, but this is only a request. |

## 12. Copies, references, and invalidation

Copying a vector copies its elements into independent storage:

```cpp
std::vector<int> original{1, 2, 3};
std::vector<int> copy = original;
copy[0] = 99;
// original is still 1, 2, 3
```

Copying a huge vector costs time and memory. If a function only reads it, pass a **const reference** so it uses the existing vector:

```cpp
void printScores(const std::vector<int>& scores) {
    for (int score : scores) {
        std::cout << score << ' ';
    }
}
```

If a function must change the caller's vector, use `std::vector<int>&` without `const`.

A pointer, reference, or iterator to a vector element can become invalid when the vector changes. **Any reallocation invalidates all of them.** Even without reallocation, inserting or erasing can invalidate positions at or after the changed spot. `reserve()` may reallocate; `push_back()` may reallocate when full. Look up an element again after such a change.

```cpp
std::vector<int> v{10, 20};
int& first = v[0];
v.push_back(30); // Might reallocate
// Do not use first here; use v[0] again instead.
```

## 13. Vector, arrays, and other containers

A **raw array** is the built-in C++ array form, such as `int a[3]`. `std::array` is a standard-library wrapper for a fixed-size array. A vector can change size while running; these two cannot.

```cpp
#include <array>
#include <vector>

int raw[3]{10, 20, 30};               // Raw array
std::array<int, 3> fixed{10, 20, 30}; // Fixed-size standard container
std::vector<int> growing{10, 20, 30}; // Can add more elements

growing.push_back(40);
// fixed.push_back(40); // No such function: fixed size
```

`std::array` remembers its size and works with standard algorithms, so prefer it over a raw array for an ordinary fixed-size collection. When a raw array is passed to many functions, it turns into a pointer and its length is no longer carried along; you then have to pass the length separately. Raw arrays remain useful for some low-level interfaces. Avoid `new[]` and `delete[]` for ordinary collections; a vector manages that memory for you.

| Type | Choose it when | Access and changes | Memory |
| --- | --- | --- | --- |
| `std::vector<T>` | Size can change, or is known only at runtime; you want fast indexing and iteration | Index `O(1)`; add at end amortized `O(1)`; middle changes `O(n)` | Owns dynamic storage; may keep spare capacity. |
| `std::array<T, N>` | Size is fixed and known when compiling | Index `O(1)`; cannot grow | Elements are inside the array object; no separate allocation for the array itself. |
| Raw array `T[N]` | A low-level interface specifically needs it | Index `O(1)`; cannot grow | No built-in size member; length is easy to lose when passing it around. |
| `std::deque<T>` | You frequently add and remove at **both** ends | Index `O(1)`; end changes `O(1)`; middle changes `O(n)` | Stored in blocks, so elements are not one contiguous row. |
| `std::list<T>` | You already have iterators to places where you often insert or erase | No fast index; finding a place `O(n)`; insert or erase there `O(1)` | Extra links and usually a separate allocation per element. |

`std::deque` (pronounced “deck”) is useful when removing from the front is common: doing that to a vector shifts all later elements. `std::list` is a linked list; fast insertion only helps once you have found the right position. Its extra memory and slower traversal often make vector the better default.

**Quick rule:** start with `std::vector`. Pick `std::array` for a fixed count known at compile time. Pick `std::deque` for frequent changes at both ends. Use a raw array or `std::list` when you have a specific reason.

## 14. A complete example

This program stores scores, sorts them, removes one score if present, and prints what remains:

```cpp
#include <algorithm>
#include <iostream>
#include <vector>

int main() {
    std::vector<int> scores;
    scores.reserve(4); // Expect about four scores; size is still 0

    scores.push_back(72);
    scores.push_back(45);
    scores.push_back(91);
    scores.push_back(80);

    std::sort(scores.begin(), scores.end()); // 45, 72, 80, 91

    auto found = std::find(scores.begin(), scores.end(), 45);
    if (found != scores.end()) {
        scores.erase(found); // 72, 80, 91
    }

    std::cout << "Scores (" << scores.size() << "): ";
    for (int score : scores) {
        std::cout << score << ' ';
    }
    std::cout << '\n'; // Scores (3): 72 80 91
}
```

The four `push_back()` calls add the scores. `reserve(4)` makes room in advance, but does not create elements. `sort` rearranges the elements. `find` gives an iterator to 45, and `erase` removes it. The range-based loop reads the remaining scores.

## 15. Common mistakes to watch for

- **Indexing an empty vector:** `std::vector<int> v; v[0] = 5;` is invalid. Use `v.push_back(5)` or `v.resize(1)` first.
- **Confusing capacity with size:** `reserve(100)` does not create 100 elements. Use `resize(100)` if you need 100 actual elements.
- **Using `front()`, `back()`, or `pop_back()` on an empty vector:** check `empty()` first.
- **Assuming `pop_back()` returns the removed value:** read or copy `back()` before removing it.
- **Using an old iterator or reference after a change:** growth can move all the elements to a new address.
- **Copying a large vector by accident:** use `const std::vector<T>&` for a function that only reads it.
- **Erasing inside a range-based loop:** use the erase-remove pattern shown above when removing values. Changing a vector while a range-based loop is using it can invalidate that loop's position.
