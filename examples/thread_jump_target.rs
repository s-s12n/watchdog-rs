use std::thread;
use std::time::Duration;

fn main() {
    println!("PID: {}", std::process::id());

    println!("waiting before spawning threads...");
    thread::sleep(Duration::from_secs(20));

    let mut handles = Vec::new();

    for i in 0..20 {
        let handle = thread::spawn(move || {
            println!("worker thread {i} started");
            thread::sleep(Duration::from_secs(60));
        });

        handles.push(handle);
    }

    println!("spawned {} threads", handles.len());

    for handle in handles {
        let _ = handle.join();
    }
}
