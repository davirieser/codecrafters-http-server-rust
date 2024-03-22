// Uncomment this block to pass the first stage
use std::net::TcpListener;
use std::io::{Write, Read};

fn main() {
    const BUFFER_SIZE : usize = 1024;
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let mut buf : [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
                loop {
                    match _stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(_n) => {
                            println!("Read: {} Bytes", _n);
                            if _n < BUFFER_SIZE {
                                break;
                            }
                        },
                        Err(e) => panic!("Error reading Data: {}", e)
                    }
                }

                let _ = _stream.write(&mut "HTTP/1.1 200 OK\r\n\r\n".as_bytes());
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
