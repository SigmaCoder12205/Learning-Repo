#include "dynamic_value.h"
#include <cctype>
#include <string>
#include <variant>
#include <sstream>

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