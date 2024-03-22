use std::net::TcpListener;
use std::io::{Write, Read};

fn split_header(header: &str) -> Option<(&str, &str)> {
    let mut iter = header.splitn(2, ':');

    let key = iter.next()?; 
    let value = iter.next()?;

    Some((key, value))
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let mut buf = String::new();
                match _stream.read_to_string(&mut buf) {
                    Ok(_n) => {
                        let mut lines = buf.lines();
                        let (method, path, version) = match lines.next() {
                            Some(line) => {
                                let parts : Vec<&str> = line.splitn(3, ' ').collect();
                                (parts[0], parts[1], parts[2])
                            },
                            None => panic!("Empty Request!")
                        };

                        let headers : Vec<(&str, &str)> = lines.filter_map(split_header).collect();

                        if path == "/" {
                            let _ = _stream.write(&mut "HTTP/1.1 200 OK\r\n\r\n".as_bytes());
                        } else {
                            let _ = _stream.write(&mut "HTTP/1.1 404 Not Found\r\n\r\n".as_bytes());
                        }
                    },
                    Err(e) => panic!("Error reading Data: {}", e),
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
