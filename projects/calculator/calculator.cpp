// Turn the while loop into a function 
// Then fix if the user enters less than 2 numbers in subtraction and other files 

#include <iostream>
#include <vector>
#include <string>
#include <sstream>
#include <limits>
#include <type_traits>

double add(const std::vector<double> &nums);
double subtract(const std::vector<double> &nums);
double multiple(const std::vector<double> &nums);
double divide(const std::vector<double> &numbers);

int main(){
    char operation;
    bool running = true;
    bool redo = false;
    std::string temp;
    std::vector<double> numbers;

    std::cout << "Enter your operation: ";
    std::cin >> operation;

    std::cin.ignore(std::numeric_limits<std::streamsize>::max(), '\n');

    while(running){
        std::cout << "Enter the number (q to quit): ";

        std::getline(std::cin, temp);

        if(temp == "q" || temp == "Q"){
            break;
        }

        std::stringstream ss(temp);

        ss.str(temp);
        double value;

        if(ss >> value && ss.eof()){  
            numbers.push_back(value);
        }
        else{
            std::cout << "Invaild type try again" << std::endl;
        }
    }

    switch (operation){
    case '+': 
        std::cout << add(numbers) << std::endl;
    break;

    case '-':

        if(numbers.size() < 2){
            std::cout << "Must enter more than 2 numbers" << std::endl;
            redo = true;
            break;
        }
        else{
            std::cout << subtract(numbers) << std::endl;
            break;
        }

    case '*':
        std::cout << multiple(numbers) << std::endl;
    break;
    
    case '/':
        if(numbers.size() < 2){
            std::cout << "Must enter more than 2 numbers" << std::endl;
            break;
        }
        else{
            std::cout << divide(numbers) << std::endl;
            break;
        }

    default:
        break;
    }

    return 0;
}

double add(const std::vector<double> &nums){
    double total = 0;

    for(int i = 0; i < nums.size(); i++){
        total += nums[i];
    }

    return total;
}

double subtract(const std::vector<double> &nums){
    double total = 0;


    total = nums[0] - nums[1];

    for(int i = 2; i < nums.size(); i++){
        total -= nums[i];
    }

    return total;
}

double multiple(const std::vector<double> &nums){
    double total = 1;

    for(int i = 0; i < nums.size(); i++){
        total *= nums[i];
    }

    return total;
}

double divide(const std::vector<double> &nums){
    double total = 0;

    total = nums[0] / nums[1];

    for(int i = 2; i< nums.size(); i++){
        if(nums[i] == 0){
            continue;
        }

        total /= nums[i];
    }

    return total;
}