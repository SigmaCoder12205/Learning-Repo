// A local socket-pair benchmark: no external service or open listening port.
// A "request" here is a fixed-size round trip between the two endpoints.
#include "../include/bench_telemetry.h"
#include <array>
#include <chrono>
#include <cstdio>
#include <sys/socket.h>
#include <unistd.h>
#include <vector>

int main() {
    int sockets[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets)) return 1;
    const bool collect = bench_telemetry_enabled();
    constexpr int iterations = 1000;
    std::array<char,256> buffer{};
    std::vector<double> requests, syscalls;
    if (collect) {
        requests.reserve(iterations);
        syscalls.reserve(iterations*4);
    }
    using Clock = std::chrono::steady_clock;
    auto call = [&](int fd, bool receive) {
        const auto start = collect ? Clock::now() : Clock::time_point{};
        const auto count = receive ? recv(fd,buffer.data(),buffer.size(),MSG_WAITALL) : send(fd,buffer.data(),buffer.size(),MSG_NOSIGNAL);
        if (collect) syscalls.push_back(std::chrono::duration<double,std::milli>(Clock::now()-start).count());
        return count == static_cast<ssize_t>(buffer.size());
    };
    for (int i=0;i<iterations;++i) {
        const auto start = collect ? Clock::now() : Clock::time_point{};
        if (!call(sockets[0],false) || !call(sockets[1],true) || !call(sockets[1],false) || !call(sockets[0],true)) return 2;
        if (collect) requests.push_back(std::chrono::duration<double,std::milli>(Clock::now()-start).count());
    }
    close(sockets[0]); close(sockets[1]);
    auto telemetry = bench_telemetry_open();
    bench_telemetry_value(&telemetry,"networking.socket_calls","count",iterations*4+1);
    bench_telemetry_value(&telemetry,"networking.socket_errors","count",0);
    bench_telemetry_value(&telemetry,"networking.connect_calls","count",0);
    bench_telemetry_value(&telemetry,"networking.network_calls","count",iterations*4+1);
    bench_telemetry_value(&telemetry,"networking.sent_bytes","B",iterations*2*buffer.size());
    bench_telemetry_value(&telemetry,"networking.received_bytes","B",iterations*2*buffer.size());
    bench_telemetry_value(&telemetry,"networking.timeouts","count",0);
    if (collect) {
        bench_telemetry_samples(&telemetry,"networking.request_ms","ms",requests.data(),requests.size());
        bench_telemetry_samples(&telemetry,"networking.syscall_ms","ms",syscalls.data(),syscalls.size());
    }
    if (bench_telemetry_close(&telemetry)) return 3;
    std::puts("1000 local round trips completed");
}
