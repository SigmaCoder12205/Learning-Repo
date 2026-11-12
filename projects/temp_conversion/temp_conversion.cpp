// Formulas 
// Celisus to Farienhiet: F = (C x 1.8) +32

#include <iostream>


double convert(const char &original, const char &convert_to, const double &original_value, const double &convert_to_value);
double formulac(const double &original, const double &convert_to, const char &convert_type);
double formulaf(const double &original, const char &convert_type);
double formulak(const double &original, const char &convert_type);


int main(){
    char temp_type;
    char convert_type;
    int temp;
    int convert_temp;

    while(true){ // Try and add type checks to make sure all types are chars
        std::cout << "Enter your current temperature type (C, F, K): ";
        std::cin >> temp_type;

        if(temp_type == 'C' || temp_type == 'K' || temp_type == 'F'){
            std::cout << "Enter the temperature: ";
            std::cin >> temp;
        }
        else{
            std::cout << "Invaild temp type try again" << std::endl;
            continue;
        }

        std::cout << "Enter the convert type: ";
        std::cin >> convert_type;

        if(convert_type == 'C' || convert_type == 'K' || convert_type == 'F'){
            break;
        }
        else{
            std::cout << "Invaild choice restart" << std::endl;
            continue;
        }
    }
}


double convert(const char &original, const char &convert_to, const double &original_value){

    switch (original){
    case 'C': formulac(original_value,  convert_to); break;
    case 'F': formulaf(original_value, convert_to); break;
    case 'K': formulak(original_value, convert_to); break;
    }

    return 0.0;
}

double formulac(const double &original, const char &convert_type){
    switch (convert_type){
    case 'F': return original * 1.8 + 32;
    case 'K': return original + 273.15;
    }
}

double formulaf(const double &original, const char &convert_type){
    switch (convert_type){
        case 'C': return (original-32)/1.8;
        case 'K': return ((original-32)/1.8)+273.15;
    }

    return 0.0;
}

double formulak(const double &original, const char &convert_type){
    switch (convert_type){
        case 'C': break;
        case 'F': break;
    }
    return 0.0;
}
