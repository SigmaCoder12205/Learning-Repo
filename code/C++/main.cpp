#define _USE_MATH_DEFINES
#include <iostream>
#include <cmath>

class Shape{
    public:
        double area;
        double volume;
};

class Cube : public Shape{
    public:
        double side;
    
    Cube(double side){
        this->side = side;
        this->area = side * side * 6;
        this->volume = pow(side, 3);
    }
};

class Sphere : public Shape{
    public: 
        double radius;

    Sphere(double radius){
        this->radius = radius;
        this->area = 4 * M_PI * (radius * radius);
        this->volume = (4 / 3.0) * M_PI * pow(radius, 3);
    }
};

int main(){
    Cube cube(10);    
    Sphere sphere(5);

    std::cout << "Area: " << sphere.area << "cm" << std::endl;
    std::cout << "Volume: " << sphere.volume << "cm" << std::endl;

    return 0;
}
