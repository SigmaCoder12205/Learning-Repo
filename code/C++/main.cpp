#include <iostream>
#include <unistd.h>  // Required for fork() and getpid()
#include <sys/types.h>

int main() {
    std::cout << "Starting program. Current PID: " << getpid() << std::endl;
    std::cout << "---------------------------------------" << std::endl;

    // The process splits into two right here
    pid_t pid = fork();

    if (pid < 0) {
        // fork failed
        std::cerr << "Fork failed!" << std::endl;
        return 1;
    } 
    else if (pid == 0) {
        // This code ONLY runs inside the Child Process
        std::cout << "[CHILD] Hello from the child process!" << std::endl;
        std::cout << "[CHILD] My PID is: " << getpid() << std::endl;
        std::cout << "[CHILD] My Parent's PID (PPID) is: " << getppid() << std::endl;
    } 
    else {
        // This code ONLY runs inside the Parent Process
        std::cout << "[PARENT] Hello from the parent process!" << std::endl;
        std::cout << "[PARENT] My PID is: " << getpid() << std::endl;
        std::cout << "[PARENT] I spawned a child with PID: " << pid << std::endl;
    }

    // Both processes will execute this line independently
    std::cout << "[PID " << getpid() << "] Program finished." << std::endl;

    return 0;
}
