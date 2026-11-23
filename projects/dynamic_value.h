#pragma once
#include <variant>
#include <string>

using DynamicValue = std::variant<int, double, char, std::string>;

DynamicValue parseInput(const std::string &input);