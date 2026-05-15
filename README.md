# watchdog-rs

`watchdog-rs` is a Linux-only process watchdog written in Rust.

It monitors a target process through procfs and reports runtime signals such as RSS memory usage, thread count, file descriptor count, and TCP socket table changes.

The project is intentionally small, but the goal is not to be a toy. It is a focused systems programming project that demonstrates Linux process introspection, defensive Rust error handling, and security-oriented monitoring logic.

## What it monitors

`watchdog-rs` currently observes:

- Resident memory usage from `/proc/<pid>/status`
- Thread count from `/proc/<pid>/status`
- File descriptor count from `/proc/<pid>/fd`
- TCP socket visibility from `/proc/<pid>/net/tcp`

## Why this exists

Many observability agents, container security tools, and EDR systems begin with a similar foundation: inspect runtime process behavior and flag suspicious changes.

This project explores that foundation directly using Linux procfs.

It is useful for:

- learning Linux process introspection
- demonstrating Rust systems programming
- building small process-level watchdogs
- experimenting with security-oriented runtime signals
- creating reproducible examples for memory growth, thread growth, and TCP activity

It is not intended to replace a production EDR, eBPF agent, or full observability platform.

## Platform support

`watchdog-rs` is Linux-only.

It depends on Linux procfs paths such as:

```text
/proc/<pid>/status
/proc/<pid>/fd
/proc/<pid>/net/tcp
```

It is not expected to work on macOS or Windows.

## Build

```bash
cargo build
```

For a release build:

```bash
cargo build --release
```

## Run

Basic usage:

```bash
cargo run -- --pid 1234 --interval 1 --max-rss-mb 500
```

Or, after building:

```bash
./target/debug/watchdog-rs --pid 1234 --interval 1 --max-rss-mb 500
```

Example with finite sampling:

```bash
cargo run -- --pid 1234 --interval 1 --max-rss-mb 500 --samples 5
```

The `--samples` option is useful for demos, tests, scripts, and CI because the process exits after a fixed number of samples.

## CLI options

```text
--pid <PID>
    Target process ID to monitor.

--interval <SECONDS>
    Sampling interval in seconds.
    Must be greater than zero.
    Default: 1

--samples <COUNT>
    Optional number of samples to collect before exiting.
    If omitted, watchdog-rs runs until interrupted.

--max-rss-mb <MB>
    Optional RSS memory threshold in megabytes.

--max-rss-growth-percent <PERCENT>
    Alert when RSS grows by this percentage relative to the baseline sample.
    Default: 25.0

--thread-jump <COUNT>
    Alert when thread count increases by more than this amount between samples.
    Default: 5
```

## Example output

```text
PID: 1234
RSS: 148 MB
Threads: 11
FDs: 42
TCP Connections: 3
Status: OK
```

Example alert output:

```text
PID: 1234
RSS: 184 MB
Threads: 12
FDs: 46
TCP Connections: 4
Status: ALERT
Alerts:
- RSS exceeds configured threshold: 184 MB > 100 MB
- RSS increased by 38.0 percent since baseline
- New TCP socket observed: 127.0.0.1:40336 -> 127.0.0.1:8080 state=01
```

## Alert logic

`watchdog-rs` currently alerts on:

- RSS exceeding a configured threshold
- RSS growth relative to the initial baseline sample
- sudden thread count jumps between adjacent samples
- newly observed TCP socket table entries

The design intentionally separates baseline comparison from previous-sample comparison:

```text
RSS growth:
    baseline sample -> current sample

Thread jumps:
    previous sample -> current sample

TCP socket changes:
    previous sample -> current sample
```

This makes the alert behavior easier to reason about and test.

## Running the included examples

The repository includes example target programs under `examples/`.

These are not part of the watchdog itself. They are controlled target processes used to exercise specific monitoring paths.

### RSS growth target

Run the memory growth target:

