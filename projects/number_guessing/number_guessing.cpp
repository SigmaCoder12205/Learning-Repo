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
#include <cstdlib>
#include <ctime>

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
    int count = 0;
    int rand_num = generate(max, min);
    std::string string_return = "";

    while(true){
        std::cout << "Enter a number to guess (q to quit): ";

        if(!(std::cin >> temp)){
            break;
        }

        if(temp == "q" || temp == "Q"){
            break;
        }

        DynamicValue value = parseInput(temp);

        if(!isAllowed<int, double>(value)){
            std::cout << "Invaild input try again" << std::endl;
            continue;
        }

        if(std::holds_alternative<int>(value)){ // Find a way to turn this into a function
           int int_value = std::get<int>(value);
            user = int_value;
        }
        else if(std::holds_alternative<double>(value)){
            double double_value = std::floor(std::get<double>(value));

            if(double_value < min || double_value > max){
                std::cout << "Can't enter anything over " << max << " or below " << min << std::endl;
                continue;
            }

            user = static_cast<int>(double_value);
        }

        string_return = checks(user, max, min);

        if(!string_return.empty()){
            std::cout << string_return << std::endl;
            continue;
        }

        count++;

        if(user == rand_num){
            std::cout << "Correct the number was " << rand_num << std::endl;
            std::cout << "You won in " << count << " attempts" << std::endl;
            break;
        }    
        else if(user < rand_num){
            std::cout << "Too low" << std::endl;
            continue;
        }
        else if(user > rand_num){
            std::cout << "Too high" << std::endl;
            continue;
        }
    }

    return 0;
}

int generate(int max, int min){
    return (rand() % (max - min + 1)) + min;
}

std::string checks(const int &user, const int &max, const int &min){
    std::string return_output = "";
    if(user > max){
       return_output = "Can't enter anything over " + std::to_string(max); 
    }
    else if(user < min){
        return_output = "Can't enter anything below " + std::to_string(min); 
    }
    return return_output;
}

