#include <iostream>



int main(){
    int *ptr = new int(20); // On the heap

    std::cout << *ptr << std::endl;

    delete ptr;
    return 0;
}
