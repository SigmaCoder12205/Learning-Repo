// Plan
// Comeback: Seperate the code into functions and other files like (Dynamic Value) so it's a useable picece of code
//           I don't want to always add it to the top of my file
// Features to add: count of goes, settings etc 

#include <iostream>
#include <cctype>
#include <string>
#include <variant>
#include <sstream>
#include <cmath>
#include "../dynamic_value.h"

int generate(int max, int min = 0);

int main(){
    srand(time(0));
    int user = 0;
    std::string temp;
    int max = 100;
    int min = 0;
    int rand_num = generate(max, min);

    std::cout << rand_num << std::endl;
    std::cout << "Enter a number to guess: ";
    std::cin >> temp;

    DynamicValue value = parseInput(temp);

    if(std::holds_alternative<int>(value)){ // Find a way to turn this into a function
        int int_value = std::get<int>(value);
        user = int_value;
    }
    else if(std::holds_alternative<double>(value)){
        double double_value = std::get<double>(value);
        user = std::round(double_value);
    }
    else{
        std::cout << "Invaild try again" << std::endl; // Bad error line fix it 
    }

    if(user > max){
       std::cout << "Can't enter anything over " << max << std::endl; 
    }
    else if(user < min){
        std::cout << "Can't enter anything below " << min << std::endl;
    }

    if(user == rand_num){
        std::cout << "Correct the number was " << rand_num << std::endl;
    }    
    else if(user < rand_num){
        std::cout << "Too low" << std::endl;
    }
    else if(user > rand_num){
        std::cout << "Too high" << std::endl;
    }

    return 0;
}

int generate(int max, int min){
    return (rand() % (max - min + 1)) + min;
}