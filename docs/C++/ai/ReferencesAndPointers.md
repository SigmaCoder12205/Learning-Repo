# References and Pointers in C++: Names, Addresses, and Lifetimes

*A practical guide from first syntax to ownership and performance. Examples use C++17 unless marked C++20. Short snippets show only the relevant lines: put `#include` directives at the top of your file, function definitions outside other functions, and ordinary statements inside a function. The complete program near the end can be compiled as written.*

## 1. The picture to keep in your head

A variable such as `int score = 42;` names an object that holds a value. That object lives somewhere in memory for a period of time called its **lifetime**.

- A **reference** gives you another way to name an existing object.
- A **pointer** is an object whose value is an address. It can hold the address of another object, or a special value, `nullptr`, meaning it points to no object.
- Neither one automatically owns the object it refers to. Keeping an address does not keep its target alive.

```cpp
int score = 42;
int& alias = score;   // alias names score
int* address = &score; // address holds score's address

alias = 50;           // Changes score
*address = 60;        // Also changes score
// score is now 60
```

Think of the object as a house. A reference is another name for that house; a pointer is a card with its street address. The card can be blank (`nullptr`) or replaced with another address. Neither saves the house if it is demolished.

This picture is useful, but a reference is a C++ language feature, not a promise that the compiler stores a hidden pointer. Often it needs no separate storage at all.

| Question | Reference `T&` | Pointer `T*` |
| --- | --- | --- |
| Must name a target when created? | Yes, for an ordinary reference. | No; initialize it to `nullptr` if there is no target yet. |
| Can represent “nothing”? | Not as a valid ordinary reference. | Yes, with `nullptr`. |
| Can switch targets? | No; assignment changes the target's value. | Yes; assign another address. |
| Access the target | Use the reference like the object. | Dereference with `*`, or use `->` for a member. |
| Own or keep target alive? | No. | No; a smart pointer is a different type. |
| Support arithmetic? | No. | Some pointers do, within an array. |

## 2. Reading `&` and `*` without getting lost

The symbols change meaning depending on where they appear:

```cpp
int x = 10;
int& ref = x;  // & in a declaration: reference to int
int* ptr = &x; // * in a declaration: pointer to int; &x: address of x
int copy = *ptr; // * in an expression: object at ptr's address
```

`*ptr` is called **dereferencing**. It lets you read or change the pointed-to object. `&x` is the **address-of** expression. `nullptr` is the modern null pointer literal; prefer it to `NULL` or `0` for pointers.

Spaces do not change the type: `int* p`, `int *p`, and `int * p` mean the same thing. The `*` or `&` belongs to each declarator, though, so avoid mixed declarations:

```cpp
int* a = &x;
int b = 5;      // Clearly an int, not a pointer

// int* p, q;  // p is a pointer, but q is an int: easy to misread
```

An address is not the value stored there. If `ptr` points to `x`, then `ptr` is an address and `*ptr` is `x`. You can print a pointer to inspect an address, but its exact number is generally not useful or predictable.

## 3. References: an alias with a lifetime requirement

### Create and change through a reference

A reference must be initialized with a suitable object when it is declared. An ordinary lvalue reference such as `int&` cannot be left unbound, and assigning to it does not make it refer to something else.

```cpp
int first = 10;
int second = 20;
int& ref = first;

ref = 15;     // first becomes 15
ref = second; // Copies second's value into first; ref still names first
second = 30;  // first stays 20
```

`&ref` and `&first` are the same address. You normally use `ref` just as you would use `first`; there is no explicit dereference step.

**Important qualification:** a reference is not a safety shield. Code can create an invalid reference through undefined behavior, such as dereferencing a null pointer. References can also **dangle** after their objects die. Saying “references cannot be null” describes a valid reference, not a runtime check performed by C++.

### Read-only references

`const T&` lets you observe a `T` without changing it through that reference:

```cpp
#include <string>

std::string name = "Ada";
const std::string& view = name;
// view += "!"; // Error: view cannot modify name
name += "!";     // Allowed: name itself is not const
// view now sees "Ada!"
```

