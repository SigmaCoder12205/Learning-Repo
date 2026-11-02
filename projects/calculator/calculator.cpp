#include <iostream>
#include <vector>
#include <string>
#include <sstream>
#include <limits>

double add(std::vector<double> numbers);
double subtract(std::vector<double> numbers);
double multiple(std::vector<double> numbers);
double divide(std::vector<double> numbers);

int main(){
    char operation;
    bool running = true;
    double num;
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
            running = false;
            break;
        }
    }

    switch (operation){
    case '+': 
        std::cout <<add(numbers) << std::endl;
    break;

    case '-':
        std::cout << subtract(numbers) << std::endl;
    break;

    case '*':
        std::cout << multiple(numbers) << std::endl;
    break;
    
    case '/':
        std::cout << divide(numbers) << std::endl;
    break;

    default:
        break;
    }

    return 0;
}

double add(std::vector<double> numbers){
    double total;

    for(int i = 0; i < numbers.size(); i++){
        total += numbers[i];
    }

    return total;
}

double subtract(std::vector<double> numbers){
    double total;

    total = numbers[0] - numbers[1];

    for(int i = 2; i < numbers.size(); i++){
        total -= numbers[i];
    }

    return total;
}

double multiple(std::vector<double> numbers){
    double total = 1;

    for(int i = 0; i < numbers.size(); i++){
        total *= numbers[i];
    }

    return total;
}

double divide(std::vector<double> numbers){
    double total;

    total = numbers[0] / numbers[1];

    for(int i = 2; i< numbers.size(); i++){
        total /= numbers[i];
    }

    return total;
}