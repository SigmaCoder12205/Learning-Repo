#include <cstdint>
#include <cstdlib>
#include <iostream>

int main(int argc, char** argv) {
    const std::uint64_t iterations = argc > 1 ? std::strtoull(argv[1], nullptr, 10) : 10000000;
    std::uint64_t checksum = 1;
    for (std::uint64_t i = 0; i < iterations; ++i)
        checksum = checksum * 6364136223846793005ULL + i;
    std::cout << checksum << '\n';
}