`const` here limits *this access path*. It does not freeze a mutable object against changes made through other names. If the original object itself is `const`, you cannot get an ordinary mutable `T&` to it.

A `const T&` can also bind to a temporary:

```cpp
const std::string& greeting = std::string("hello");
// In this direct initialization, the temporary string lives as long as greeting.
```

Lifetime extension has exceptions. In particular, returning a reference to a local object or to a temporary created for a function call does **not** make that object survive for the caller. Do not build an API around an assumed extension you have not checked.

An ordinary mutable `int&` needs a suitable lvalue. It cannot bind directly to a temporary such as `1 + 2`. This prevents you from accidentally treating a short-lived result as a required mutable object.

### Where references are especially useful

References are a natural choice when a function requires an existing object, when a loop should edit actual elements, or when you want an alias to a stable part of an object:

```cpp
#include <vector>

std::vector<int> scores{10, 20, 30};
for (int& score : scores) {
    score += 5; // Changes the vector's elements
}
```

For read-only loops over expensive-to-copy elements, `const auto&` avoids copies. For tiny values such as `int`, copying in a loop is often simpler and just as fast.

### `auto` can quietly make a copy

`auto` by itself usually drops references and top-level `const`. Add `&` when you want an alias:

```cpp
int x = 10;
int& ref = x;
auto copy = ref;       // int: independent copy
auto& alias = ref;     // int&: changes x
const auto& reader = x; // const int&: read-only access

copy = 20;  // x is still 10
alias = 30; // x is now 30
```

This distinction matters especially in range-based loops and when binding the result of a function that returns a reference.

## 4. Pointers: addresses you can store and change

```cpp
int first = 10;
int second = 20;
int* ptr = &first;

*ptr = 11;     // first becomes 11
ptr = &second; // ptr now points to second
*ptr = 21;     // second becomes 21
ptr = nullptr; // ptr now points to nothing
```

**Initialize every pointer.** `int* ptr;` inside a function leaves it with an indeterminate value. Reading or dereferencing it is not a way to discover whether it happens to point somewhere useful.

Check an optional pointer before dereferencing it:

```cpp
if (ptr != nullptr) {
    // Safe from null dereference, provided ptr still points to a live int.
    *ptr = 30;
}
```

`if (ptr)` means the same null check. A non-null pointer can still be dangling, point one past an array, or otherwise be invalid to dereference. A null check alone cannot establish that a target is alive.

Two pointers can be compared with `==` to ask whether they identify the same place; this does **not** compare the values stored in the objects they point to. To compare those values, first establish that both pointers can be dereferenced, then compare `*a == *b`.

### Changing a pointer versus changing its target

```cpp
int a = 1;
int b = 2;
int* p = &a;

p = &b; // Change the pointer: a is still 1
*p = 7; // Change the target: b becomes 7
```

The pointer itself is an object, so it has its own address: `&p` has type `int**`. A pointer to pointer is useful when a function must change the caller's pointer or when working with a C API, but a reference to the pointer (`int*&`) is often easier to read in C++.

### A pointer does not mean heap allocation

`&a` points to a local object here. The pointer does not allocate memory, and it must never be passed to `delete`:

```cpp
int a = 5;
int* p = &a; // No allocation
// delete p; // Wrong: a was not created by new
```

Pointers can point to local objects, global objects, members, array elements, or dynamically allocated objects. **Address, location, and ownership are separate questions.**

## 5. `const` combinations: what may change?

Read the declaration from the identifier outward:

| Declaration | Change the pointed-to value through this name? | Point somewhere else? |
| --- | --- | --- |
| `int* p` | Yes | Yes |
| `const int* p` | No | Yes |
| `int* const p = &x` | Yes | No |
| `const int* const p = &x` | No | No |
| `int& r = x` | Yes | No: references cannot be reseated |
| `const int& r = x` | No | No |

`const int*` can also be written `int const*`. Both mean “pointer to const int.”

