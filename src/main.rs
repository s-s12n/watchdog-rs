use anyhow::{Context, Result, bail};
use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::net::Ipv4Addr;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "watchdog-rs")]
#[command(about = "Linux process monitor and security watchdog")]
struct Args {
    #[arg(long)]
    pid: i32,

    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    interval: u64,

    #[arg(long)]
    samples: Option<u64>,

    #[arg(long)]
    max_rss_mb: Option<u64>,

    #[arg(long, default_value_t = 25.0)]
    max_rss_growth_percent: f64,

    #[arg(long, default_value_t = 5)]
    thread_jump: u64,
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    pid: i32,
    rss_mb: u64,
    threads: u64,
    fd_count: u64,
    tcp_connections: Vec<String>,
}

#[derive(Debug, Clone)]
struct Thresholds {
    max_rss_mb: Option<u64>,
    max_rss_growth_percent: f64,
    thread_jump: u64,
}

#[derive(Debug, Clone)]
struct Alert {
    message: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let thresholds = Thresholds {
        max_rss_mb: args.max_rss_mb,
        max_rss_growth_percent: args.max_rss_growth_percent,
        thread_jump: args.thread_jump,
    };

    let baseline = collect_snapshot(args.pid)?;
    let mut previous = baseline.clone();

    let mut sample_count = 0_u64;

    loop {
        let current = collect_snapshot(args.pid)?;

        let alerts = evaluate_alerts(&baseline, &previous, &current, &thresholds);

        print_snapshot(&current, &alerts);

        previous = current;
        sample_count += 1;

        if let Some(max_samples) = args.samples {
            if sample_count >= max_samples {
                break;
            }
        }

        std::thread::sleep(Duration::from_secs(args.interval));
    }

    Ok(())
}

fn collect_snapshot(pid: i32) -> Result<ProcessSnapshot> {
    let status_path = format!("/proc/{pid}/status");
    let fd_path = format!("/proc/{pid}/fd");
    let tcp_path = format!("/proc/{pid}/net/tcp");

    let status_contents = fs::read_to_string(&status_path)
        .with_context(|| format!("failed to read {status_path}"))?;

    let rss_mb = parse_rss_mb(&status_contents)?;
    let threads = parse_threads(&status_contents)?;

    let fd_count = count_fds(&fd_path)?;
    let tcp_connections = read_tcp_connections(&tcp_path)?;

    Ok(ProcessSnapshot {
        pid,
        rss_mb,
        threads,
        fd_count,
        tcp_connections,
    })
}

fn parse_rss_mb(status_contents: &str) -> Result<u64> {
    for line in status_contents.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();

            let rss_kb: u64 = parts
                .get(1)
                .context("VmRSS line exists but has no numeric value")?
                .parse()
                .context("failed to parse VmRSS value as number")?;

            return Ok(rss_kb.div_ceil(1024));
        }
    }

    bail!("VmRSS not found in /proc status")
}

fn parse_threads(status_contents: &str) -> Result<u64> {
    for line in status_contents.lines() {
        if line.starts_with("Threads:") {
            let parts: Vec<&str> = line.split_whitespace().collect();

            let threads: u64 = parts
                .get(1)
                .context("Threads line exists but has no numeric value")?
                .parse()
                .context("failed to parse Threads value as number")?;

            return Ok(threads);
        }
    }

    bail!("Threads not found in /proc status")
}

fn count_fds(fd_path: &str) -> Result<u64> {
    let count = fs::read_dir(fd_path)
        .with_context(|| format!("failed to read {fd_path}"))?
        .count();

    Ok(count as u64)
}

fn read_tcp_connections(tcp_path: &str) -> Result<Vec<String>> {
    let contents =
        fs::read_to_string(tcp_path).with_context(|| format!("failed to read {tcp_path}"))?;

    let connections = contents
        .lines()
        .skip(1)
        .filter_map(parse_tcp_connection_line)
        .collect();

    Ok(connections)
}

fn parse_tcp_connection_line(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    let local_address = parts.get(1)?;
    let remote_address = parts.get(2)?;
    let state = parts.get(3)?;

    let local = decode_ipv4_socket_address(local_address)?;
    let remote = decode_ipv4_socket_address(remote_address)?;

    Some(format!("{local} -> {remote} state={state}"))
}

fn decode_ipv4_socket_address(value: &str) -> Option<String> {
    let (ip_hex, port_hex) = value.split_once(':')?;

    let raw_ip = u32::from_str_radix(ip_hex, 16).ok()?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;

    let bytes = raw_ip.to_le_bytes();
    let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);

    Some(format!("{ip}:{port}"))
}

fn print_snapshot(snapshot: &ProcessSnapshot, alerts: &[Alert]) {
    println!("PID: {}", snapshot.pid);
    println!("RSS: {} MB", snapshot.rss_mb);
    println!("Threads: {}", snapshot.threads);
    println!("FDs: {}", snapshot.fd_count);
    println!("TCP Connections: {}", snapshot.tcp_connections.len());

    if alerts.is_empty() {
        println!("Status: OK");
    } else {
        println!("Status: ALERT");
        println!("Alerts:");
        for alert in alerts {
            println!("- {}", alert.message);
        }
    }

    println!();
}

fn evaluate_alerts(
    baseline: &ProcessSnapshot,
    previous: &ProcessSnapshot,
    current: &ProcessSnapshot,
    thresholds: &Thresholds,
) -> Vec<Alert> {
    let mut alerts = Vec::new();

    if let Some(max_rss_mb) = thresholds.max_rss_mb {
        if current.rss_mb > max_rss_mb {
            alerts.push(Alert {
                message: format!(
                    "RSS exceeds configured threshold: {} MB > {} MB",
                    current.rss_mb, max_rss_mb
                ),
            });
        }
    }

    if baseline.rss_mb > 0 && current.rss_mb > baseline.rss_mb {
        let growth_percent =
            ((current.rss_mb - baseline.rss_mb) as f64 / baseline.rss_mb as f64) * 100.0;

        if growth_percent >= thresholds.max_rss_growth_percent {
            alerts.push(Alert {
                message: format!(
                    "RSS increased by {:.1} percent since baseline",
                    growth_percent
                ),
            });
        }
    }

    if current.threads > previous.threads + thresholds.thread_jump {
        alerts.push(Alert {
            message: format!(
                "Thread count jumped from {} to {}",
                previous.threads, current.threads
            ),
        });
    }

    let previous_connections: HashSet<&String> = previous.tcp_connections.iter().collect();

    for connection in &current.tcp_connections {
        if !previous_connections.contains(connection) {
            alerts.push(Alert {
                message: format!("New TCP socket observed: {connection}"),
            });
        }
    }

    alerts
}
