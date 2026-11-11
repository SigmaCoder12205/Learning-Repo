// READ COMEBACK.TXT

#include <iostream>


double convert(const char &original, const char &convert_to, const double &original_value, const double &convert_to_value);
double formulac(const double &original, const double &convert_to);
double formulaf(const double &original, const double &convert_to);
double formulak(const double &original, const double &convert_to);


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
            break;
        }
        else{
            std::cout << "Invaild temp type try again" << std::endl;
            continue;
        }
    }

    while(true){
        std::cout << "Enter the convert type temperature: ";
        std::cin >> convert_type;

        if(convert_type == 'C' || convert_type == 'K' || convert_type == 'F'){
            std::cout << "Enter the convert type temperature: ";
            std::cin >> convert_temp;
            break;
        }
        else{
            std::cout << "Invaild type try again" << std::endl;
            continue;
        }
    }

    convert(temp_type, convert_type, temp, convert_temp);

    return 0;
}

double convert(const char &original, const char &convert_to, const double &original_value,
               const double &convert_to_value){


    return 0.0;
}

double formulac(const double &original, const double &convert_to){

    return 0.0;
}

double formulaf(const double &original, const double &convert_to){

    return 0.0;
}

double formulak(const double &original, const double &convert_to){

    return 0.0;
}
