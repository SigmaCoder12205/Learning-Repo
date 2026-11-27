// bench validator in C++: stdin (the target's stdout) must be whitespace-separated
// integers in nondecreasing order. Build once, then pass the binary to --validate:
//   g++ -O2 -std=c++17 examples/validators/check_sorted.cpp -o build/check_sorted
//   bench ./sorter --validate build/check_sorted
#include <iostream>

int main() {
    long long previous = 0, value = 0, count = 0;
    while (std::cin >> value) {
        if (count > 0 && value < previous) {
            std::cout << "item " << count << " (" << value << ") is smaller than " << previous << "\n";
            return 1;
        }
        previous = value;
        ++count;
    }
    if (!std::cin.eof()) {
        std::cout << "stdout contains a non-integer after item " << count << "\n";
        return 1;
    }
    std::cout << "bench-metric items " << count << " count\n";
    return 0;
}
