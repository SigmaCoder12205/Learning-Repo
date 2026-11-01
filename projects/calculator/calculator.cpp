#include <iostream>
#include <vector>
#include <string>
#include <sstream>
#include <limits>

void add();
void subtract();
void multiple();
void divide();

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

    for(auto value : numbers){
        std::cout << value << std::endl;
    }

    return 0;
}