```cpp
int x = 10;
int y = 20;
const int* p = &x;
p = &y;   // Fine: the pointer can change
// *p = 30; // Error: cannot change y through p
y = 30;  // Fine: y is a mutable object

int* const fixed = &x;
*fixed = 40; // Fine: x changes
// fixed = &y; // Error: fixed itself is const
```

Constness is about permitted access. It does not automatically make shared data thread safe; another alias may still mutate it.

## 6. Passing objects to functions

The right parameter type communicates whether the function needs a copy, must receive an object, may receive no object, or takes ownership.

```cpp
#include <string>

void setToZero(int value) { value = 0; }       // Changes only a copy
void increment(int& value) { ++value; }        // Changes caller's int
bool isEmpty(const std::string& text) {        // Reads caller's string
    return text.empty();
}
void resetIfPresent(int* value) {              // Null means "do nothing"
    if (value) *value = 0;
}
```

```cpp
int count = 5;
setToZero(count);       // count is still 5
increment(count);       // count is 6
resetIfPresent(&count); // count is 0
resetIfPresent(nullptr); // Also valid
```

| Intent | Common parameter | Why |
| --- | --- | --- |
| Read a small, cheap-to-copy value | `T` | Simple; a local copy can be efficient. |
| Read a larger existing object without copying | `const T&` | Requires a value, avoids copying. |
| Modify an existing caller-owned object | `T&` | Required output or input/output object. |
| Modify or inspect an object that may be absent | `T*` or `const T*` | `nullptr` clearly represents absence. |
| Take a separate value to keep or move from | `T` | The function owns its parameter value. |
| Take exclusive ownership of a dynamic object | `std::unique_ptr<T>` | Ownership transfers visibly. |
| Share ownership | `std::shared_ptr<T>` | Retains the object through a reference count. |

Passing a pointer **by value** copies the address. The function can change the target, but changing its local pointer does not change the caller's pointer. To reseat the caller's pointer, use `T*&`, `T**`, or return the new pointer; choose the clearest interface.

For a function that returns an answer, prefer returning it normally over using an output reference just to avoid a copy. Compilers can often construct return values directly, and C++ can move eligible values efficiently. Use output references when the API genuinely needs to modify an existing object or produce multiple results.

### Returning references and pointers

Return a reference only when the returned object will outlive its use and the API promises it exists:

```cpp
#include <vector>

int& first(std::vector<int>& values) {
    return values.at(0); // Throws if empty; reference valid while element stays valid
}
```

Returning a pointer can represent “there may be no result”:

```cpp
int* findValue(std::vector<int>& values, int wanted) {
    for (int& value : values) {
        if (value == wanted) return &value;
    }
    return nullptr;
}
```

The vector must still exist, and operations that invalidate its elements also invalidate either returned handle. Never return `&local` or a reference to `local` when `local` is a function's ordinary local variable: it dies on return.

## 7. Lifetime: the rule behind most pointer bugs

An object is usable only during its lifetime. A **dangling** pointer or reference still appears to name an address or object, but its former target is gone. Using it to access the target is undefined behavior: C++ promises no particular result.

```cpp
int* p = nullptr;
{
    int local = 7;
    p = &local;
} // local's lifetime ends
// *p is invalid, even if the old bytes still seem to contain 7.
```

A `const&` parameter can safely read a temporary during the call. It cannot safely save a reference to that temporary for later:

```cpp
#include <string>

const std::string* saved = nullptr;
void remember(const std::string& text) { saved = &text; }

remember(std::string("brief"));
// saved dangles after the call's full expression ends.
```

### Containers may move or remove their elements

```cpp
#include <vector>

std::vector<int> values{1, 2};
int& firstValue = values[0];
values.push_back(3); // May reallocate
// firstValue may now dangle. Fetch values[0] again.
```

With `std::vector`, **reallocation invalidates all pointers, references, and iterators to elements**. Even without reallocation, inserting or erasing can invalidate handles at or after the changed position. `erase` destroys removed elements. `std::string` can similarly invalidate pointers, references, and views into its characters when its storage changes.

