#include <iostream>
#include <typeinfo>

inline int square(int x) { return x * x; }


int main(){
    unsigned int x = -10;

    std::cout << x << std::endl;
    return 0;
}
