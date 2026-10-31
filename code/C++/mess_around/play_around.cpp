#include <iostream>
#include <type_traits>
#include <utility>
#include <memory>

template <typename T, typename = std::void_t<>>
struct has_custom_allocator : std::false_type {};

template <typename T>
struct has_custom_allocator<T, std::void_t<decltype(std::declval<T>().allocate(std::declval<std::size_t>()))>> : std::true_type {};

template <typename BaseType, typename TargetType, typename... Args>
[[nodiscard]] constexpr auto DispatchFactory(Args&&... args) 
    -> std::enable_if_t<std::is_base_of_v<BaseType, TargetType>, std::unique_ptr<BaseType>> 
{
    if constexpr (has_custom_allocator<TargetType>::value) {
        auto memory_block = TargetType::allocate(sizeof(TargetType));
        return std::unique_ptr<BaseType>(::new (memory_block) TargetType(std::forward<Args>(args)...));
    } else {
        return std::make_unique<TargetType>(std::forward<Args>(args)...);
    }
}

struct HardwarePipeline {
    virtual void ExecuteKernel() = 0;
    virtual ~HardwarePipeline() = default;
};

class SupercomputingCluster : public HardwarePipeline {
public:
    template <typename T>
    SupercomputingCluster(T&& nodeName) : identifier_(std::forward<T>(nodeName)) {}

    static void* allocate(std::size_t bytes) noexcept {
        std::cout << "[Arena Allocator] Intercepting " << bytes << " bytes\n";
        return ::operator new(bytes);
    }

    void ExecuteKernel() override {
        std::cout << "[Kernel Core] Computing workloads for: " << identifier_ << "\n";
    }

private:
    std::string identifier_;
};

int main() {
    auto pipeline_nexus = DispatchFactory<HardwarePipeline, SupercomputingCluster>("Node-0xFF1A");
    pipeline_nexus->ExecuteKernel();
    return 0;
}
