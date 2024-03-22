use std::net::{TcpListener, TcpStream};
use std::io::{Write, Read};
use std::thread;
use std::env;
use std::sync::Arc;
use std::path::PathBuf;
use std::fs;
use std::convert::TryFrom;

#[derive(PartialEq, Eq, Debug)]
enum HttpMethod {
    GET,
    POST,
    PUT,
}

impl TryFrom<&str> for HttpMethod {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "GET" => Ok(HttpMethod::GET),
            "POST" => Ok(HttpMethod::POST),
            "PUT" => Ok(HttpMethod::PUT),
            _ => Err(()),
        }
    } 
}

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

fn send_file(stream: &mut TcpStream, path: &str, dir: &str) {
    let file_name = path.strip_prefix("/files/").unwrap();
    let absolute_path = format!("{dir}{file_name}");
    
    let file_path = PathBuf::from(absolute_path);

    if !file_path.exists() || !file_path.is_file() {
        let _ = write!(stream, "HTTP/1.1 404 Not Found\r\n\r\n");
    } else {
        match fs::read(file_path) {
            Ok(contents) => {
                let len = contents.len();

                let _ = write!(stream, "HTTP/1.1 200 OK\r\n");
                let _ = write!(stream, "Content-Type: application/octet-stream\r\n");
                let _ = write!(stream, "Content-Length: {len}\r\n\r\n");

                let _ = stream.write(&contents);
            }
            Err(e) => panic!("Could not open File: {e}"),
        }
    }
}

fn save_file(stream: &mut TcpStream, path: &str, dir: &str, body: String) {
    let file_name = path.strip_prefix("/files/").unwrap();
    let absolute_path = format!("{dir}{file_name}");
    
    let file_path = PathBuf::from(absolute_path);

    match fs::write(file_path, body) {
        Ok(()) => {
            let _ = write!(stream, "HTTP/1.1 201 Created\r\n\r\n");
        }
        Err(_) => {
            let _ = write!(stream, "HTTP/1.1 500 Internal Server Error\r\n\r\n");
        }
    }
}

fn handle_connection(stream: &mut TcpStream, dir: Arc<Option<String>>) {
    match read_to_string(stream) {
        Some(buf) => {
            let mut lines = buf.lines();
            let (method, path, version) = match lines.next() {
                Some(line) => {
                    let parts : Vec<&str> = line.splitn(3, ' ').collect();
                    (parts[0], parts[1], parts[2])
                },
                None => panic!("Empty Request!")
            };
            let method = HttpMethod::try_from(method).expect("Error parsing HTTP Method");

            let headers = lines.by_ref().take_while(|line| line.is_empty());
            let headers : Vec<(&str, &str)> = headers.filter_map(split_header).collect();

            let body = lines.fold(String::new(), |a, b| a + b + "\n"); 

            match path {
                "/" => {
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\n\r\n");
                },
                "/user-agent" => {
                    match headers.iter().find(|(key, _)| *key == "User-Agent") {
                        Some((_, user_agent)) => {
                            let len = user_agent.len();

                            let _ = write!(stream, "HTTP/1.1 200 OK\r\n");
                            let _ = write!(stream, "Content-Type: text/plain\r\n");
                            let _ = write!(stream, "Content-Length: {len}\r\n");

                            let _ = write!(stream, "\r\n{user_agent}");
                        }
                        None => {
                            let _ = write!(stream, "HTTP/1.1 404 Not Found\r\n\r\n");
                        }
                    }
                }
                _ if path.starts_with("/echo/") => {
                    let message = path.strip_prefix("/echo/").unwrap();
                    let len = message.len();

                    let _ = write!(stream, "HTTP/1.1 200 OK\r\n");
                    let _ = write!(stream, "Content-Type: text/plain\r\n");
                    let _ = write!(stream, "Content-Length: {len}\r\n");

                    let _ = write!(stream, "\r\n{message}");
                }
                _ if path.starts_with("/files/") => {
                    match dir.as_ref() {
                        Some(dir) => {
                            if method == HttpMethod::GET {
                                send_file(stream, path, dir);
                            } else if method == HttpMethod::POST {
                                save_file(stream, path, dir, body);
                            }
                        }
                        None => {
                            let _ = write!(stream, "HTTP/1.1 404 Not Found\r\n\r\n");
                        }
                    }
                }
                _ => {
                    let _ = write!(stream, "HTTP/1.1 404 Not Found\r\n\r\n");
                }
            }
        },
        None => panic!("Error reading Data"),
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let dir = args
        .iter()
        .position(|arg| arg == "--directory")
        .and_then(|idx| args.get(idx + 1).cloned())
        .map(|dir| 
            if !dir.ends_with("/") {
                format!("{dir}/")
            } else {
                dir
            }
        );

    let arc = Arc::new(dir);

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    let mut handles = vec![];

    for stream in listener.incoming() {
        let dir_ref = arc.clone();
        match stream {
            Ok(mut stream) => {
                handles.push(thread::spawn(move || handle_connection(&mut stream, dir_ref)));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }

    for handle in handles {
        let _ = handle.join();
    }
}
