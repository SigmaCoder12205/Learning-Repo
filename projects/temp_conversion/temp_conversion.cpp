// Run code and you'll see the errror or bug


#include <iostream>
#include <cctype>
#include <string>
#include <variant>
#include <sstream>
#include <limits>

std::variant<double, std::string> convert(const char &original, const char &convert_to, const double &original_value);
std::variant<double, std::string> formulac(const double &original, const char &convert_type);
std::variant<double, std::string>  formulaf(const double &original, const char &convert_type);
std::variant<double, std::string> formulak(const double &original, const char &convert_type);

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
    char temp_type;
    char convert_type;
    double temp;
    std::variant<double, std::string> results;
    char upper_temp_type;
    char upper_convert_type;

    while(true){ // Try and add type checks to make sure all types are chars
        std::string placeholder;
        std::cout << "Enter your current temperature type (C, F, K): ";
        std::cin >> placeholder;

        if(placeholder.length() > 1){
            std::cout << "Can't enter any text" << std::endl;
            continue;
        }
    
        DynamicValue val = parseInput(placeholder);
        
        if(std::holds_alternative<int>(val)){
            std::cout << "Can't enter any numbers" << std::endl;
            continue;
        }
        else if(std::holds_alternative<double>(val)){
            std::cout << "Can't enter any numbers" << std::endl;
            continue;
        }
        else if(std::holds_alternative<char>(val)){
            char char_value = std::get<char>(val);
            temp_type = char_value;
            upper_temp_type = static_cast<char>(std::toupper(static_cast<unsigned char>(temp_type)));
            
            // Write the check for if it's not equal to C, K, F

            if(upper_temp_type != 'C' && upper_temp_type != 'K' && upper_temp_type != 'F'){
                std::cout << "Can only enter C, K or F" << std::endl;
                continue;
            }
            break;
        }
        else if(std::holds_alternative<std::string>(val)){
            char char_val1 = placeholder.at(0);
            temp_type = char_val1;
            upper_temp_type = static_cast<char>(std::toupper(static_cast<unsigned char>(temp_type)));

            if(upper_temp_type != 'C' && upper_temp_type != 'K' && upper_temp_type != 'F'){
                std::cout << "Can only enter C, K or F" << std::endl;
                continue;
            }
            break;
           
        }

    }

    std::cin.ignore(std::numeric_limits<std::streamsize>::max(), '\n');
    while(true){
        if(upper_temp_type == 'C' || upper_temp_type == 'K' || upper_temp_type == 'F'){
            std::string raw;
            std::cout << "Enter the temperature: ";
            std::getline(std::cin, raw);

            DynamicValue value = parseInput(raw);

            if(std::holds_alternative<int>(value)){
                int int_val = std::get<int>(value);
                temp = int_val;
                break;
            }
            else if(std::holds_alternative<double>(value)){
                double double_val = std::get<double>(value);
                temp = double_val;
                break;
            }
            else if(std::holds_alternative<char>(value)){
                std::cout << "Invaild input try again" << std::endl;
                continue;
            }
            else if(std::holds_alternative<std::string>(value)){
                std::string string_val = std::get<std::string>(value);
                std::cout << "Invaild input try again" << std::endl;
                continue;
            }
        }
        else{
            std::cout << "Invaild temp type try again" << std::endl;
            continue;
        }
    }

    while(true){ // Keep working on nested while loop (way to remember: I was trying to find a way to exit a if statement)
        std::string temp1 = "";
        std::cout << "Enter the convert type: ";
        std::cin >> temp1;
        
        DynamicValue val1 = parseInput(temp1);

        if(std::holds_alternative<int>(val1)){
            std::cout << "Can't enter any numbers" << std::endl;
            continue;   
        }
        else if(std::holds_alternative<double>(val1)){
            std::cout << "Can't enter any numbers" << std::endl;
            continue;
        }
        else if(std::holds_alternative<char>(val1)){
            char char_val1 = std::get<char>(val1);
            convert_type = char_val1;
            upper_convert_type = static_cast<char>(std::toupper(static_cast<unsigned char>(convert_type)));
        }
        else if(std::holds_alternative<std::string>(val1)){
            if(temp1.length() > 1){
                std::cout << "Can't enter any text" << std::endl;
                continue;
            }
            else{
                char char_val2 = temp1.at(0);
                convert_type = char_val2;
                upper_convert_type = static_cast<char>(std::toupper(static_cast<unsigned char>(convert_type)));
            }
        }


        if(upper_convert_type == 'C' || upper_convert_type == 'K' || upper_convert_type == 'F'){
            break;
        }
        else{
            std::cout << "Invaild choice" << std::endl;
            continue;
        }
    }

    results = convert(upper_temp_type, upper_convert_type, temp);
    std::cout << "Answer: ";
    std::visit([](const auto& result) {std::cout << result << std::endl;}, results);
}


std::variant<double, std::string> convert(const char &original, const char &convert_to, const double &original_value){
    std::variant<double, std::string> answer = 0.0;
    switch (original){
        case 'C': answer = formulac(original_value, convert_to); break;
        case 'F': answer = formulaf(original_value, convert_to); break;
        case 'K': answer = formulak(original_value, convert_to); break;
        default: return -.01; // -.01 meaning original isn't any of those options
    }
    return answer;
}

std::variant<double, std::string> formulac(const double &original, const char &convert_type){
    double answer = 0.0;
    switch (convert_type){
        case 'F': answer = original * 1.8 + 32; break;
        case 'K': answer = original + 273.15; break;
        case 'C': answer =  original; break;
        default: answer = -.01;
    }

    if(convert_type == 'K' && answer <= 0){
        return "Absoloute Zero";     
    }

    return answer;
}

std::variant<double, std::string> formulaf(const double &original, const char &convert_type){
    double answer = 0.0;
    switch (convert_type){
        case 'C': answer = (original-32)/1.8; break;
        case 'K': answer = ((original-32)/1.8)+273.15; break;
        case 'F': answer = original; break;
        default: answer = -.01;
    }

    if(convert_type == 'K' && answer <= 0){
        return "Absoloute Zero";
    }

    return answer;
}

std::variant<double, std::string> formulak(const double &original, const char &convert_type){
    double answer = 0.0;
    switch (convert_type){
        case 'C': answer = original - 273.15; break;
        case 'F': answer = ((original - 273.15) * 1.8) + 32; break;
        case 'K': answer = original; break;
        default: answer = -.01;
    }

    if(answer <= 0){
        return "Absoloute Zero";
    }

    return answer;
}
