CXX = g++
CXXFLAGS = -std=c++17 -Wall -Wextra -Wpedantic -g

TARGET = number_guessing/game
SOURCES = number_guessing/number_guessing.cpp dynamic_value.cpp
HEADERS = check_dynamic_value.h dynamic_value.h

$(TARGET): $(SOURCES) $(HEADERS)
	$(CXX) $(CXXFLAGS) $(SOURCES) -o $(TARGET)

run: $(TARGET)
	./$(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: run clean