// Plan
// Comeback: Seperate the code into functions and other files like (Dynamic Value) so it's a useable picece of code
//           I don't want to always add it to the top of my file
// Features to add: count of goes, settings etc 

// LOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOK AT THIS: add in the check_dynamic_value to code and replace std::holdsalternative

#include <iostream>
#include <cctype>
#include <string>
#include <variant>
#include <sstream>
#include <cmath>
#include "../dynamic_value.h"
#include "../check_dynamic_value.h"

int generate(int max, int min = 0);
std::string checks(const int &user, const int &max, const int &min);

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

    if(!isAllowed<int, double>(value)){
        std::cout << "Invaild input try again" << std::endl;
    }

    if(std::holds_alternative<int>(value)){ // Find a way to turn this into a function
        int int_value = std::get<int>(value);
        user = int_value;
    }
    else if(std::holds_alternative<double>(value)){
        double double_value = std::get<double>(value);
        user = std::round(double_value);
    }

    checks(user, max, min);

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

std::string checks(const int &user, const int &max, const int &min){
    std::string return_output = "";
    if(user > max){
       return_output = "Can't enter anything over " + max; 
    }
    else if(user < min){
        std::cout << "Can't enter anything below " << min << std::endl;
        return_output = "Can't enter anything below " + min; 
    }
    return return_output;
}