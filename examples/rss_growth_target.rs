use std::thread;
use std::time::Duration;

fn main() {
    println!("PID: {}", std::process::id());

    let mut chunks: Vec<Vec<u8>> = Vec::new();

    loop {
        let mut chunk = vec![0_u8; 10 * 1024 * 1024];

        for byte in chunk.iter_mut() {
            *byte = 1;
        }

        chunks.push(chunk);

        println!("allocated and touched {} MB", chunks.len() * 10);

        thread::sleep(Duration::from_secs(1));
    }
}
