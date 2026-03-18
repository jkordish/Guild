#![allow(dead_code)]

use std::fmt::Write as FmtWrite;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const JSON_BODY: &str = "{\"service\":\"guild-http\",\"message\":\"deterministic\",\"nested\":{\"count\":2},\"items\":[{\"name\":\"alpha\"},{\"name\":\"beta\"}]}";
const LARGE_BODY_BYTES: usize = 8 * 1024;
const SLOW_RESPONSE_MS: u64 = 250;

pub struct HttpTestServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl HttpTestServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("local HTTP test server binds");
        listener
            .set_nonblocking(true)
            .expect("local HTTP test server configures nonblocking accept");
        let addr = listener
            .local_addr()
            .expect("local HTTP test server exposes local addr");
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || serve(&listener, &thread_shutdown));

        Self {
            addr,
            shutdown,
            handle: Some(handle),
        }
    }

    pub fn host() -> &'static str {
        "127.0.0.1"
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", Self::host(), self.port())
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url(), path)
    }

    pub fn json_url(&self) -> String {
        self.url("/json")
    }

    pub fn slow_json_url(&self) -> String {
        self.url("/slow")
    }

    pub fn large_json_url(&self) -> String {
        self.url("/large")
    }

    pub fn redirect_json_url(&self) -> String {
        self.url("/redirect-json")
    }

    pub fn redirect_chain_url(&self) -> String {
        self.url("/redirect-chain-1")
    }

    pub fn localhost_json_url(&self) -> String {
        format!("http://localhost:{}/json", self.port())
    }
}

impl Drop for HttpTestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn large_response_bytes() -> usize {
    large_json_body().len()
}

pub fn slow_response_ms() -> u64 {
    SLOW_RESPONSE_MS
}

fn serve(listener: &TcpListener, shutdown: &AtomicBool) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_connection(stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    let (status_code, status_text, body, delay_ms, location) = match path {
        "/json" => (200, "OK", JSON_BODY.to_owned(), 0, None),
        "/slow" => (200, "OK", JSON_BODY.to_owned(), SLOW_RESPONSE_MS, None),
        "/large" => (200, "OK", large_json_body(), 0, None),
        "/redirect-json" => (
            302,
            "Found",
            "{\"redirect\":\"json\"}".to_owned(),
            0,
            Some("/json"),
        ),
        "/redirect-chain-1" => (
            302,
            "Found",
            "{\"redirect\":\"chain-1\"}".to_owned(),
            0,
            Some("/redirect-chain-2"),
        ),
        "/redirect-chain-2" => (
            302,
            "Found",
            "{\"redirect\":\"chain-2\"}".to_owned(),
            0,
            Some("/json"),
        ),
        _ => (
            404,
            "Not Found",
            "{\"error\":\"not-found\"}".to_owned(),
            0,
            None,
        ),
    };

    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    let content_length = body.len();
    let mut response_head = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {content_length}\r\nConnection: close\r\n"
    );
    if let Some(location) = location {
        write!(&mut response_head, "Location: {location}\r\n")
            .expect("writing into a String cannot fail");
    }
    response_head.push_str("\r\n");
    stream.write_all(response_head.as_bytes())?;
    if method != "HEAD" {
        stream.write_all(body.as_bytes())?;
    }
    stream.flush()?;

    Ok(())
}

fn large_json_body() -> String {
    format!(
        "{{\"service\":\"guild-http\",\"payload\":\"{}\"}}",
        "x".repeat(LARGE_BODY_BYTES)
    )
}
