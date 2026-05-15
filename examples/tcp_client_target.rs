use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn main() {
    println!("TCP client PID: {}", std::process::id());
    println!("waiting 20 seconds before opening TCP connections...");
    thread::sleep(Duration::from_secs(20));

    let mut sockets = Vec::new();

    loop {
        match TcpStream::connect("127.0.0.1:8080") {
            Ok(stream) => {
                println!("opened TCP connection {}", sockets.len() + 1);
                sockets.push(stream);
            }
            Err(error) => {
                eprintln!("connect failed: {error}");
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}
