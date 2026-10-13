//! Test-only stand-in for the GitHub Releases API, used by scripts/update-smoke.ps1.
//! Binds 127.0.0.1 on an ephemeral port and writes it to `<dir>/port`. Each request
//! is appended to `<dir>/requests.log`, then answered with `<dir>/status`
//! (default "200 OK") and `<dir>/body`, after `<dir>/delay` milliseconds if present.
//! Exits when `<dir>/stop` exists. Never listens on other interfaces.
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

fn read(dir: &Path, name: &str) -> Option<String> {
    fs::read_to_string(dir.join(name))
        .ok()
        .map(|text| text.trim().to_string())
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: update_server <directory>");
        return;
    };
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    listener
        .set_nonblocking(true)
        .expect("non-blocking listener");
    let port = listener.local_addr().expect("local address").port();
    // Write then rename, so the script never reads a partial port number.
    let partial = dir.join("port.partial");
    fs::write(&partial, port.to_string()).expect("write port");
    fs::rename(&partial, dir.join("port")).expect("publish port");
    while !dir.join("stop").exists() {
        let Ok((mut stream, _)) = listener.accept() else {
            thread::sleep(Duration::from_millis(20));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 65536 {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => request.extend_from_slice(&buffer[..n]),
            }
        }
        if let Ok(mut log) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("requests.log"))
        {
            let _ = log.write_all(&request);
            let _ = log.write_all(b"=====\r\n");
        }
        let delay = read(&dir, "delay")
            .and_then(|delay| delay.parse().ok())
            .unwrap_or(0);
        thread::sleep(Duration::from_millis(delay));
        let status = read(&dir, "status").unwrap_or_else(|| "200 OK".into());
        let body = fs::read(dir.join("body")).unwrap_or_default();
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
    }
}