```bash
cargo run --example rss_growth_target
```

It prints its PID:

```text
PID: 12345
allocated and touched 10 MB
allocated and touched 20 MB
```

In another terminal, monitor that PID:

```bash
cargo run -- --pid 12345 --interval 1 --max-rss-mb 1000 --max-rss-growth-percent 10 --samples 6
```

Expected result:

```text
Status: ALERT
Alerts:
- RSS increased by ... percent since baseline
```

### Thread jump target

Run the thread jump target:

```bash
cargo run --example thread_jump_target
```

It prints its PID and waits before spawning worker threads.

In another terminal, monitor that PID:

```bash
cargo run -- --pid 12345 --interval 1 --max-rss-mb 1000 --thread-jump 5 --samples 35
```

Expected result:

```text
Status: ALERT
Alerts:
- Thread count jumped from 1 to 21
```

The exact numbers may vary depending on the runtime and system.

### TCP socket target

Start the TCP server:

```bash
cargo run --example tcp_server_target
```

In another terminal, start the TCP client:

```bash
cargo run --example tcp_client_target
```

The client prints its PID and waits before opening connections.

In a third terminal, monitor the client PID:

```bash
cargo run -- --pid 12345 --interval 1 --max-rss-mb 1000 --samples 30
```

Expected result:

```text
Status: ALERT
Alerts:
- New TCP socket observed: 127.0.0.1:40336 -> 127.0.0.1:8080 state=01
```

On localhost tests, you may see both sides of a loopback connection represented in the TCP socket table.

Example:

```text
127.0.0.1:40336 -> 127.0.0.1:8080 state=01
127.0.0.1:8080 -> 127.0.0.1:40336 state=01
```

This is expected for the current implementation.

## Development checks

Before committing changes:

```bash
cargo fmt
cargo check
cargo clippy
```

Run a quick functional check against the current shell:

```bash
cargo run -- --pid $$ --interval 1 --max-rss-mb 1000 --samples 2
```

Expected result:

```text
Status: OK
```

Force an RSS threshold alert:

```bash
cargo run -- --pid $$ --interval 1 --max-rss-mb 1 --samples 2
```

Expected result:

```text
Status: ALERT
Alerts:
- RSS exceeds configured threshold: ...
```

Test invalid PID handling:

```bash
cargo run -- --pid 999999999 --interval 1 --samples 1
```

Expected result:

```text
failed to read /proc/999999999/status
```

Test invalid interval handling:

```bash
cargo run -- --pid $$ --interval 0 --samples 1
```

Expected result:

```text
[ERROR]
The CLI should reject interval zero before monitoring starts.
```

## Design notes

The current implementation keeps the project intentionally simple:

```text
CLI args
    -> thresholds
    -> collect process snapshot
    -> evaluate alerts
    -> print result
```

The core model is a process snapshot:

```text
PID
RSS memory
thread count
file descriptor count
TCP socket entries
```

Alert evaluation is pure comparison logic over snapshots. This keeps the monitoring logic easier to test and reason about.

## Limitations

This is not a full EDR, production security agent, or complete observability system.

Important limitations:

- Linux only
- no eBPF
- no daemon mode
- no systemd service integration
- no historical database
- no JSON output yet
- no Prometheus exporter yet
- no process tree tracking
- no container namespace normalization
- no user, command line, or executable path enrichment yet
- no allowlist for expected remote addresses yet
- TCP socket data comes from `/proc/<pid>/net/tcp`, which reflects the process network namespace rather than a perfect per-process socket inventory in every case

The TCP logic should be interpreted as socket table observation, not full socket ownership attribution.

A more advanced implementation would correlate socket inodes from `/proc/<pid>/fd` with entries in `/proc/net/tcp` or use eBPF for stronger attribution.

## Future work

Planned improvements:

- TODO - will update later

## Project status

This is useful as a small monitoring tool and as a foundation for deeper process observability work.

