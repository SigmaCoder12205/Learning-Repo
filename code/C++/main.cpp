#include <iostream>
#include <vector>

int main(){
    std::vector<int> numbers;

    numbers.push_back(10);
    numbers.push_back(20);

    std::cout << numbers.empty() << std::endl;

    return 0;
}
