#include <iostream>
#include <fstream>

int main() {
    // Opens a new file (or wipes an existing one clean)
    std::ofstream csvFile("/home/arch/Projects/Learning-Repo/code/data/test.csv");

    if (!csvFile.is_open()) {
        std::cerr << "Error opening file!" << std::endl;
        return 1;
    }

    // 1. Write the Header Row
    csvFile << "ID,Name,Language,City\n";

    // 2. Write Data Rows
    csvFile << "1,Archer,C++,Sydney\n";
    csvFile << "2,Linus,C,\"Portland, Oregon\"\n"; // Note: Quotes used because City contains a comma

    csvFile.close();
    std::cout << "New CSV created successfully." << std::endl;
    return 0;
}
