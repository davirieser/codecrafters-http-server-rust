use std::net::TcpListener;
use std::io::{Write, Read};

fn split_header(header: &str) -> Option<(&str, &str)> {
    let mut iter = header.splitn(2, ':');

    let key = iter.next()?; 
    let value = iter.next()?.trim_start();

    Some((key, value))
}

fn read_to_string<R: Read>(stream: &mut R) -> Option<String> {
    const BUFFER_SIZE : usize = 1024;

    let mut buf = [0 as u8; 1024];
    let mut vec = Vec::new();

    loop {
        match stream.read(&mut buf) {
            Ok(n) => {
                vec.extend_from_slice(&buf);
                if n < BUFFER_SIZE {
                    break;
                }
            }
            Err(e) => {
                return None;
            }
        }
    }

    String::from_utf8(vec).ok()
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                match read_to_string(&mut _stream) {
                    Some(buf) => {
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
                    None => panic!("Error reading Data"),
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
