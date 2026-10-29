#include <iostream>

struct Human {
  int age;
  bool hungry;
};

int main() {
  Human human;

  human.age = 13;
  human.hungry = true;

  std::cout << human.age << std::endl;

  std::cout << "Hello World" << std::endl;

  return 0;
}
