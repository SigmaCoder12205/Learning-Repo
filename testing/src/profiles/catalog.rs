// Shared catalog for CLI flags, validation, reporting and documentation.
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub kind: &'static str,
}
pub struct Topic {
    pub flag: &'static str,
    pub title: &'static str,
    pub guidance: &'static str,
    pub metrics: &'static [Metric],
}
pub struct Profile {
    pub flag: &'static str,
    pub title: &'static str,
    pub context: &'static [&'static str],
    pub topics: &'static [Topic],
}
pub const PROFILES: &[Profile] = &[
Profile { flag:"networking", title:"Networking", context:&["system","voluntary_switches","cpu"], topics:&[
Topic { flag:"sockets", title:"Socket operations", guidance:"Check failed connects, descriptor limits and nonblocking retry behavior.", metrics:&[
Metric { key:"networking.socket_calls", label:"Socket operations", unit:"count", kind:"counter" },
Metric { key:"networking.socket_errors", label:"Socket syscall errors (including retries)", unit:"count", kind:"counter" },
Metric { key:"networking.connect_calls", label:"Connect attempts", unit:"count", kind:"counter" },
]},
Topic { flag:"bandwidth", title:"Network transfer", guidance:"Socket bytes are application-visible transfers, not wire bytes; loopback and both endpoints can be counted.", metrics:&[
Metric { key:"networking.sent_bytes", label:"Bytes sent", unit:"B", kind:"counter" },
Metric { key:"networking.received_bytes", label:"Bytes received", unit:"B", kind:"counter" },
Metric { key:"networking.network_calls", label:"Network syscalls", unit:"count", kind:"counter" },
]},
Topic { flag:"latency", title:"Network latency", guidance:"Syscall elapsed time is not request latency. Use a fixed-rate load driver to investigate coordinated omission.", metrics:&[
Metric { key:"networking.syscall_ms", label:"Network syscall duration", unit:"ms", kind:"samples" },
Metric { key:"networking.request_ms", label:"Request duration", unit:"ms", kind:"samples" },
Metric { key:"networking.timeouts", label:"Request timeouts", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"frontend", title:"Frontend", context:&["wall","rss","cpu"], topics:&[
Topic { flag:"rendering", title:"Page rendering", guidance:"Collect browser measurements; executable duration is not LCP or layout stability.", metrics:&[
Metric { key:"frontend.lcp_ms", label:"Largest Contentful Paint", unit:"ms", kind:"samples" },
Metric { key:"frontend.cls", label:"Cumulative Layout Shift", unit:"score", kind:"gauge" },
Metric { key:"frontend.long_task_ms", label:"Main-thread long task duration", unit:"ms", kind:"samples" },
]},
Topic { flag:"interactivity", title:"UI responsiveness", guidance:"Measure interactions in the browser; distinguish input delay from handler and presentation work.", metrics:&[
Metric { key:"frontend.inp_ms", label:"Interaction to Next Paint", unit:"ms", kind:"samples" },
Metric { key:"frontend.input_delay_ms", label:"Input delay", unit:"ms", kind:"samples" },
Metric { key:"frontend.handler_ms", label:"Event handler duration", unit:"ms", kind:"samples" },
]},
Topic { flag:"accessibility", title:"Accessibility checks", guidance:"Import findings from an accessibility test driver. Automated checks do not establish WCAG conformance.", metrics:&[
Metric { key:"frontend.a11y_violations", label:"Automated accessibility violations", unit:"count", kind:"counter" },
Metric { key:"frontend.a11y_checks", label:"Accessibility checks executed", unit:"count", kind:"counter" },
Metric { key:"frontend.keyboard_failures", label:"Keyboard navigation test failures", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"backend", title:"Backend", context:&["cpu","rss","voluntary_switches"], topics:&[
Topic { flag:"api", title:"API requests", guidance:"Keep successful and failed response timings separate; process invocations are not request counts.", metrics:&[
Metric { key:"backend.request_ms", label:"Successful API request duration", unit:"ms", kind:"samples" },
Metric { key:"backend.requests", label:"API requests completed", unit:"count", kind:"counter" },
Metric { key:"backend.errors", label:"Failed API requests", unit:"count", kind:"counter" },
]},
Topic { flag:"database", title:"Database queries", guidance:"Use query plans and buffer statistics to investigate slow queries; RSS is not a database cache hit rate.", metrics:&[
Metric { key:"backend.query_ms", label:"Query duration", unit:"ms", kind:"samples" },
Metric { key:"backend.queries", label:"Queries executed", unit:"count", kind:"counter" },
Metric { key:"backend.lock_wait_ms", label:"Database lock wait", unit:"ms", kind:"samples" },
]},
Topic { flag:"distributed", title:"Distributed operations", guidance:"Record queue delays and retries; local counters cannot establish remote service saturation.", metrics:&[
Metric { key:"backend.rpc_ms", label:"RPC duration", unit:"ms", kind:"samples" },
Metric { key:"backend.queue_ms", label:"Queue wait", unit:"ms", kind:"samples" },
Metric { key:"backend.retries", label:"RPC retries", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"mobile", title:"Mobile", context:&["rss","cpu","wall"], topics:&[
Topic { flag:"startup", title:"Application startup", guidance:"Emit device TTID/TTFD via a host test driver; Linux process lifetime is not Android or iOS startup.", metrics:&[
Metric { key:"mobile.ttid_ms", label:"Time to initial display", unit:"ms", kind:"samples" },
Metric { key:"mobile.ttfd_ms", label:"Time to full display", unit:"ms", kind:"samples" },
Metric { key:"mobile.startup_failures", label:"Startup failures", unit:"count", kind:"counter" },
]},
Topic { flag:"responsiveness", title:"Mobile responsiveness", guidance:"Measure actual device frame times and ANRs; emulator host measurements have a different scope.", metrics:&[
Metric { key:"mobile.frame_ms", label:"Presented frame interval", unit:"ms", kind:"samples" },
Metric { key:"mobile.anrs", label:"Application-not-responding events", unit:"count", kind:"counter" },
Metric { key:"mobile.dropped_frames", label:"Dropped frames", unit:"count", kind:"counter" },
]},
Topic { flag:"battery", title:"Device energy", guidance:"Device energy and wake locks require device instrumentation; host CPU time is not battery drain.", metrics:&[
Metric { key:"mobile.energy_j", label:"Device energy consumed", unit:"J", kind:"gauge" },
Metric { key:"mobile.wake_lock_ms", label:"Wake lock hold duration", unit:"ms", kind:"samples" },
Metric { key:"mobile.wakeups", label:"Device wakeups", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"gamedev", title:"Game development", context:&["cpu","rss","involuntary_switches"], topics:&[
Topic { flag:"gameplay", title:"Simulation and gameplay", guidance:"Record simulation work separately from presented frames to distinguish CPU logic and rendering costs.", metrics:&[
Metric { key:"gamedev.tick_ms", label:"Simulation tick duration", unit:"ms", kind:"samples" },
Metric { key:"gamedev.physics_ms", label:"Physics step duration", unit:"ms", kind:"samples" },
Metric { key:"gamedev.entities", label:"Active entity count", unit:"count", kind:"gauge" },
]},
Topic { flag:"graphics", title:"Graphics and frame pacing", guidance:"Frame interval samples support FPS and budget analysis. GPU timings require GPU timestamps or synchronization.", metrics:&[
Metric { key:"gamedev.frame_ms", label:"Presented frame interval", unit:"ms", kind:"samples" },
Metric { key:"gamedev.gpu_frame_ms", label:"GPU frame duration", unit:"ms", kind:"samples" },
Metric { key:"gamedev.draw_calls", label:"Draw calls per frame", unit:"count", kind:"samples" },
]},
Topic { flag:"game-ai", title:"Game AI", guidance:"Benchmark representative path searches and decision updates, including worst-case scenes.", metrics:&[
Metric { key:"gamedev.pathfinding_ms", label:"Pathfinding duration", unit:"ms", kind:"samples" },
Metric { key:"gamedev.decision_ms", label:"AI decision duration", unit:"ms", kind:"samples" },
Metric { key:"gamedev.nodes_expanded", label:"Search nodes expanded", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"data", title:"Data and machine learning", context:&["cpu","rss","major_faults"], topics:&[
Topic { flag:"pipelines", title:"Data pipelines", guidance:"Record rows and bytes processed together with stage durations; distinguish I/O waits from compute.", metrics:&[
Metric { key:"data.rows", label:"Rows processed", unit:"count", kind:"counter" },
Metric { key:"data.input_bytes", label:"Input bytes processed", unit:"B", kind:"counter" },
Metric { key:"data.stage_ms", label:"Pipeline stage duration", unit:"ms", kind:"samples" },
]},
Topic { flag:"analytics", title:"Analytics and quantitative workloads", guidance:"Use a fixed dataset and verify outputs; query latency and spill volume reveal different bottlenecks.", metrics:&[
Metric { key:"data.query_ms", label:"Analytical query duration", unit:"ms", kind:"samples" },
Metric { key:"data.spill_bytes", label:"Bytes spilled to disk", unit:"B", kind:"counter" },
Metric { key:"data.result_rows", label:"Result rows produced", unit:"count", kind:"counter" },
]},
Topic { flag:"ml", title:"Machine learning", guidance:"Measure accelerator work with correct synchronization; warm up models and keep batch size fixed.", metrics:&[
Metric { key:"data.inference_ms", label:"Inference duration", unit:"ms", kind:"samples" },
Metric { key:"data.training_step_ms", label:"Training step duration", unit:"ms", kind:"samples" },
Metric { key:"data.samples", label:"Samples processed", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"devops", title:"DevOps", context:&["wall","rss","cpu_percent"], topics:&[
Topic { flag:"ci", title:"CI and build pipelines", guidance:"Use equivalent cache state and inputs when comparing build stages and cache behavior.", metrics:&[
Metric { key:"devops.stage_ms", label:"CI stage duration", unit:"ms", kind:"samples" },
Metric { key:"devops.cache_hits", label:"Build cache hits", unit:"count", kind:"counter" },
Metric { key:"devops.cache_misses", label:"Build cache misses", unit:"count", kind:"counter" },
]},
Topic { flag:"sre", title:"Service reliability", guidance:"Track latency, traffic, errors and saturation using workload instrumentation; host PSI is shared context.", metrics:&[
Metric { key:"devops.request_ms", label:"Service request duration", unit:"ms", kind:"samples" },
Metric { key:"devops.requests", label:"Service requests", unit:"count", kind:"counter" },
Metric { key:"devops.errors", label:"Service errors", unit:"count", kind:"counter" },
]},
Topic { flag:"containers", title:"Container workloads", guidance:"Import counters from the target cgroup; host pressure is not target-container attribution.", metrics:&[
Metric { key:"devops.throttled_ms", label:"Cgroup CPU throttled time", unit:"ms", kind:"gauge" },
Metric { key:"devops.oom_events", label:"Cgroup OOM events", unit:"count", kind:"counter" },
Metric { key:"devops.restarts", label:"Container restarts", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"qa", title:"Quality assurance", context:&["wall","rss","stderr_bytes"], topics:&[
Topic { flag:"load-testing", title:"Load tests", guidance:"Use an external load driver with controlled arrivals. cppbench does not generate HTTP load or correct coordinated omission.", metrics:&[
Metric { key:"qa.response_ms", label:"Load-test response duration", unit:"ms", kind:"samples" },
Metric { key:"qa.requests", label:"Requests attempted", unit:"count", kind:"counter" },
Metric { key:"qa.failures", label:"Failed requests", unit:"count", kind:"counter" },
]},
Topic { flag:"soak", title:"Soak tests", guidance:"Collect samples within one long invocation; fresh-process RSS across runs cannot prove an in-process leak.", metrics:&[
Metric { key:"qa.heap_bytes", label:"In-process heap usage samples", unit:"B", kind:"samples" },
Metric { key:"qa.open_fds", label:"In-process open descriptor samples", unit:"count", kind:"samples" },
Metric { key:"qa.errors", label:"Soak-test errors", unit:"count", kind:"counter" },
]},
Topic { flag:"correctness", title:"Correctness tests", guidance:"Assertions are reported by the test driver. Exit status and expected-output checks remain separate.", metrics:&[
Metric { key:"qa.assertions", label:"Assertions executed", unit:"count", kind:"counter" },
Metric { key:"qa.assertion_failures", label:"Failed assertions", unit:"count", kind:"counter" },
Metric { key:"qa.cases", label:"Test cases executed", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"embedded", title:"Embedded and robotics", context:&["major_faults","voluntary_switches","involuntary_switches"], topics:&[
Topic { flag:"realtime", title:"Real-time tasks", guidance:"Observed tails and deadline misses do not prove a worst-case bound or hard real-time certification.", metrics:&[
Metric { key:"embedded.cycle_ms", label:"Control-cycle duration", unit:"ms", kind:"samples" },
Metric { key:"embedded.jitter_ms", label:"Absolute scheduling jitter", unit:"ms", kind:"samples" },
Metric { key:"embedded.deadline_misses", label:"Reported deadline misses", unit:"count", kind:"counter" },
]},
Topic { flag:"iot", title:"IoT firmware", guidance:"Use a device driver to emit target timings and energy; host-process counters describe only the driver.", metrics:&[
Metric { key:"embedded.message_ms", label:"Message delivery duration", unit:"ms", kind:"samples" },
Metric { key:"embedded.messages", label:"Messages delivered", unit:"count", kind:"counter" },
Metric { key:"embedded.energy_j", label:"Device energy consumed", unit:"J", kind:"gauge" },
]},
Topic { flag:"robotics", title:"Robotics control", guidance:"Measure sensor-to-actuator delay and control-loop timing in representative scenes.", metrics:&[
Metric { key:"embedded.control_ms", label:"Control-loop duration", unit:"ms", kind:"samples" },
Metric { key:"embedded.sensor_ms", label:"Sensor-to-actuator delay", unit:"ms", kind:"samples" },
Metric { key:"embedded.missed_updates", label:"Missed control updates", unit:"count", kind:"counter" },
]},
]},
Profile { flag:"systems", title:"Systems programming", context:&["system","major_faults","involuntary_switches"], topics:&[
Topic { flag:"compilation", title:"Compiler workloads", guidance:"Benchmark the compiler executable or emit compiler phase timings; use time traces for attribution.", metrics:&[
Metric { key:"systems.parse_ms", label:"Compiler parse phase", unit:"ms", kind:"samples" },
Metric { key:"systems.optimize_ms", label:"Compiler optimization phase", unit:"ms", kind:"samples" },
Metric { key:"systems.codegen_ms", label:"Compiler code generation phase", unit:"ms", kind:"samples" },
]},
Topic { flag:"storage", title:"Storage I/O", guidance:"Record completed operations and bytes; wait4 block counters are not device IOPS or byte counts.", metrics:&[
Metric { key:"systems.io_ms", label:"Completed I/O latency", unit:"ms", kind:"samples" },
Metric { key:"systems.io_operations", label:"Completed I/O operations", unit:"count", kind:"counter" },
Metric { key:"systems.io_bytes", label:"Completed I/O bytes", unit:"B", kind:"counter" },
]},
Topic { flag:"concurrency", title:"Concurrent execution", guidance:"Context switches alone do not establish lock contention; record waits and completed tasks.", metrics:&[
Metric { key:"systems.lock_wait_ms", label:"Lock wait duration", unit:"ms", kind:"samples" },
Metric { key:"systems.tasks", label:"Tasks completed", unit:"count", kind:"counter" },
Metric { key:"systems.queue_ms", label:"Task queue wait", unit:"ms", kind:"samples" },
]},
]},
];
