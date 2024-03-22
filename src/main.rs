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

                        match path {
                            "/" => {
                                let _ = write!(_stream, "HTTP/1.1 200 OK\r\n\r\n");
                            },
                            "/user-agent" => {
                                match headers.iter().find(|(key, _)| *key == "User-Agent") {
                                    Some((_, user_agent)) => {
                                        let len = user_agent.len();

                                        let _ = write!(_stream, "HTTP/1.1 200 OK\r\n\r\n");
                                        let _ = write!(_stream, "Content-Type: text/plain\r\n");
                                        let _ = write!(_stream, "Content-Length: {len}\r\n");

                                        let _ = write!(_stream, "\r\n{user_agent}");
                                    }
                                    None => {
                                        let _ = write!(_stream, "HTTP/1.1 404 Not Found\r\n\r\n");
                                    }
                                }
                            }
                            _ if path.starts_with("/echo/") => {
                                let message = path.strip_prefix("/echo/").unwrap();
                                let len = message.len();

                                let _ = write!(_stream, "HTTP/1.1 200 OK\r\n");
                                let _ = write!(_stream, "Content-Type: text/plain\r\n");
                                let _ = write!(_stream, "Content-Length: {len}\r\n");

                                let _ = write!(_stream, "\r\n{message}");
                            }
                            _ => {
                                let _ = write!(_stream, "HTTP/1.1 404 Not Found\r\n\r\n");
                            }
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
