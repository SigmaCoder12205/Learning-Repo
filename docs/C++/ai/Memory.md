# Memory in C++: a beginner's guide

Each topic starts with a **metaphor**, followed by an **actual explanation**. The metaphors help you picture the idea; the explanations tell you what C++ really does and where the picture stops being accurate.

Examples use **C++17**. Small snippets show only the relevant code: put their `#include` lines at the top of a file and ordinary statements inside a function such as `main()`. Function and class definitions go outside `main()`. Section 17 contains a complete program you can compile as written.

Read sections 1–8 first to build the foundation. Then work through pointers, containers, ownership, and mistakes. You do not need to memorise every detail on your first reading.

**Contents**

1. [What is memory?](#1-what-is-memory)
2. [Bits, bytes, and the size of a type](#2-bits-bytes-and-the-size-of-a-type)
3. [Objects, values, and addresses](#3-objects-values-and-addresses)
4. [FIFO and LIFO](#4-fifo-and-lifo-two-different-orders)
5. [Scope, lifetime, and storage duration](#5-scope-lifetime-and-storage-duration)
6. [The stack and function calls](#6-the-stack-and-function-calls)
7. [The heap and dynamic allocation](#7-the-heap-and-dynamic-allocation)
8. [Comparing storage categories](#8-stack-heap-and-other-storage-how-they-differ)
9. [Pointers and references](#9-pointers-and-references)
10. [Arrays, contiguous storage, and bounds](#10-arrays-contiguous-storage-and-bounds)
11. [How vectors and strings use memory](#11-how-vectors-and-strings-use-memory)
12. [Constructors, destructors, and RAII](#12-constructors-destructors-and-raii)
13. [Ownership and smart pointers](#13-ownership-and-smart-pointers)
14. [Copying, moving, and passing to functions](#14-copying-moving-and-passing-to-functions)
15. [Common memory mistakes](#15-common-memory-mistakes)
16. [Alignment, padding, and access speed](#16-alignment-padding-and-access-speed)
17. [A complete program](#17-a-complete-program-watch-lifetimes-happen)
18. [Practice questions and quick reference](#18-practice-explain-it-back-to-yourself)

## 1. What is memory?

### Metaphor first: a workbench and a cupboard

Imagine working on a model airplane. Your cupboard stores supplies between projects. Your workbench holds the pieces you are using right now.

Your computer's drive is like the cupboard. Its working memory is like the workbench. Opening a saved project brings information into the workspace so you can use and change it. Putting something on the workbench does not automatically save it back in the cupboard.

### Actual explanation

When a program runs, it needs storage for its data: numbers, text, objects, images, and information about function calls. We call this **memory**.

**RAM** is the computer's physical working memory. An SSD or hard drive provides persistent storage. A variable in a running program does not automatically become a saved file. Your program must explicitly write data to a file or another persistent destination if you want to keep it.

On a typical desktop operating system, your program uses **virtual memory addresses**. The operating system and hardware translate these into physical storage. Consequently, an address printed by your program is usually a virtual address, not a direct position on a RAM chip.

For learning C++, start with this model: a program can use regions of memory to hold objects, and it must follow rules about which objects it can access and when.

## 2. Bits, bytes, and the size of a type

### Metaphor first: switches arranged into boxes

A bit is like a switch that can be off or on. Group switches into small boxes, and you can represent more possibilities. Different kinds of data need different numbers of boxes.

A tiny label and a detailed blueprint do not require the same amount of storage.

### Actual explanation

A **bit** has two possible values: `0` and `1`. Eight bits can represent 256 different combinations. A **byte** is the basic addressable storage unit in C++. On ordinary desktop systems, a byte contains eight bits.

C++ guarantees that `sizeof(char)` is `1`, but that means one **C++ byte**. The number of bits in that byte is given by `CHAR_BIT` from `<climits>`. It is at least eight. The standard describes memory as sequences of addressable bytes. See the [C++ working draft's memory model](https://eel.is/c++draft/intro.memory).

A **type** describes the values an object can hold and the operations allowed on it. It also determines its storage size and representation.

| Type | What it represents | Common size on desktop systems |
| --- | --- | --- |
| `char` | A character-sized integer value | 1 byte |
| `bool` | `true` or `false` | 1 byte |
| `int` | A whole number | 4 bytes |
| `double` | A floating-point number | 8 bytes |
| `int*` | A pointer to an `int` | 8 bytes on many 64-bit systems |

These common sizes are examples, not portable promises. In particular, a `bool` is not necessarily stored in one bit, and an `int` is not guaranteed to be four bytes.

Use `sizeof` to ask your implementation:

```cpp
#include <climits>
#include <iostream>

int score = 42;
std::cout << "Bits per byte: " << CHAR_BIT << '\n';
std::cout << "Bytes in int: " << sizeof(int) << '\n';
std::cout << "Bytes in score: " << sizeof(score) << '\n';
```

`sizeof(score)` measures the storage occupied by the `int` object. It does not measure the length of its name or the number of decimal digits in its value. On a system with four-byte `int`s, `int score = 1;` and `int score = 1000000;` both occupy four bytes.

The same underlying bits can mean different things under different types. Bytes do not carry labels saying “I am an integer” or “I am text.” C++'s type and object rules determine how you may interpret them.

## 3. Objects, values, and addresses

### Metaphor first: numbered lockers

Imagine a hallway of numbered lockers. A locker number tells you where something is. The contents tell you what is there. A label such as “Alex's supplies” gives you a convenient name for it.

The label, the locker number, and the contents are three different things.

### Actual explanation

Consider:

```cpp
int score = 42;
```

- `score` is the variable's **name**.
- The `int` it names is an **object** occupying storage.
- `42` is the object's current **value**.
- The object's **address** identifies where its storage begins.

In C++, “object” includes an ordinary `int`; it does not only mean an instance of a class you wrote.

Suppose an `int` occupies four bytes and this one starts at the invented address `1000`:

```text
Byte address:    1000    1001    1002    1003
                [----- one int object -----]
                          value: 42
```

This diagram shows the region occupied by the object. It does not show how the number 42 is encoded across those bytes.

The expression `&score` obtains its address:

```cpp
#include <iostream>

int score = 42;
std::cout << score << '\n';  // Prints the value: 42
std::cout << &score << '\n'; // Prints an address
```

Addresses are commonly printed in hexadecimal, often beginning with `0x`. The exact address can change between runs. You should not hard-code one you saw in the output.

The compiler may also keep a value in a CPU register or remove unnecessary storage entirely, provided the program still behaves as C++ requires. The locker picture describes the language-level idea; it is not an exact hardware diagram.

## 4. FIFO and LIFO: two different orders

### FIFO — metaphor first: a checkout queue

Three people join a checkout queue: Alice, then Ben, then Cara. Alice is served first because she arrived first. Ben follows, then Cara.

### FIFO — actual explanation

**FIFO** means **First In, First Out**. The earliest item added is the earliest item removed. This is the usual behaviour of a **queue**.

```text
Add:     Alice → Ben → Cara
Remove:  Alice → Ben → Cara
```

In C++, `std::queue` provides this behaviour:

```cpp
#include <queue>

std::queue<int> jobs;
jobs.push(10);
jobs.push(20);
jobs.push(30);

int next = jobs.front(); // 10: the earliest item
jobs.pop();              // Removes 10; does not return it
// The front is now 20.
```

FIFO describes an order of processing. It does not specify whether the queue's storage comes from the stack or heap.

### LIFO — metaphor first: a pile of plates

Put plate A on a table, plate B on top of A, and plate C on top of B. You normally take C off first, then B, then A.

The last plate placed on the pile is the first one removed.

### LIFO — actual explanation

**LIFO** means **Last In, First Out**. The newest item is removed first. This is the behaviour of a **stack** data structure.

```text
Add:     A → B → C
Remove:  C → B → A
```

The usual **function call stack** follows this nested pattern: when function A calls B, B finishes before A resumes. We will examine that in section 6.

`std::stack` is a container adapter you can use in your own program. It is a separate concept from the implementation's function call stack, even though both use LIFO behaviour.

## 5. Scope, lifetime, and storage duration

### Metaphor first: a room, a resident, and a lease

Imagine an office building. A name on a room directory tells you where that name is recognised. A resident is present for some period. The room itself is available under a lease.

Knowing where a name appears, when someone is present, and how long the room is available are related questions, but they are not identical.

### Actual explanation

Three terms matter:

- **Scope:** the part of the source code where a name can be used to refer to a declaration.
- **Lifetime:** the period during execution when an object exists. For ordinary examples here, it begins when initialization completes and ends when the object is destroyed.
- **Storage duration:** the rules governing how long the underlying storage is available.

```cpp
int outer = 10;

{
    int inner = 20;
    outer += inner; // Both names can be used here.
}                   // inner's lifetime ends here.

// inner = 30;      // Error: the name is out of scope.
// outer is still alive and has value 30.
```

Braces form a **block**. An ordinary local variable such as `inner` has **automatic storage duration**: C++ handles its destruction when execution leaves the block. You do not call `delete` on it.

C++ defines four storage duration categories: automatic, static, thread, and dynamic. These are language rules; “stack” and “heap” are common implementation terms. See the [working draft's storage duration categories](https://eel.is/c++draft/basic.stc).

You can have storage without a live object in it, and you can keep a pointer after its target has died. The important question before accessing data is: **Does the object I am accessing still exist?**

## 6. The stack and function calls

### Metaphor first: interrupted tasks on a desk

You are doing homework and need to check a recipe. You put the homework aside and open the recipe. The recipe asks you to check a conversion chart, so you open that too.

You finish with the chart, return to the recipe, then return to the homework. Each unfinished task waits underneath the newer one.

### Actual explanation

A typical C++ implementation uses a **call stack** to keep track of active function calls. Each call usually has a **stack frame** containing information needed for that call, which may include local variables, saved registers, and a return address.

```cpp
#include <iostream>

void second() {
    int b = 20;
    std::cout << b << '\n';
}

void first() {
    int a = 10;
    second();
    std::cout << a << '\n';
}

int main() {
    first();
}
```

While `second()` runs, the conceptual stack is:

```text
Top:     second() — current call
         first()  — waiting for second()
Bottom:  main()   — waiting for first()
```

`second()` returns first. Then `first()` resumes and eventually returns to `main()`. The output is `20`, then `10`.

Ordinary local objects are commonly stored in stack frames. Their lifetimes end automatically when their enclosing blocks are exited. For local class objects, destruction normally happens in reverse order of completed construction.

There are two important qualifications:

- C++ guarantees the lifetime rules, not a particular physical stack layout. Optimisation can inline a function or keep a local value in a register.
- A local pointer's lifetime and its target's lifetime are separate. Destroying a pointer variable does not automatically destroy a dynamically allocated object it points to.

The call stack has a practical size limit. Very deep **recursion**—a function calling itself, directly or indirectly—or very large local arrays can exhaust it. This is commonly called a **stack overflow**. It is different from writing beyond the end of an array.

## 7. The heap and dynamic allocation

### Metaphor first: renting storage away from your desk

Your desk is useful for short tasks, but sometimes you need a rented storage unit. You can rent one while working and keep it after you leave the desk.

Leaving the desk does not cancel the rental. Someone must remain responsible for ending it.

### Actual explanation

**Dynamic allocation** obtains storage while the program runs. People commonly call the storage used for this the **heap** or **free store**. A dynamically allocated object's lifetime is not tied to the block containing a pointer to it.

Here is manual allocation, shown so you can understand it:

```cpp
int* number = new int{42}; // Create a dynamically allocated int.
*number = 50;             // Change that int.

delete number;            // Destroy the int and release its storage.
number = nullptr;         // Record that this pointer has no target now.
```

Read the first line in parts:

1. `new int{42}` obtains storage and initializes an `int` with value 42.
2. The expression returns a pointer to that object.
3. `int* number` declares a separate pointer object that stores that pointer value.

If this code is inside a function, `number` is an ordinary local variable, commonly on the stack. The `int` it points to has dynamic storage duration.

```text
Local pointer object                  Dynamically allocated object
+--------------------+                +--------------------------+
| number: an address | -------------> | int value: 42            |
+--------------------+                +--------------------------+
```

`delete number` does not delete the pointer variable `number`. It ends the target object's lifetime and releases its allocated storage. The pointer does not automatically become null, which is why the example assigns `nullptr` afterward.

Releasing storage lets the allocator reuse it. It does not promise to erase the old bytes or immediately reduce the memory usage reported by the operating system.

That assignment updates only `number`. If another pointer holds the same address, it still contains an invalid address for accessing the old object.

For arrays, allocation and deallocation forms must match:

```cpp
int* values = new int[3]{10, 20, 30};
int first = values[0]; // 10
delete[] values;      // Array new[] must be paired with delete[].
values = nullptr;
```

| How storage was obtained | Matching release |
| --- | --- |
| `new T` | `delete` |
| `new T[n]` | `delete[]` |
| C allocation functions such as `malloc` | `free` |

Do not mix these families. `malloc` and `free` are C functions; they do not themselves run C++ constructors or destructors.

Ordinary throwing `new` reports allocation failure with an exception such as `std::bad_alloc`. It does not normally return `nullptr`.

In everyday C++, prefer containers and smart pointers that handle release for you. Manual allocation is useful to understand, but it makes correctness depend on remembering cleanup along every possible path.

## 8. Stack, heap, and other storage: how they differ

### Metaphor first: temporary desks, rentals, and permanent cupboards

A temporary desk is cleared when a task ends. A rented unit stays rented until its responsible owner gives it back. A shared office cupboard is available throughout the working day.

Each has a different rule for how long storage remains available.

### Actual explanation

| Storage category | Example | What controls its duration? |
| --- | --- | --- |
| Automatic | Ordinary local `int x = 1;` | The enclosing block or function execution |
| Dynamic | The object created by `new int{1}` | Allocation and deallocation, often managed by an owner |
| Static | A global variable or local `static` | Storage lasts for the program's duration |
| Thread | A `thread_local` variable | Storage lasts for its thread's duration |

“Global” describes where a variable is declared. `static` can give a local variable persistent storage while its name remains local:

```cpp
int nextId() {
    static int id = 0; // Initialized once; retained between calls.
    return ++id;
}
```

Successive calls return `1`, `2`, then `3`. Leaving the function does not reset `id`.

The main practical comparison is:

| Question | Typical stack storage for locals | Typical heap allocation |
| --- | --- | --- |
| Cleanup | Automatic when the block exits | Requires a release operation, ideally performed by an owner |
| May the object outlive the creating call? | An ordinary local object cannot | Yes, if its lifetime is managed correctly |
| Allocation overhead | Usually small | Often higher; allocator bookkeeping is involved |
| Available size | Limited call stack | Often supports larger allocations, but is still limited |
| Access cost after allocation | Depends on the actual access pattern | Depends on the actual access pattern |

Heap storage is not automatically slow to read, and stack storage is not automatically correct. Lifetime, ownership, and how data is arranged matter more than a blanket “stack good, heap bad” rule.

Also, **dynamic** does not mean “automatically resizes.” A single allocation has a fixed size. A growable container such as a vector manages replacement allocations when it needs more room.

## 9. Pointers and references

### Metaphor first: an address card and a second name

A pointer is like a card containing a house's address. You can replace the address or leave the card blank. Reading the card is different from entering the house.

A reference is like giving the same house a second name. Both names identify the same house. Neither the card nor the extra name keeps the house standing.

### Actual explanation

A **pointer** is an object whose value can identify another object. A **reference** is an alias for an existing object.

```cpp
int score = 10;
int* pointer = &score;
int& reference = score;

*pointer = 20;  // Changes score to 20.
reference = 30; // Changes score to 30.
```

The symbols depend on context:

| Expression or declaration | Meaning |
| --- | --- |
| `int* pointer` | Declare a pointer to an `int` |
| `&score` | Obtain the address of `score` |
| `*pointer` | Access the pointed-to object; called dereferencing |
| `int& reference = score` | Bind a reference to `score` |

`pointer` is the address value. `*pointer` accesses the target. `&pointer` is the address of the pointer object itself.

```cpp
int other = 99;
pointer = &other; // Changes which object pointer points to.
*pointer = 100;   // Changes other.

reference = other; // Copies 100 into score; reference still names score.
```

A pointer can hold `nullptr`, meaning it points to no object:

```cpp
int* optional = nullptr;

if (optional != nullptr) {
    // This block is skipped.
    *optional = 5;
}
```

A valid ordinary reference must refer to an appropriate object; it has no equivalent empty state. A pointer can change targets, while an already bound reference cannot.

Neither raw pointers nor references automatically manage their targets' lifetimes. A pointer can point to a local variable, a global variable, an array element, or a dynamically allocated object. Seeing `*` in a declaration does not tell you “this came from the heap.”

## 10. Arrays, contiguous storage, and bounds

### Metaphor first: a row of assigned seats

You book three adjacent seats, numbered 0, 1, and 2. Moving one seat forward means moving by a whole seat, not one centimetre. Seat 3 is outside your booking, even if a seat happens to exist there.

### Actual explanation

An array stores elements of one type **contiguously**: each follows the previous element in memory.

```cpp
int values[3]{10, 20, 30};
int* first = &values[0];

int a = *first;       // 10
int b = *(first + 1); // 20
int c = values[2];   // 30
```

Adding one to an `int*` advances by one `int` element. If `sizeof(int)` is 4, the conceptual byte addresses might be:

```text
Element:        values[0]   values[1]   values[2]
Start address:  1000        1004        1008
Value:         10          20          30
```

These addresses are illustrative. Pointer arithmetic is defined within an array and up to **one past its end**. You may form `first + 3`, but you may not dereference it. Moving a pointer arbitrarily into unrelated memory is not valid array traversal.

The valid indices above are `0`, `1`, and `2`. Both `values[3]` and `values[-1]` are outside the array. Built-in array indexing does not perform a bounds check.

An array and a pointer are different things. In many expressions an array is converted to a pointer to its first element, but the pointer does not carry the array length:

```cpp
int values[3]{10, 20, 30};
int* pointer = values;

// sizeof(values) is 3 * sizeof(int).
// sizeof(pointer) is the size of the pointer itself.
```

For a fixed-size collection, `std::array<int, 3>` from `<array>` keeps the length as part of its type and provides `.size()` and checked `.at()` access. It stores its elements inside the array object; it does not need a separate heap allocation for them.

## 11. How vectors and strings use memory

### Metaphor first: a booking record and a rented hall

A booking record on your desk describes a rented hall: where it is, how many guests are present, and how many guests fit. The record is small even when the hall is large.

If the guests outgrow the hall, you book a larger hall, move them, and release the old one. An old invitation showing the previous location is now out of date.

### Actual explanation

A local `std::vector` object can have automatic storage duration while managing dynamically allocated element storage:

```cpp
#include <vector>

std::vector<int> scores{10, 20, 30};
```

The vector object keeps bookkeeping information about its elements and allocation. The element storage is separate from that object.

```text
Local vector object                    Allocated element storage
+-------------------------+            +----+----+----+-----+-----+
| bookkeeping about       | ---------> | 10 | 20 | 30 |     |     |
| storage, size, capacity |            +----+----+----+-----+-----+
+-------------------------+             three elements; spare capacity
```

This is a conceptual drawing, not a required representation. The diagram shows possible spare capacity; a three-element vector is not guaranteed to have any.

- **Size** is the number of existing elements.
- **Capacity** is the number of elements its storage can accommodate before reallocation is necessary.
- **Reallocation** replaces the element storage, moving or copying existing elements and releasing the old allocation.

```cpp
std::vector<int> scores{10, 20, 30};
scores.reserve(10); // Capacity is at least 10; size remains 3.
scores.push_back(40); // Size becomes 4.

// scores[7] = 99; // Invalid: element 7 does not exist.
scores.resize(8);  // Size becomes 8; new int elements start at zero.
scores[7] = 99;    // Now valid.
```

**Allocated capacity is not the same as constructed elements.** Stay within `.size()` when accessing elements. Use `.at(index)` when you want a bounds check; it throws `std::out_of_range` for an invalid index.

When reallocation happens, old pointers, references, and iterators to vector elements become invalid. An **iterator** is an object used to identify a position in a collection.

```cpp
std::vector<int> scores{10, 20, 30};
int* first = &scores[0];

// Request more capacity than the current allocation provides.
scores.reserve(scores.capacity() + 1);

// *first is no longer valid to access.
first = &scores[0]; // Obtain a fresh pointer after reallocation.
```

This example assumes the larger allocation succeeds. Other container operations can invalidate element access too; reallocation is one important case, not the only one.

`sizeof(scores)` measures the vector object itself. It does not include the separate allocation containing its elements. For a vector of `int`, an approximate element-storage calculation is `scores.capacity() * sizeof(int)`, excluding allocator overhead.

`std::string` also manages character storage. Many implementations keep short text directly inside the string object, called **small string optimisation**, and use dynamic storage for longer text. The threshold and representation are implementation details. Do not assume every string allocates separately.

When a vector or string is destroyed, it releases storage it owns automatically. That behaviour is an example of the next concept.

## 12. Constructors, destructors, and RAII

### Metaphor first: a tool case with an automatic return system

Imagine a tool case that checks out tools when you open it and returns them when you finish with the case. You can concentrate on the work because the case is responsible for its tools.

Even if you leave the job early, the return system still runs.

### Actual explanation

A **constructor** initializes a class object. A **destructor** performs the object's cleanup when its lifetime ends. A destructor has the class name preceded by `~`.

**RAII** stands for **Resource Acquisition Is Initialization**. It is the C++ pattern of making an object responsible for a resource, acquiring it during initialization and releasing it through destruction. Resources include allocated memory, open files, and locked mutexes.

Standard C++ does not provide a general tracing garbage collector that automatically finds unreachable allocations made with `new`. RAII instead ties cleanup to the lifetime of a responsible owner.

For ordinary local objects, C++ invokes destructors when their blocks are exited. This includes leaving by `return` and exception stack unwinding. An **exception** is a mechanism for reporting an error to a caller. Abrupt process termination is different; it does not guarantee this cleanup.

```cpp
#include <iostream>
#include <string>

struct Message {
    std::string text;

    explicit Message(const std::string& value) : text(value) {
        std::cout << "Constructed: " << text << '\n';
    }

    ~Message() {
        std::cout << "Destroyed: " << text << '\n';
    }
};
```

`struct` defines a class type whose members are public by default. `text(value)` initializes the member before the constructor body runs. You do not need to master `explicit` yet; here it prevents unintended implicit conversions into `Message`.

Inside a function:

```cpp
{
    Message first{"first"};
    Message second{"second"};
} // Destroys second, then first.
```

This prints construction in the order `first`, `second`, then destruction in the order `second`, `first`. The `std::string` members perform their own cleanup as part of destroying their containing objects.

You usually do not need to write a destructor yourself. Types such as `std::vector`, `std::string`, and smart pointers already implement resource management. Composing them into your own classes lets cleanup happen automatically. This is often called the **Rule of Zero**: avoid writing custom ownership-related special member functions when existing types can handle ownership.

The C++ Core Guidelines recommend automatic resource management through resource-owning objects. See [R.1: manage resources automatically using RAII](https://isocpp.github.io/CppCoreGuidelines/CppCoreGuidelines#Rr-raii).

## 13. Ownership and smart pointers

### Metaphor first: responsibility for a rented room

An address tells you where a room is. Ownership tells you who must return its key and end its rental.

With one responsible tenant, the arrangement is straightforward. Responsibility can be handed to someone else. With a shared rental, the room stays rented until the last responsible tenant gives up their share. A visitor can know the address without becoming a tenant.

### Actual explanation

**Ownership** means responsibility for a resource's lifetime. Before accessing a resource, you should be able to explain who keeps it alive and who eventually releases it.

A raw pointer such as `int*` does not encode ownership. A **smart pointer** is a class that stores pointer-like access while applying a lifetime-management policy. Include `<memory>` to use the standard smart pointers.

### One owner: metaphor first

There is one checkout card for a tool. You can hand the card to someone else, but you cannot duplicate the responsibility by photocopying it.

### One owner: actual explanation

`std::unique_ptr<T>` represents exclusive ownership. Destroying it destroys its owned object and releases the allocation. It cannot be copied, but ownership can be moved.

```cpp
#include <memory>
#include <utility>

auto first = std::make_unique<int>(42);
*first = 50;

// auto second = first; // Compile error: ownership cannot be copied.
auto second = std::move(first); // Transfer ownership to second.
// first is now empty; second owns the int with value 50.
```

`auto` asks the compiler to deduce a variable's type. Here the type is `std::unique_ptr<int>`. `std::make_unique<int>(42)` creates the `int` and returns its owning smart pointer.

`std::move` permits a move operation; it does not itself move bytes or perform the transfer. The `unique_ptr` move constructor performs this transfer.

Use unique ownership when you need an individually allocated object with one owner. For a simple local integer, `int value = 42;` is still simpler. Smart pointers are useful when dynamic allocation is actually needed.

### Shared owners: metaphor first

Several people share responsibility for a storage unit. Each holds a membership card. The unit is returned when the last membership ends. A visitor's pass lets someone check whether the unit is still available without keeping the rental active.

### Shared owners: actual explanation

`std::shared_ptr<T>` supports shared ownership. Copying it creates another owner of the **same object**; it does not copy that object. The managed object is destroyed when the last owning `shared_ptr` releases it.

```cpp
#include <memory>

auto first = std::make_shared<int>(42);
auto second = first; // Two owners of the same int.

first.reset();       // Releases first's ownership; int is still alive.
*second = 99;        // Valid: second still owns it.
second.reset();      // Last owner releases it; int is destroyed.
```

`std::weak_ptr<T>` observes an object managed by shared ownership without keeping that object alive. Call `.lock()` to try to obtain a `shared_ptr` before using it. An empty result means the managed object is no longer available.

If two objects own one another through `shared_ptr`s, they can form an ownership cycle that prevents automatic cleanup. Using `weak_ptr` for an appropriate observing relationship can break the cycle.

Shared ownership requires bookkeeping. Use it when the lifetime really has multiple independent owners, rather than as the default replacement for every pointer. A raw pointer or reference is often suitable for temporary, non-owning access to an object whose owner stays alive.

## 14. Copying, moving, and passing to functions

### Metaphor first: photocopying a notebook, borrowing it, or handing it over

A photocopy is a separate notebook. Borrowing lets you use the original. Handing it over transfers responsibility to another person.

Copying an address card is different from photocopying the notebook: two cards still lead to the same notebook.

### Actual explanation

Passing a parameter **by value** creates a parameter object. Passing an existing lvalue by value normally copies it. Passing **by reference** allows the function to use the caller's object without making that copy.

```cpp
#include <string>

void changeCopy(int value) {
    value = 99; // Changes this function's parameter only.
}

void changeOriginal(int& value) {
    value = 99; // Changes the caller's int.
}

std::size_t length(const std::string& text) {
    return text.size(); // Observe the caller's string without copying it.
}
```

`const std::string&` means a reference through which this function cannot modify the string. `std::size_t` is an unsigned integer type commonly used for object sizes and counts.

For ordinary value types, copying creates an independent value:

```cpp
#include <vector>

std::vector<int> original{1, 2, 3};
std::vector<int> copy = original;
copy[0] = 99; // original[0] remains 1.

int number = 10;
int* first = &number;
int* second = first; // Copies the pointer, not number.
*second = 20;        // number becomes 20; both pointers identify it.
```

The exact meaning of copying depends on the type. A vector of `int` copies its integer elements into independent storage. A vector of raw pointers copies pointer values, so its pointers still identify the same target objects.

**Moving** allows a type to transfer resources instead of duplicating them. Moving a vector can transfer its element allocation; it does not generally need to copy every integer to a new allocation.

For standard library types, a moved-from object generally remains valid, but its value may be unspecified unless a stronger guarantee applies. You can destroy or assign to it; do not assume it retains its old contents. A moved-from `unique_ptr` is specifically guaranteed to be empty.

Returning an owning value is normal C++:

```cpp
std::vector<int> makeScores() {
    return {10, 20, 30};
}
```

The caller receives a valid vector. Return-value construction and move support avoid the idea that you must return a pointer to a local object to get acceptable performance.

## 15. Common memory mistakes

### 15.1 Uninitialized values — metaphor first

You receive a reusable box but never put your own label inside. Opening it does not reliably tell you what your label should have said.

### Uninitialized values — actual explanation

An ordinary local built-in variable is not automatically initialized to a useful value:

```cpp
int unknown;         // No initialized int value to use.
// int x = unknown;  // Invalid: reads an uninitialized int.

int count{};            // Initialized to zero.
int* pointer = nullptr; // Initialized to the null pointer value.
```

In the C++17 examples here, reading an uninitialized ordinary `int` causes **undefined behavior**. That means the language puts no requirements on the resulting behaviour. It is more serious than getting an unpredictable number.

Initialize variables before reading them. Do not rely on a debug build or a previous run appearing to give you zero.

### 15.2 A memory leak — metaphor first

You rent a storage unit and lose the only record of it. The rental continues, but you no longer have the information needed to return it.

### A memory leak — actual explanation

A **memory leak** occurs when allocated memory is not released when it should be. A common cause is losing the last owning pointer:

```cpp
void leakExample() {
    int* pointer = new int{42};
    pointer = nullptr; // Loses the address; does not release the allocation.
}
```

This is a deliberately incorrect example. Leaving the function destroys the local pointer but does not release the allocated `int`. Repeating leaks can steadily increase memory consumption. Operating systems typically reclaim a process's memory when it exits, but that does not solve leaks in a running program.

A `unique_ptr` would retain responsibility for the allocation and release it on destruction.

### 15.3 Dangling pointers and use-after-free — metaphor first

You keep an address card after its house has been demolished. The street number is still written down, but it no longer gives you permission to enter the old house. A new building at the same address would not be the old house either.

### Dangling pointers and use-after-free — actual explanation

A **dangling pointer** refers to storage after its target object's lifetime has ended. Accessing the old object through it is invalid. When this happens after dynamic storage has been released, it is commonly called **use-after-free**.

```cpp
int* first = new int{42};
int* second = first;

delete first;
first = nullptr;

// int value = *second; // Invalid: second is dangling.
```

The address in `second` was not automatically updated. A null check on `second` cannot establish that the target is alive: a dangling pointer is not necessarily null.

References can dangle too. Do not return a pointer or reference to an ordinary local variable:

```cpp
int* badResult() {
    int local = 42;
    return &local; // Wrong: local dies when this function returns.
}

int goodResult() {
    int local = 42;
    return local; // Returns the value, not access to the destroyed local.
}
```

The `badResult` example is intentionally incorrect and should produce a compiler warning. Calling it does not keep `local` alive. The same problem occurs when returning `int&` bound to `local`.

### 15.4 Double deletion and invalid deletion — metaphor first

Two people each believe they hold the one checkout card and both try to return the same tool. Or someone tries to return a tool that was never checked out through that system.

### Double deletion and invalid deletion — actual explanation

An allocation must be released exactly once through the correct mechanism:

```cpp
int* first = new int{42};
int* second = first;
delete first;
// delete second; // Invalid: allocation has already been released.

int local = 10;
// delete &local; // Invalid: local was not created by new.
```

Likewise, do not manually delete the object owned by a smart pointer. The owner will try to clean it up later. Multiple raw pointers to one object are allowed; multiple independent claims of exclusive ownership are the problem.

Deleting a null pointer is safe, but that rule does not make dangling pointers safe to delete.

### 15.5 Out-of-bounds access — metaphor first

You rent three lockers and open a fourth because it is nearby. Its physical existence does not make it yours.

### Out-of-bounds access — actual explanation

Reading or writing outside an array's valid elements is undefined behavior:

```cpp
int values[3]{10, 20, 30};
// values[3] = 99; // Invalid: valid indices are 0, 1, and 2.
```

It might crash, corrupt unrelated data, or appear to work. A lack of an immediate crash does not establish correctness. For a vector or `std::array`, `.at()` is useful when you want checked access.

The rules apply even if the unwanted access stays inside a larger allocation. For example, spare vector capacity does not make nonexistent elements valid to access.

### 15.6 A non-owning view that outlives its data — metaphor first

You keep a window frame after the building behind it has disappeared. The frame cannot preserve the scene it once showed.

### A non-owning view that outlives its data — actual explanation

Some useful types describe data without owning it. For example, `std::string_view` from `<string_view>` refers to a character sequence; it does not keep a string alive.

```cpp
#include <string>
#include <string_view>

std::string text = "hello";
std::string_view view = text; // Borrows text's characters.
```

`view` is useful only while the referenced characters remain valid. Destroying `text`, or modifying it in a way that invalidates access to its storage, can invalidate the view. Copying a view copies the description, not the characters.

Use an owning `std::string` when the text must survive independently. The same lifetime question applies to raw pointers, references, and iterators: **Who owns the data, and will that owner keep it valid for this use?**

## 16. Alignment, padding, and access speed

### Metaphor first: furniture on a marked floor

Imagine a warehouse floor with marked positions for furniture. A large cabinet must start at a suitable mark. Placing a small stool before it can leave an empty gap so the cabinet starts in the right place.

Also, collecting supplies from neighbouring shelves is often quicker than visiting scattered shelves across the warehouse.

### Actual explanation

**Alignment** is a requirement on where an object may begin in memory. Use `alignof(T)` to inspect a type's alignment requirement. The compiler normally handles alignment for ordinary declarations and standard allocation.

**Padding** consists of extra storage used to satisfy layout requirements. A class or struct can therefore occupy more than the sum of its member sizes:

```cpp
struct Example {
    char letter;
    int number;
};

// sizeof(Example) might be 8 even if char is 1 byte and int is 4.
// The exact size and alignment depend on the implementation.
```

Do not calculate object layouts merely by adding member sizes, and do not assume every byte in an object is meaningful data you initialized.

Processors also use small, fast **caches** to keep recently used memory nearby. Accessing neighbouring elements, as in a vector, often uses caches efficiently. Following pointers through scattered allocations can require more separate memory fetches.

This is why memory performance depends on layout and access patterns as well as allocation costs. For now, concentrate on correct lifetimes and clear ownership. Measure performance before introducing complicated memory management.

## 17. A complete program: watch lifetimes happen

### Metaphor first: visitors and an independently rented room

An outer visitor arrives. A helper arranges a rented room and hands over responsibility for it. An inner visitor arrives for a shorter meeting, then leaves. The room remains available until its responsible owner leaves, and the outer visitor leaves last.

### Actual explanation

Save the following as `memory_demo.cpp`. It shows local object lifetimes, pointer and reference access, vector ownership, and transferring an owning pointer from a function.

```cpp
#include <iostream>
#include <memory>
#include <string>
#include <vector>

struct Message {
    std::string text;

    explicit Message(const std::string& value) : text(value) {
        std::cout << "Constructed: " << text << '\n';
    }

    ~Message() {
        std::cout << "Destroyed: " << text << '\n';
    }
};

std::unique_ptr<Message> makeMessage() {
    return std::make_unique<Message>("dynamic");
}

int main() {
    Message outer{"outer"};
    auto owner = makeMessage();

    {
        Message inner{"inner"};

        int score = 10;
        int* pointer = &score;
        int& reference = score;
        *pointer = 20;
        reference = 30;
        std::cout << "score: " << score << '\n';

        std::vector<int> scores{10, 20};
        scores.push_back(30);
        std::cout << "scores:";
        for (int value : scores) {
            std::cout << ' ' << value;
        }
        std::cout << '\n';
    }

    // -> accesses a member through a pointer or smart pointer.
    std::cout << "Still alive: " << owner->text << '\n';
}
```

On a system with GCC installed, compile and run it from the directory containing the file:

```sh
g++ -std=c++17 -Wall -Wextra -Wpedantic -g memory_demo.cpp -o memory_demo
./memory_demo
```

Expected output:

```text
Constructed: outer
Constructed: dynamic
Constructed: inner
score: 30
scores: 10 20 30
Destroyed: inner
Still alive: dynamic
Destroyed: dynamic
Destroyed: outer
```

Notice the sequence:

1. `makeMessage()` finishes, but the dynamically allocated `Message` stays alive because ownership reaches the caller.
2. `pointer` and `reference` access the same `score`, so the final score is 30.
3. Leaving the inner block destroys its vector, which releases its element storage. The inner `Message` is also destroyed.
4. The dynamic `Message` remains available through `owner`.
5. At the end of `main()`, `owner` is destroyed before `outer`, because it was constructed later. Its destructor destroys the dynamic `Message`.

You can also build this example with runtime checks:

```sh
g++ -std=c++17 -Wall -Wextra -Wpedantic -g -O1 -fsanitize=address,undefined -fno-omit-frame-pointer memory_demo.cpp -o memory_demo
./memory_demo
```

**AddressSanitizer** detects many invalid memory accesses, and **UndefinedBehaviorSanitizer** detects certain other invalid operations. These tools help find mistakes, but they do not detect every lifetime or bounds error. Compiler warnings are useful too; read and resolve them rather than treating a successful build as proof of correctness.

## 18. Practice: explain it back to yourself

### Metaphor first: checking a map before travelling

Recognising a map is easier than navigating from it. Before running an example, predict the route. Then compare what happens with your prediction.

### Actual explanation

Try these questions without running the snippets. None of the invalid operations shown in comments should be enabled just to see whether they crash.

**Question 1: what changes?**

```cpp
int a = 5;
int b = a;
int* pointer = &a;
b = 10;
*pointer = 20;
```

What are the final values of `a` and `b`?

**Question 2: which object survives?**

```cpp
auto owner = std::make_unique<int>(7);
{
    int* observer = owner.get();
    *observer = 8;
}
// Is the int still alive here? What value does it hold?
```

`.get()` obtains a non-owning raw pointer without transferring ownership.

**Question 3: is the index valid?**

```cpp
std::vector<int> values;
values.reserve(100);
// Would values[0] = 5; be valid here?
```

**Question 4: what is copied?**

```cpp
auto first = std::make_shared<int>(5);
auto second = first;
*second = 9;
// What does *first read? How many int objects were created?
```

**Question 5: which order?**

Items A, B, and C arrive in that order. What removal order does FIFO use? What removal order does LIFO use?

**Question 6: why is this dangerous?**

You keep a pointer to a vector element, then grow the vector until it reallocates. Can you still use the old pointer?

### Answers

1. `a` is 20; `b` is 10. Copying the integer created an independent value, while dereferencing the pointer changed `a`.
2. Yes. The integer holds 8. The inner pointer was only an observer; the outer `unique_ptr` is still its owner.
3. No. Capacity is at least 100, but size is zero. Add an element with `push_back`, or create elements with `resize`, before accessing them.
4. `*first` reads 9. There is one managed integer and two shared owners of it.
5. FIFO: A, B, C. LIFO: C, B, A.
6. No. Reallocation invalidates the old pointer. Obtain fresh access to the element after the operation succeeds.

### Quick reference

| Term | Meaning |
| --- | --- |
| Byte | A basic addressable unit of storage |
| Object | A region of storage used to represent a value of a type |
| Address | A location identifying storage |
| Pointer | An object that can hold an address or a null pointer value |
| Reference | An alias for an existing object |
| Scope | Where a name can be used in source code |
| Lifetime | When an object exists during execution |
| Stack | Common mechanism for tracking nested function calls and local storage |
| Heap | Common term for storage used by dynamic allocations |
| Ownership | Responsibility for a resource's lifetime and release |
| RAII | Managing a resource through an owning object's construction and destruction |
| Leak | Allocated memory that is not released when it should be |
| Dangling access | Access through a pointer, reference, or view whose target is no longer valid |
| Undefined behavior | An operation for which C++ imposes no requirements on the result |

When reading C++ code, practise asking: **What object is this? Who owns it? When does its lifetime end? Is this access still valid?**

For more practice with the same ideas, continue with [References and Pointers](ReferencesAndPointers.md), [std::vector](StdVectors.md), and [C Strings](CStrings.md).
