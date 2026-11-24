#pragma once 

#include <variant>
#include "dynamic_value.h"

template<typename ... AllowedTypes>
bool isAllowed(const DynamicValue &value){
    return (std::holds_alternative<AllowedTypes>(value) || ...);
}