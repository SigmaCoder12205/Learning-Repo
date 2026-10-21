#include <iostream>
int main(){
    std::string currencey;
    double amount;
    double finalAmount;
    std::cout << "Enter your currencey: ";
    std::cin >> currencey;
    std::cout << "Enter the amount: ";
    std::cin >> amount;
    finalAmount = amount * 1.42;
    std::cout << "AUD: " << finalAmount << std::endl;
    return 0;
}