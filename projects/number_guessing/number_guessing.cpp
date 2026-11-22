// Plan
// try to have as little code in main as possible 
// meaning set most of the code into functions 
// Features to add: count of goes, settings etc 

#include <iostream>
#include <cctype>
#include <string>
#include <variant>
#include <sstream>
#include <cmath>

int generate();

using DynamicValue = std::variant<int, double, char, std::string>;

DynamicValue parseInput(const std::string &input){
    if(input.length() == 1 && !std::isdigit(static_cast<unsigned char>(input[0]))){
        return input[0];
    }

    std::istringstream intStream(input);
    int int_value;
    
    if((intStream >> int_value) && (intStream >> std::ws).eof()){
        return int_value;
    }

    std::istringstream doubleStream(input);
    double double_value;

    if((doubleStream >> double_value) && (doubleStream >> std::ws).eof()){
        return double_value;
    }

    return input;
}

int main(){
    srand(time(0));
    int user;
    std::string temp;
    int max = 100;
    int min = 0;
    int rand_num = generate(max, min);

    std::cout << rand_num << std::endl;
    std::cout << "Enter a number to guess: ";
    std::cin >> temp;

    DynamicValue value = parseInput(temp);

    if(std::holds_alternative<int>(value)){
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

int generate(int max, int min = 0){
    return (rand() % (max - min + 1)) + min;
}