use std::io;
use std::env;
use std::sync::Arc;
use std::path::PathBuf;
use std::convert::TryFrom;
use std::collections::HashMap;
use std::fmt::{Debug, Display};

use anyhow::{Error, Result};

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::fs::File;

#[derive(Debug)]
struct Request<'a> {
    method: HttpMethod,
    path: &'a str,
    http_version: &'a str,
    headers: HashMap<&'a str, HeaderValue<'a>>, 
    body: &'a str,
}

struct Response {
    status_code: HttpStatusCode,
    headers: Vec<(String, String)>,
    body: String,
}

impl Response {
    pub fn new(status_code: HttpStatusCode, headers: Vec<(String, String)>, body: String) -> Self {
        Response {
            status_code,
            headers,
            body,
        }
    }
    pub fn new_without_body(status_code: HttpStatusCode, headers: Vec<(String, String)>) -> Self {
        Response {
            status_code,
            headers,
            body: String::with_capacity(0),
        }
    }
    pub async fn write_to<W>(&self, w: &mut W) -> io::Result<usize>
    where 
        W: AsyncWriteExt + Unpin
    {
        let status_code_int = usize::from(self.status_code);
        let mut buf = format!("HTTP/1.1 {status_code_int} {}\r\n", self.status_code);

        for (key, value) in &self.headers {
            buf += key;
            buf += ": ";
            buf += value;
            buf += "\r\n";
        }

        buf += "\r\n";

        buf += &self.body;

        w.write(buf.as_bytes()).await
    }
}

impl From<HttpStatusCode> for Response {
    fn from(sc: HttpStatusCode) -> Self {
        Response::new(sc, Vec::with_capacity(0), String::with_capacity(0))
    }
}

#[derive(Debug, PartialEq)]
enum HeaderValue<'a> {
    Single(&'a str),
    Multiple(Vec<&'a str>),
}

#[derive(PartialEq, Eq, Debug)]
enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
    TRACE,
    CONNECT,
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy)]
enum HttpStatusCode {
    Ok,
    Created,

    BadRequest,
    NotFound,

    InternalServerError,
}

impl From<HttpStatusCode> for usize {
    fn from(sc: HttpStatusCode) -> usize {
        match sc {
            HttpStatusCode::Ok => 200,
            HttpStatusCode::Created => 201,
            HttpStatusCode::BadRequest => 400,
            HttpStatusCode::NotFound => 404,
            HttpStatusCode::InternalServerError => 500,
        }
    }
}

impl Display for HttpStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Ok => "OK",
            Self::Created => "Created",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::InternalServerError => "Internal Server Error",
        };

        write!(f, "{str}")
    }
}

impl TryFrom<&str> for HttpMethod {
    type Error = ();

    fn try_from(method: &str) -> Result<Self, Self::Error> {
        match method.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::GET),
            "POST" => Ok(HttpMethod::POST),
            "PUT" => Ok(HttpMethod::PUT),
            "DELETE" => Ok(HttpMethod::DELETE),
            "PATCH" => Ok(HttpMethod::PATCH),
            "HEAD" => Ok(HttpMethod::HEAD),
            "OPTIONS" => Ok(HttpMethod::OPTIONS),
            "TRACE" => Ok(HttpMethod::TRACE),
            "CONNECT" => Ok(HttpMethod::CONNECT),
            _ => Err(()),
        }
    } 
}

#[derive(Debug)]
enum RouteError {
    NoMatch,
    Error(Error),
}

impl std::error::Error for RouteError {}

impl Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteError::NoMatch => write!(f, "Route did not match Path!"),
            RouteError::Error(e) => write!(f, "Internal Error: {e}"),
        }
    }
}

struct RouteDefinition<T, M, A>
where 
    M: Fn(&str) -> Option<T>,
    A: Fn(Request, T) -> Result<Response>,
{
    matches: M,
    action: A,
}

