//! Domain definitions shared by the CLI, telemetry validation and reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
    Samples,
}
impl MetricKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
            Self::Samples => "samples",
        }
    }
}
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub kind: MetricKind,
}
impl Metric {
    const fn counter(key: &'static str, label: &'static str, unit: &'static str) -> Self {
        Self {
            key,
            label,
            unit,
            kind: MetricKind::Counter,
        }
    }
    const fn gauge(key: &'static str, label: &'static str, unit: &'static str) -> Self {
        Self {
            key,
            label,
            unit,
            kind: MetricKind::Gauge,
        }
    }
    const fn samples(key: &'static str, label: &'static str, unit: &'static str) -> Self {
        Self {
            key,
            label,
            unit,
            kind: MetricKind::Samples,
        }
    }
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
    Profile {
        flag: "networking",
        title: "Networking",
        context: &["system", "voluntary_switches", "cpu"],
        topics: &[
            Topic {
                flag: "sockets",
                title: "Socket operations",
                guidance: concat!(
                    "Check failed connects, descriptor limits and nonblocking retry ",
                    "behavior.",
                ),
                metrics: &[
                    Metric::counter("networking.socket_calls", "Socket operations", "count"),
                    Metric::counter(
                        "networking.socket_errors",
                        "Socket syscall errors (including retries)",
                        "count",
                    ),
                    Metric::counter("networking.connect_calls", "Connect attempts", "count"),
                ],
            },
            Topic {
                flag: "bandwidth",
                title: "Network transfer",
                guidance: concat!(
                    "Socket bytes are application-visible transfers, not wire bytes; ",
                    "loopback and both endpoints can be counted.",
                ),
                metrics: &[
                    Metric::counter("networking.sent_bytes", "Bytes sent", "B"),
                    Metric::counter("networking.received_bytes", "Bytes received", "B"),
                    Metric::counter("networking.network_calls", "Network syscalls", "count"),
                ],
            },
            Topic {
                flag: "latency",
                title: "Network latency",
                guidance: concat!(
                    "Syscall elapsed time is not request latency. Use a fixed-rate ",
                    "load driver to investigate coordinated omission.",
                ),
                metrics: &[
                    Metric::samples("networking.syscall_ms", "Network syscall duration", "ms"),
                    Metric::samples("networking.request_ms", "Request duration", "ms"),
                    Metric::counter("networking.timeouts", "Request timeouts", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "frontend",
        title: "Frontend",
        context: &["wall", "rss", "cpu"],
        topics: &[
            Topic {
                flag: "rendering",
                title: "Page rendering",
                guidance: concat!(
                    "Collect browser measurements; executable duration is not LCP or ",
                    "layout stability.",
                ),
                metrics: &[
                    Metric::samples("frontend.lcp_ms", "Largest Contentful Paint", "ms"),
                    Metric::gauge("frontend.cls", "Cumulative Layout Shift", "score"),
                    Metric::samples(
                        "frontend.long_task_ms",
                        "Main-thread long task duration",
                        "ms",
                    ),
                ],
            },
            Topic {
                flag: "interactivity",
                title: "UI responsiveness",
                guidance: concat!(
                    "Measure interactions in the browser; distinguish input delay from",
                    " handler and presentation work.",
                ),
                metrics: &[
                    Metric::samples("frontend.inp_ms", "Interaction to Next Paint", "ms"),
                    Metric::samples("frontend.input_delay_ms", "Input delay", "ms"),
                    Metric::samples("frontend.handler_ms", "Event handler duration", "ms"),
                ],
            },
            Topic {
                flag: "accessibility",
                title: "Accessibility checks",
                guidance: concat!(
                    "Import findings from an accessibility test driver. Automated ",
                    "checks do not establish WCAG conformance.",
                ),
                metrics: &[
                    Metric::counter(
                        "frontend.a11y_violations",
                        "Automated accessibility violations",
                        "count",
                    ),
                    Metric::counter(
                        "frontend.a11y_checks",
                        "Accessibility checks executed",
                        "count",
                    ),
                    Metric::counter(
                        "frontend.keyboard_failures",
                        "Keyboard navigation test failures",
                        "count",
                    ),
                ],
            },
        ],
    },
    Profile {
        flag: "backend",
        title: "Backend",
        context: &["cpu", "rss", "voluntary_switches"],
        topics: &[
            Topic {
                flag: "api",
                title: "API requests",
                guidance: concat!(
                    "Keep successful and failed response timings separate; process ",
                    "invocations are not request counts.",
                ),
                metrics: &[
                    Metric::samples(
                        "backend.request_ms",
                        "Successful API request duration",
                        "ms",
                    ),
                    Metric::counter("backend.requests", "API requests completed", "count"),
                    Metric::counter("backend.errors", "Failed API requests", "count"),
                ],
            },
            Topic {
                flag: "database",
                title: "Database queries",
                guidance: concat!(
                    "Use query plans and buffer statistics to investigate slow ",
                    "queries; RSS is not a database cache hit rate.",
                ),
                metrics: &[
                    Metric::samples("backend.query_ms", "Query duration", "ms"),
                    Metric::counter("backend.queries", "Queries executed", "count"),
                    Metric::samples("backend.lock_wait_ms", "Database lock wait", "ms"),
                ],
            },
            Topic {
                flag: "distributed",
                title: "Distributed operations",
                guidance: concat!(
                    "Record queue delays and retries; local counters cannot establish ",
                    "remote service saturation.",
                ),
                metrics: &[
                    Metric::samples("backend.rpc_ms", "RPC duration", "ms"),
                    Metric::samples("backend.queue_ms", "Queue wait", "ms"),
                    Metric::counter("backend.retries", "RPC retries", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "mobile",
        title: "Mobile",
        context: &["rss", "cpu", "wall"],
        topics: &[
            Topic {
                flag: "startup",
                title: "Application startup",
                guidance: concat!(
                    "Emit device TTID/TTFD via a host test driver; Linux process ",
                    "lifetime is not Android or iOS startup.",
                ),
                metrics: &[
                    Metric::samples("mobile.ttid_ms", "Time to initial display", "ms"),
                    Metric::samples("mobile.ttfd_ms", "Time to full display", "ms"),
                    Metric::counter("mobile.startup_failures", "Startup failures", "count"),
                ],
            },
            Topic {
                flag: "responsiveness",
                title: "Mobile responsiveness",
                guidance: concat!(
                    "Measure actual device frame times and ANRs; emulator host ",
                    "measurements have a different scope.",
                ),
                metrics: &[
                    Metric::samples("mobile.frame_ms", "Presented frame interval", "ms"),
                    Metric::counter("mobile.anrs", "Application-not-responding events", "count"),
                    Metric::counter("mobile.dropped_frames", "Dropped frames", "count"),
                ],
            },
            Topic {
                flag: "battery",
                title: "Device energy",
                guidance: concat!(
                    "Device energy and wake locks require device instrumentation; host",
                    " CPU time is not battery drain.",
                ),
                metrics: &[
                    Metric::gauge("mobile.energy_j", "Device energy consumed", "J"),
                    Metric::samples("mobile.wake_lock_ms", "Wake lock hold duration", "ms"),
                    Metric::counter("mobile.wakeups", "Device wakeups", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "gamedev",
        title: "Game development",
        context: &["cpu", "rss", "involuntary_switches"],
        topics: &[
            Topic {
                flag: "gameplay",
                title: "Simulation and gameplay",
                guidance: concat!(
                    "Record simulation work separately from presented frames to ",
                    "distinguish CPU logic and rendering costs.",
                ),
                metrics: &[
                    Metric::samples("gamedev.tick_ms", "Simulation tick duration", "ms"),
                    Metric::samples("gamedev.physics_ms", "Physics step duration", "ms"),
                    Metric::gauge("gamedev.entities", "Active entity count", "count"),
                ],
            },
            Topic {
                flag: "graphics",
                title: "Graphics and frame pacing",
                guidance: concat!(
                    "Frame interval samples support FPS and budget analysis. GPU ",
                    "timings require GPU timestamps or synchronization.",
                ),
                metrics: &[
                    Metric::samples("gamedev.frame_ms", "Presented frame interval", "ms"),
                    Metric::samples("gamedev.gpu_frame_ms", "GPU frame duration", "ms"),
                    Metric::samples("gamedev.draw_calls", "Draw calls per frame", "count"),
                ],
            },
            Topic {
                flag: "game-ai",
                title: "Game AI",
                guidance: concat!(
                    "Benchmark representative path searches and decision updates, ",
                    "including worst-case scenes.",
                ),
                metrics: &[
                    Metric::samples("gamedev.pathfinding_ms", "Pathfinding duration", "ms"),
                    Metric::samples("gamedev.decision_ms", "AI decision duration", "ms"),
                    Metric::counter("gamedev.nodes_expanded", "Search nodes expanded", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "data",
        title: "Data and machine learning",
        context: &["cpu", "rss", "major_faults"],
        topics: &[
            Topic {
                flag: "pipelines",
                title: "Data pipelines",
                guidance: concat!(
                    "Record rows and bytes processed together with stage durations; ",
                    "distinguish I/O waits from compute.",
                ),
                metrics: &[
                    Metric::counter("data.rows", "Rows processed", "count"),
                    Metric::counter("data.input_bytes", "Input bytes processed", "B"),
                    Metric::samples("data.stage_ms", "Pipeline stage duration", "ms"),
                ],
            },
            Topic {
                flag: "analytics",
                title: "Analytics and quantitative workloads",
                guidance: concat!(
                    "Use a fixed dataset and verify outputs; query latency and spill ",
                    "volume reveal different bottlenecks.",
                ),
                metrics: &[
                    Metric::samples("data.query_ms", "Analytical query duration", "ms"),
                    Metric::counter("data.spill_bytes", "Bytes spilled to disk", "B"),
                    Metric::counter("data.result_rows", "Result rows produced", "count"),
                ],
            },
            Topic {
                flag: "ml",
                title: "Machine learning",
                guidance: concat!(
                    "Measure accelerator work with correct synchronization; warm up ",
                    "models and keep batch size fixed.",
                ),
                metrics: &[
                    Metric::samples("data.inference_ms", "Inference duration", "ms"),
                    Metric::samples("data.training_step_ms", "Training step duration", "ms"),
                    Metric::counter("data.samples", "Samples processed", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "devops",
        title: "DevOps",
        context: &["wall", "rss", "cpu_percent"],
        topics: &[
            Topic {
                flag: "ci",
                title: "CI and build pipelines",
                guidance: concat!(
                    "Use equivalent cache state and inputs when comparing build stages",
                    " and cache behavior.",
                ),
                metrics: &[
                    Metric::samples("devops.stage_ms", "CI stage duration", "ms"),
                    Metric::counter("devops.cache_hits", "Build cache hits", "count"),
                    Metric::counter("devops.cache_misses", "Build cache misses", "count"),
                ],
            },
            Topic {
                flag: "sre",
                title: "Service reliability",
                guidance: concat!(
                    "Track latency, traffic, errors and saturation using workload ",
                    "instrumentation; host PSI is shared context.",
                ),
                metrics: &[
                    Metric::samples("devops.request_ms", "Service request duration", "ms"),
                    Metric::counter("devops.requests", "Service requests", "count"),
                    Metric::counter("devops.errors", "Service errors", "count"),
                ],
            },
            Topic {
                flag: "containers",
                title: "Container workloads",
                guidance: concat!(
                    "Import counters from the target cgroup; host pressure is not ",
                    "target-container attribution.",
                ),
                metrics: &[
                    Metric::gauge("devops.throttled_ms", "Cgroup CPU throttled time", "ms"),
                    Metric::counter("devops.oom_events", "Cgroup OOM events", "count"),
                    Metric::counter("devops.restarts", "Container restarts", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "qa",
        title: "Quality assurance",
        context: &["wall", "rss", "stderr_bytes"],
        topics: &[
            Topic {
                flag: "load-testing",
                title: "Load tests",
                guidance: concat!(
                    "Use an external load driver with controlled arrivals. cppbench ",
                    "does not generate HTTP load or correct coordinated omission.",
                ),
                metrics: &[
                    Metric::samples("qa.response_ms", "Load-test response duration", "ms"),
                    Metric::counter("qa.requests", "Requests attempted", "count"),
                    Metric::counter("qa.failures", "Failed requests", "count"),
                ],
            },
            Topic {
                flag: "soak",
                title: "Soak tests",
                guidance: concat!(
                    "Collect samples within one long invocation; fresh-process RSS ",
                    "across runs cannot prove an in-process leak.",
                ),
                metrics: &[
                    Metric::samples("qa.heap_bytes", "In-process heap usage samples", "B"),
                    Metric::samples("qa.open_fds", "In-process open descriptor samples", "count"),
                    Metric::counter("qa.errors", "Soak-test errors", "count"),
                ],
            },
            Topic {
                flag: "correctness",
                title: "Correctness tests",
                guidance: concat!(
                    "Assertions are reported by the test driver. Exit status and ",
                    "expected-output checks remain separate.",
                ),
                metrics: &[
                    Metric::counter("qa.assertions", "Assertions executed", "count"),
                    Metric::counter("qa.assertion_failures", "Failed assertions", "count"),
                    Metric::counter("qa.cases", "Test cases executed", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "embedded",
        title: "Embedded and robotics",
        context: &["major_faults", "voluntary_switches", "involuntary_switches"],
        topics: &[
            Topic {
                flag: "realtime",
                title: "Real-time tasks",
                guidance: concat!(
                    "Observed tails and deadline misses do not prove a worst-case ",
                    "bound or hard real-time certification.",
                ),
                metrics: &[
                    Metric::samples("embedded.cycle_ms", "Control-cycle duration", "ms"),
                    Metric::samples("embedded.jitter_ms", "Absolute scheduling jitter", "ms"),
                    Metric::counter(
                        "embedded.deadline_misses",
                        "Reported deadline misses",
                        "count",
                    ),
                ],
            },
            Topic {
                flag: "iot",
                title: "IoT firmware",
                guidance: concat!(
                    "Use a device driver to emit target timings and energy; ",
                    "host-process counters describe only the driver.",
                ),
                metrics: &[
                    Metric::samples("embedded.message_ms", "Message delivery duration", "ms"),
                    Metric::counter("embedded.messages", "Messages delivered", "count"),
                    Metric::gauge("embedded.energy_j", "Device energy consumed", "J"),
                ],
            },
            Topic {
                flag: "robotics",
                title: "Robotics control",
                guidance: concat!(
                    "Measure sensor-to-actuator delay and control-loop timing in ",
                    "representative scenes.",
                ),
                metrics: &[
                    Metric::samples("embedded.control_ms", "Control-loop duration", "ms"),
                    Metric::samples("embedded.sensor_ms", "Sensor-to-actuator delay", "ms"),
                    Metric::counter("embedded.missed_updates", "Missed control updates", "count"),
                ],
            },
        ],
    },
    Profile {
        flag: "systems",
        title: "Systems programming",
        context: &["system", "major_faults", "involuntary_switches"],
        topics: &[
            Topic {
                flag: "compilation",
                title: "Compiler workloads",
                guidance: concat!(
                    "Benchmark the compiler executable or emit compiler phase timings;",
                    " use time traces for attribution.",
                ),
                metrics: &[
                    Metric::samples("systems.parse_ms", "Compiler parse phase", "ms"),
                    Metric::samples("systems.optimize_ms", "Compiler optimization phase", "ms"),
                    Metric::samples("systems.codegen_ms", "Compiler code generation phase", "ms"),
                ],
            },
            Topic {
                flag: "storage",
                title: "Storage I/O",
                guidance: concat!(
                    "Record completed operations and bytes; wait4 block counters are ",
                    "not device IOPS or byte counts.",
                ),
                metrics: &[
                    Metric::samples("systems.io_ms", "Completed I/O latency", "ms"),
                    Metric::counter("systems.io_operations", "Completed I/O operations", "count"),
                    Metric::counter("systems.io_bytes", "Completed I/O bytes", "B"),
                ],
            },
            Topic {
                flag: "concurrency",
                title: "Concurrent execution",
                guidance: concat!(
                    "Context switches alone do not establish lock contention; record ",
                    "waits and completed tasks.",
                ),
                metrics: &[
                    Metric::samples("systems.lock_wait_ms", "Lock wait duration", "ms"),
                    Metric::counter("systems.tasks", "Tasks completed", "count"),
                    Metric::samples("systems.queue_ms", "Task queue wait", "ms"),
                ],
            },
        ],
    },
];
