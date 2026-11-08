// Comeback and make the parsing function 

#include <iostream>
#include <vector>
#include <string>
#include <sstream>
#include <limits>

double add(const std::vector<double> &nums);
double subtract(const std::vector<double> &nums);
double multiple(const std::vector<double> &nums);
double divide(const std::vector<double> &numbers);
std::vector<double> numbers_loop();
char operation_loop();
double parsing(const char &operation, const std::vector<double> &nums);

int main(){
    char operation;
    std::string temp;
    std::vector<double> numbers;

    operation = operation_loop();

    numbers = numbers_loop();

    std::cout << "Answer: " << parsing(operation, numbers) << std::endl;

    return 0;
}

double parsing(const char &operation, const std::vector<double> &nums){
    double output = 0.0;
    
    switch (operation){
        case '+': 
            output = add(nums);
        break;

        case '-':
            output = subtract(nums);
        break;

        case '*':
            output = multiple(nums);
        break;
    
        case '/':
            output = divide(nums);
        break;
    }
    return output;
}

char operation_loop(){
    char operation;

    std::cout << "Enter your operation: ";
    std::cin >> operation;

    std::cin.ignore(std::numeric_limits<std::streamsize>::max(), '\n');

    return operation;

}

std::vector<double> numbers_loop(){
    std::string temp;
    std::vector<double> numbers;
    
    while(true){
        std::cout << "Enter the number (q to quit): ";

        std::getline(std::cin, temp);

        if(temp == "q" || temp == "Q"){
            if(numbers.size() < 2){
                std::cout << "Must enter at least 2 numbers" << std::endl;
                continue;
            }
            else{
                break;
            }
            
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

    return numbers;
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