Other lifetime traps include returning a view into a temporary, holding a pointer after `delete`, capturing a reference in a callback that runs after the object dies, and using a pointer to a destroyed owner. A pointer's non-null value does not reveal any of these.

**Practical rule:** make the owner and the expected lifetime obvious. If that is hard, keep an index, copy the needed value, or redesign the API so the data is owned where it is used. An index into a vector can survive reallocation, but only if the logical element still exists at that index after changes.

## 8. Ownership and memory management

A **non-owning** pointer or reference only accesses someone else's object. An **owner** is responsible for keeping an object alive and eventually cleaning it up. Raw `T*` and `T&` do not express ownership by themselves.

For ordinary values, simply store the object directly. For a changing collection, use `std::vector<T>`. When a dynamic object needs independent lifetime, use a smart pointer that owns it.

```cpp
#include <memory>

auto item = std::make_unique<int>(42);
int* observer = item.get(); // Borrowed; item still owns the int
*observer = 43;
item.reset();               // Destroys the int
// observer now dangles. Never dereference it.
```

`std::unique_ptr<T>` owns one dynamically allocated `T`. It cleans up automatically when it dies or is reset. It cannot be copied, but can be moved to transfer ownership:

```cpp
auto owner = std::make_unique<int>(10);
auto newOwner = std::move(owner); // newOwner owns the int; owner is empty
```

Include `<utility>` for `std::move`. The line above moves the **ownership handle**; it does not copy the `int`.

`std::shared_ptr<T>` allows several owners to keep one object alive. Copying it changes a reference count and is usually more expensive than copying a raw pointer. Use it only when lifetime truly is shared. A cycle of shared owners can keep objects alive forever; use `std::weak_ptr<T>` for a non-owning link into a shared ownership graph.

Manual `new` and `delete` exist, but they make you responsible for matching every allocation to exactly one deletion, including paths that throw exceptions. Prefer direct values, containers, and smart pointers. Never `delete` a borrowed address, never `delete` the same allocation twice, and never use a pointer after its owner deletes the target. `delete nullptr` is allowed, but it is rarely a reason to manage memory manually.

**Ownership is not the same as reachability.** A raw pointer does not extend an object's lifetime. A `shared_ptr` passed to a function may extend it; a `const T&` passed to that function does not.

## 9. Arrays, pointer arithmetic, and safer views

An array is **not** a pointer. It contains its elements. In many expressions, though, an array converts to a pointer to its first element; this is called **array-to-pointer decay**.

```cpp
int numbers[3]{10, 20, 30};
int* p = numbers;     // Same first-element address as &numbers[0]
int second = *(p + 1); // 20; same element as numbers[1]
```

Pointer arithmetic moves in units of the pointed-to type: `p + 1` points to the next `int`, not the next byte. `p[i]` means `*(p + i)`.

Arithmetic and ordering comparisons are only meaningful within the same array object (including its one-past-end position). You may form a pointer one past the last element to mark an end, but you may **not dereference it**. Going farther or using unrelated objects as one array is invalid. A bare pointer does not carry a length.

```cpp
void printThree(const int* values) {
    // values alone gives no way to know that the caller supplied three ints.
}
```

For an owned, fixed-size group use `std::array<T, N>`; for an owned, resizable group use `std::vector<T>`. For a borrowed, contiguous group use `std::span<T>` (C++20):

```cpp
// C++20
#include <span>

void doubleAll(std::span<int> values) {
    for (int& value : values) value *= 2;
}
```

`std::span` carries a pointer and a size conceptually, so the function can work with an array, vector, or suitable contiguous storage without owning or copying the elements. The source must remain alive, and reallocation can invalidate the span. In C++17, pass `const std::vector<T>&` when the function specifically wants a vector, or a pointer plus length when interoperability requires it.

For borrowed read-only text, `std::string_view` also carries a length; it does not own characters or promise null termination. Use `std::string` when you need to own text. As with a span, do not store a view longer than its source lives.

## 10. Rvalue references and moving: the next layer

So far, `T&` has meant an **lvalue reference**, usually bound to an existing named object. `T&&` is an **rvalue reference**, usually bound to a temporary or to an expression marked movable. These are related but serve different jobs.

