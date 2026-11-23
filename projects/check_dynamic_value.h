#pragma once

#include <vector>
#include <variant>

void check(std::vector<std::string> allowed, std::variant<int, double, char, std::string> value);