struct Route
{
    run: Box<dyn Fn(Request<'_>) -> Result<Response>>,
}

impl<T, M, A> From<RouteDefinition<T, M, A>> for Route
where
    M: Fn(&str) -> Option<T> + 'static,
    A: Fn(Request, T) -> Result<Response> + 'static,
{
    fn from(definition: RouteDefinition<T, M, A>) -> Self {
        let run = Box::new(move |request: Request| {
            match (definition.matches)(request.path) {
                Some(matches) => (definition.action)(request, matches),
                None => Err(Error::new(RouteError::NoMatch)),
            }
        });

        Self { run }
    }
}

fn split_header(header: &str) -> Option<(&str, &str)> {
    let mut iter = header.splitn(2, ':');

    let key = iter.next()?; 
    let value = iter.next()?.trim_start();

    Some((key, value))
}

async fn read_to_string<R: AsyncReadExt + std::marker::Unpin>(stream: &mut R) -> Result<String> {
    const BUFFER_SIZE : usize = 1024;

    let mut buf = [0 as u8; 1024];
    let mut vec = Vec::new();

    loop {
        match stream.read(&mut buf).await {
            Ok(n) => {
                vec.extend_from_slice(&buf[..n]);
                if n < BUFFER_SIZE {
                    return Ok(String::from_utf8(vec)?);
                }
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

async fn send_file(stream: &mut TcpStream, path: &str, dir: &str) -> io::Result<u64> {
    let file_name = path.strip_prefix("/files/").unwrap();
    let absolute_path = format!("{dir}{file_name}");
    
    let file_path = PathBuf::from(absolute_path);

    let mut file = File::open(file_path).await?;
    let size = file.metadata().await?.len();
    
    let response = Response::new_without_body(
        HttpStatusCode::Ok, 
        vec![
            ("Content-Type".to_string(), "application/octet-stream".to_string()),
            ("Content-Length".to_string(), format!("{size}"))
        ],
    );

    response.write_to(stream).await?;
    tokio::io::copy(&mut file, stream).await
}

async fn save_file(stream: &mut TcpStream, path: &str, dir: &str, body: String) -> io::Result<usize> {
    let file_name = path.strip_prefix("/files/").unwrap();
    let absolute_path = format!("{dir}{file_name}");
    
    let file_path = PathBuf::from(absolute_path);

    let mut file = File::open(file_path).await?;
    let res = file.write(body.as_bytes()).await;
    Response::from(res
        .as_ref()
        .map_or_else(|_e| HttpStatusCode::InternalServerError, |_v| HttpStatusCode::Ok)
    ).write_to(stream).await?;
        
    res
}

async fn handle_connection(mut stream: TcpStream, dir: Arc<Option<String>>) {
    match read_to_string(&mut stream).await {
        Ok(buf) => {
            let mut lines = buf.lines();
            let header_line = lines.next()
                .and_then(|line| line.find(' ').and_then(|i| Some((line, i))))
                .and_then(|(line, idx1)| match (idx1, line.rfind(' ')) {
                    (idx1, Some(idx2)) if idx1 != idx2 => Some((*&line[..idx1].trim(), *&line[idx1+1..idx2].trim(), *&line[idx2+1..].trim())),
                    (_, _) => None,
                });
            let (method, path, version) = match header_line {
                Some(t) => t,
                None => {
                    println!("Invalid Request");
                    Response::from(HttpStatusCode::BadRequest).write_to(&mut stream).await;
                    panic!("");
                }
            };

            let method = HttpMethod::try_from(method).expect("Error parsing HTTP Method");

            println!("Method: {method}, Path: {path}, Version: {version}");

            let headers = lines.by_ref().take_while(|line| !line.is_empty());
            let headers : Vec<(&str, &str)> = headers.filter_map(split_header).collect();

            let mut body = lines.fold(String::new(), |a, b| a + b + "\n"); 
            if !body.is_empty() {
                // NOTE: Remove last newline that is inserted by the fold.
                body.truncate(body.len() - 1);
            }

            match path {
                "/" => {
                    Response::from(HttpStatusCode::Ok).write_to(&mut stream).await;
                },
                "/user-agent" => {
                    match headers.iter().find(|(key, _)| *key == "User-Agent") {
                        Some((_, user_agent)) => {
                            let len = format!("{}", user_agent.len());

                            let response = Response::new(
                                HttpStatusCode::Ok,
                                vec![
                                    ("Content-Type".to_string(), "text/plain".to_string()),
                                    ("Content-Length".to_string(), len),
                                ],
                                user_agent.to_string()
                            );

                            response.write_to(&mut stream).await;
                        }
                        None => {
                            Response::from(HttpStatusCode::NotFound).write_to(&mut stream).await;
                        }
                    }
                }
                _ if path.starts_with("/echo/") => {
                    let message = path.strip_prefix("/echo/").unwrap();
                    let len = format!("{}", message.len());

                    let response = Response::new(
                        HttpStatusCode::Ok,
                        vec![
                            ("Content-Type".to_string(), "text/plain".to_string()),
                            ("Content-Length".to_string(), len),
                        ],
                        message.to_string()
                    );
                    
                    response.write_to(&mut stream).await;
                }
                _ if path.starts_with("/files/") => {
                    match dir.as_ref() {
                        Some(dir) => {
                            if method == HttpMethod::GET {
                                send_file(&mut stream, path, dir).await;
                            } else if method == HttpMethod::POST {
                                save_file(&mut stream, path, dir, body).await;
                            }
                        }
                        None => {
                            Response::from(HttpStatusCode::NotFound).write_to(&mut stream).await;
                        }
                    }
                }
                _ => {
                    Response::from(HttpStatusCode::NotFound).write_to(&mut stream).await;
                }
            }
        },
        Err(e) => println!("Error reading Data: {e}"),
    }
}

async fn main_loop() -> io::Result<()> {
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

    let listener = TcpListener::bind("127.0.0.1:4221").await.unwrap();

    loop {
        let (socket, socket_addr) = listener.accept().await?;
        
        println!("New Connection from {socket_addr}");

        let dir_ref = arc.clone();
        tokio::spawn(async move {
            handle_connection(socket, dir_ref).await
        });
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    tokio::select! {
        biased;
        v = main_loop() => v,
        v = signal::ctrl_c() => v,
    }
}