```cpp
#include <string>
#include <utility>

std::string source = "a lot of text";
std::string destination = std::move(source);
```

`std::move(source)` does not itself move bytes. It marks `source` as eligible for moving; the selected string constructor does the work. After a move, `source` is valid but its value is unspecified, so assign it a new value or inspect it only in ways allowed for a valid string.

```cpp
std::string&& temporaryRef = std::string("hello");
// The temporary lives as long as temporaryRef in this direct binding.
```

A **named** rvalue reference expression is still an lvalue: inside a function, `value` in `void consume(T&& value)` is a named object. Generic templates add another rule: in a deduced `T&&` parameter, `T&&` can become a **forwarding reference** that accepts both lvalues and rvalues. That is useful when writing generic wrappers, but it is not needed for ordinary reference and pointer use. Do not sprinkle `std::move` into code merely because moving sounds faster; moving from something you still need makes code harder to reason about.

## 11. Speed and memory: what the syntax does not tell you

Neither references nor pointers are inherently faster. Both generally provide direct access to an existing object and avoid copying that object. Compilers often generate similar machine code for a reference parameter and a non-null pointer parameter. The real costs are usually the copy you avoided, a dynamic allocation, an extra memory access, cache behavior, or shared ownership bookkeeping.

| Choice | Typical cost | Watch for |
| --- | --- | --- |
| Pass a small `int`, enum, or pointer by value | A small copy | Usually the clearest and fast. |
| Pass a large object by `const&` | No object copy | Requires the original to stay alive; aliasing can limit some optimizations. |
| Pass a pointer by value | Copies an address | Dereferencing may touch distant memory; null is possible. |
| Copy a large `std::vector<T>` | Copies its elements and allocates storage | Useful when an independent copy is actually wanted. |
| Move a `std::vector<T>` | Often transfers ownership of its storage | Source is left valid but changed; exact cost depends on operation and allocator. |
| Use `std::unique_ptr<T>` | One owner handle plus dynamic object allocation | Allocation and pointer chasing may outweigh any copy saved. |
| Copy a `std::shared_ptr<T>` | Updates shared ownership bookkeeping | Use only for real shared lifetime. |

On a common 64-bit system an object pointer often occupies 8 bytes, but the C++ language does not require every pointer to have that size or representation. `sizeof(ref)` reports the size of the **referred-to type**, not storage used to implement a reference. A reference member may take space; the amount is implementation-dependent.

Passing `const T&` is not always an optimization. For cheap scalar types, by-value is often as good or better. Passing a large object by value is appropriate when the function needs its own copy or plans to keep/move it. Returning a large value is also usually fine because of copy elision and move support; measure real code before introducing awkward indirection.

Memory layout often matters more than pointer syntax. A vector of objects stores them contiguously, which is usually friendly to CPU caches. A vector of owning pointers stores the pointers contiguously, but its objects may be scattered across allocations. Use indirect storage when you need stable object identity, polymorphism, or independent lifetimes, not automatically for speed.

Neither `const&` nor a pointer is automatically thread safe. If multiple threads access the same object and one writes without proper synchronization, the program can have a data race.

## 12. A decision guide

| What you need | Start with |
| --- | --- |
| A local number or small simple value | Store/pass `T` directly. |
| A local object that owns its own data | Store `T` directly. |
| A function must read a large existing object | `const T&`. |
| A function must modify a required existing object | `T&`. |
| “No object” is a normal possibility | `T*` or `const T*`; document what null means. |
| An optional *owned* value | `std::optional<T>` (C++17). |
| An optional borrowed reference | `T*`, or `std::optional<std::reference_wrapper<T>>` when that interface helps. |
| A sequence you own and can resize | `std::vector<T>`. |
| A fixed-size sequence you own | `std::array<T, N>`. |
| A borrowed contiguous sequence | `std::span<T>` (C++20), or pointer plus size for a C-style API. |
| A dynamic object with one owner | `std::unique_ptr<T>`. |
| A dynamic object with genuinely shared owners | `std::shared_ptr<T>`; use `std::weak_ptr<T>` for non-owning links. |
| An independent result from a function | Return `T` by value. |

