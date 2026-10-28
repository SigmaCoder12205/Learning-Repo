#include <iostream>
#include <fstream>

int main(){
    std::ifstream inputFile("/home/arch/Projects/Learning-Repo/code/data/test.txt");

    if(!inputFile.is_open()){
        std::cout << "Could not open the file" << std::endl;
        return 1;
    }

    std::string line;

    while(std::getline(inputFile, line)){
        std::cout << line << std::endl;
    }

    inputFile.close();

    return 0;
}
