#include <iostream>

void makeTable();


int main(){
    int *ptr = new int(20); // On the heap

    delete ptr; // clears the space you were givin

    return 0;
}

void makeTable(){
    int nails = 9; // On the stack
}