There is no `std::optional<T&>` in C++17. A pointer is often the simplest optional reference. When you do use a raw pointer, say whether it is borrowed, whether null is allowed, and what operations can invalidate it.

## 13. Less common pointer forms

These exist, but most everyday code does not need them:

- `void*` can hold an object address without its type. You cannot dereference it until converting to the correct type. Use typed interfaces in C++ where possible; `void*` is common at C boundaries.
- A **function pointer** holds a callable function's address: `int (*operation)(int, int) = &add;`. Use it for simple callbacks, or use a lambda/standard callable when you need captured state.
- A **pointer to member** is different from an object pointer: `int Widget::* field = &Widget::value;`. It needs an object to be used. This is mostly relevant to generic or framework code.
- `T**` points to a pointer; `T*&` references a pointer. Both can let a function replace a caller's pointer, but returning a result is often clearer.
- Casts such as `reinterpret_cast`, raw byte access, and manually managed placement/lifetime rules are advanced low-level tools. They do not make an invalid address valid. Use them only when an interface or systems task truly requires them.

## 14. Complete example: required, optional, and owned

This program uses a required reference for an update, an optional pointer for a lookup, and a vector that owns its elements:

```cpp
#include <iostream>
#include <vector>

struct Player {
    int id;
    int score;
};

void addPoints(Player& player, int points) {
    player.score += points;
}

const Player* findPlayer(const std::vector<Player>& players, int id) {
    for (const Player& player : players) {
        if (player.id == id) return &player;
    }
    return nullptr;
}

int main() {
    std::vector<Player> players{{1, 10}, {2, 20}};

    addPoints(players[0], 5);

    if (const Player* found = findPlayer(players, 1)) {
        std::cout << "Player " << found->id
                  << " has " << found->score << " points\n";
    }

    // Print: Player 1 has 15 points
}
```

`found->score` is shorthand for `(*found).score`. The pointer is checked before use. It borrows an element owned by `players`, so it must not be kept across a change that could invalidate vector elements.

Compile with a C++17 compiler, for example:

```sh
g++ -std=c++17 -Wall -Wextra -pedantic example.cpp -o example
./example
```

## 15. Common mistakes and a quick checklist

1. **Confusing `ref = other` with reseating a reference.** It assigns a value to the original target.
2. **Using an uninitialized pointer.** Initialize to a valid address or `nullptr`.
3. **Dereferencing `nullptr` or a dangling pointer.** Null checks catch only the first case. Track ownership and lifetime as well.
4. **Returning a pointer or reference to a local object.** Local storage ends when the function returns.
5. **Assuming `const T&` extends every temporary's life.** Direct binding can extend it, but a function return or stored borrowed reference needs separate lifetime reasoning.
6. **Keeping an element reference while growing or erasing from a vector.** Reacquire the element afterward.
7. **Using `delete` on a pointer that merely borrows an object.** Only the owner destroys it; prefer automatic owners.
8. **Treating an array pointer as though it knows its length.** Pass a size or use `std::span`.
9. **Passing every parameter by reference for speed.** Cheap values are often best passed by value.
10. **Using `shared_ptr` to avoid deciding who owns an object.** Shared ownership has costs and can create cycles.

Before storing or returning a pointer, reference, span, or string view, ask: **Who owns the target? How long will it live? Can an operation move or destroy it? Can absence occur?** Those four answers prevent most bugs in this topic.

### Try it yourself

1. Write `void doubleValue(int&)` and `void doubleIfPresent(int*)`. Call the second with both `&number` and `nullptr`.
2. Make a vector, save a reference to its first element, and reason about which vector operations could invalidate that reference. Reacquire it before use.
3. Write a search function that returns `const int*` for a found element and `nullptr` otherwise. Explain who owns the element and how long the returned pointer stays valid.

For the containers and text used in these examples, continue with [the vector guide](StdVectors.md) and [the string guide](CStrings.md).
