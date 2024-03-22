// Uncomment this block to pass the first stage
use std::net::TcpListener;
use std::io::{Write, Read};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let mut buf : [u8; 1024] = [0; 1024];
                loop {
                    match _stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(_n) => {},
                        Err(e) => eprintln!("Error reading Data: {}", e)
                    }
                }

                let _ = write!(_stream, "HTTP/1.1 200 OK\r\n\r\n");
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
