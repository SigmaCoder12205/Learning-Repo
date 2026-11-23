#include <check_dynamic_value.h>

#include <vector>
#include <iostream>
#include <variant>

void check(std::vector<std::string> allowed, std::variant<int, double, char, std::string> value){
    for(const auto &type : allowed){
           
    }
}