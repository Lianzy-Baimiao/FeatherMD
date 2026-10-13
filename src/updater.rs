//! The only network access in FeatherMD: an opt-in HTTPS GET of the public
//! GitHub "latest release" record through the system WinHTTP component.
//! Nothing is downloaded, installed or uploaded; documents and settings never
//! leave the machine. Author / company / last editor: lianzy.
use feathermd::i18n::{tr, trf};
use feathermd::update::{self, LATEST_API, MAX_RESPONSE, Release};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::Networking::WinHttp::*;

pub type Outcome = Result<Release, String>;

/// Test seam: when set, checks go to a local `http://127.0.0.1:<port>/` server.
/// Any other value is ignored, so this can never redirect checks off the machine.
const TEST_URL: &str = "FEATHERMD_TEST_UPDATE_URL";
const USER_AGENT: &str = concat!("FeatherMD/", env!("CARGO_PKG_VERSION"));
/// A check always ends within this time, whatever the network does.
const DEADLINE: Duration = Duration::from_secs(15);

/// One check in flight on a background thread. WinHTTP's own response timeout
/// is not reliable, so the UI enforces the deadline when polling.
pub struct Pending {
    rx: mpsc::Receiver<Outcome>,
    deadline: Instant,
}
impl Pending {
    /// `None` while still waiting. A slow thread is abandoned, not joined; its
    /// WinHTTP handles close when it returns.
    pub fn poll(&self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(tr("更新检查意外中止").into())),
            Err(mpsc::TryRecvError::Empty) if Instant::now() >= self.deadline => {
                Some(Err(tr("连接超时，请检查网络或代理").into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}

pub fn start() -> Pending {
    let url = std::env::var(TEST_URL)
        .ok()
        .filter(|url| loopback(url))
        .unwrap_or_else(|| LATEST_API.to_string());
    start_at(url, DEADLINE)
}

fn start_at(url: String, deadline: Duration) -> Pending {
    let (tx, rx) = mpsc::channel();
    let worker = tx.clone();
    let started = thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            let _ = worker.send(get(&url, deadline).and_then(|body| update::parse_latest(&body)));
        });
    if started.is_err() {
        let _ = tx.send(Err(tr("无法启动更新检查").into()));
    }
    Pending {
        rx,
        deadline: Instant::now() + deadline,
    }
}

/// `http://127.0.0.1:<port>/...` only: no names, other addresses or credentials.
fn loopback(url: &str) -> bool {
    url.strip_prefix("http://127.0.0.1:")
        .and_then(|rest| {
            let end = rest.find('/').unwrap_or(rest.len());
            rest[..end].parse::<u16>().ok()
        })
        .is_some_and(|port| port != 0)
}

struct Url<'a> {
    secure: bool,
    host: &'a str,
    port: u16,
    path: &'a str,
}
fn split(url: &str) -> Option<Url<'_>> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else {
        (false, url.strip_prefix("http://")?)
    };
    let slash = rest.find('/').unwrap_or(rest.len());
    let (authority, path) = rest.split_at(slash);
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    if host.is_empty() || authority.contains('@') {
        return None;
    }
    Some(Url {
        secure,
        host,
        port,
        path: if path.is_empty() { "/" } else { path },
    })
}

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { WinHttpCloseHandle(self.0) };
    }
}
fn open(handle: *mut core::ffi::c_void) -> Result<Handle, String> {
    if handle.is_null() {
        Err(failure())
    } else {
        Ok(Handle(handle))
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn get(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let target = split(url).ok_or(tr("更新地址无效"))?;
    // Plain HTTP is only for the loopback test server; GitHub is always HTTPS.
    if !target.secure && !loopback(url) {
        return Err(tr("更新地址无效").into());
    }
    let agent = wide(USER_AGENT);
    let session = unsafe {
        let access = if loopback(url) {
            WINHTTP_ACCESS_TYPE_NO_PROXY
        } else {
            // Uses the user's Windows proxy settings, which many users need for GitHub.
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
        };
        let mut session = WinHttpOpen(
            agent.as_ptr(),
            access,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() && access == WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY {
            session = WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            );
        }
        open(session)?
    };
    let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    unsafe { WinHttpSetTimeouts(session.0, ms, ms, ms, ms) };
    // SetTimeouts does not cover waiting for response headers (90 s by default).
    let wait = ms as u32;
    unsafe {
        WinHttpSetOption(
            session.0,
            WINHTTP_OPTION_RECEIVE_RESPONSE_TIMEOUT,
            (&wait as *const u32).cast(),
            size_of::<u32>() as u32,
        )
    };
    let host = wide(target.host);
    let connection = open(unsafe { WinHttpConnect(session.0, host.as_ptr(), target.port, 0) })?;
    let verb = wide("GET");
    let path = wide(target.path);
    let request = open(unsafe {
        WinHttpOpenRequest(
            connection.0,
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            if target.secure {
                WINHTTP_FLAG_SECURE
            } else {
                0
            },
        )
    })?;
    // Certificate validation stays at WinHTTP's defaults; never relax it.
    let headers =
        wide("Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n");
    let sent = unsafe {
        WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            u32::MAX,
            std::ptr::null(),
            0,
            0,
            0,
        ) != 0
            && WinHttpReceiveResponse(request.0, std::ptr::null_mut()) != 0
    };
    if !sent {
        return Err(failure());
    }
    let mut status = 0u32;
    let mut size = size_of::<u32>() as u32;
    if unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(failure());
    }
    match status {
        200 => {}
        403 | 429 => return Err(tr("GitHub 暂时限制了访问频率，请稍后再试").into()),
        404 => return Err(tr("没有找到正式发布").into()),
        407 => return Err(tr("代理服务器需要身份验证").into()),
        code => return Err(trf("GitHub 返回 HTTP {}", &[&code])),
    }
    let mut body = Vec::new();
    loop {
        let mut available = 0u32;
        if unsafe { WinHttpQueryDataAvailable(request.0, &mut available) } == 0 {
            return Err(failure());
        }
        if available == 0 {
            return Ok(body);
        }
        if body.len() + available as usize > MAX_RESPONSE {
            return Err(tr("更新信息过大").into());
        }
        let start = body.len();
        body.resize(start + available as usize, 0);
        let mut read = 0u32;
        if unsafe {
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                available,
                &mut read,
            )
        } == 0
        {
            return Err(failure());
        }
        body.truncate(start + read as usize);
    }
}

fn failure() -> String {
    let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
    match code {
        ERROR_WINHTTP_TIMEOUT => tr("连接超时，请检查网络或代理").into(),
        ERROR_WINHTTP_NAME_NOT_RESOLVED => tr("无法解析 GitHub 地址，请检查网络").into(),
        ERROR_WINHTTP_CANNOT_CONNECT | ERROR_WINHTTP_CONNECTION_ERROR => {
            tr("无法连接到 GitHub，请检查网络或代理").into()
        }
        ERROR_WINHTTP_SECURE_FAILURE => tr("安全连接验证失败，已停止检查").into(),
        ERROR_WINHTTP_INVALID_SERVER_RESPONSE => tr("服务器响应无效").into(),
        code => trf("网络错误（{}）", &[&code]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    /// One-shot loopback server: returns its URL and a handle yielding the request text.
    fn serve(response: Vec<u8>, delay_ms: u64) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/repos/Lianzy-Baimiao/FeatherMD/releases/latest",
            listener.local_addr().unwrap().port()
        );
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 2048];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buffer[..n]),
                }
            }
            thread::sleep(Duration::from_millis(delay_ms));
            let _ = stream.write_all(&response);
            String::from_utf8_lossy(&request).into_owned()
        });
        (url, server)
    }
    fn reply(status: &str, body: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn loopback_override_cannot_leave_the_machine() {
        for good in [
            "http://127.0.0.1:8080/",
            "http://127.0.0.1:1/x/latest",
            "http://127.0.0.1:65535",
        ] {
            assert!(loopback(good), "{good}");
        }
        for bad in [
            "https://127.0.0.1:8080/",
            "http://localhost:8080/",
            "http://127.0.0.1/",
            "http://127.0.0.1:0/",
            "http://127.0.0.1:99999/",
            "http://127.0.0.1:80@evil.example/",
            "http://127.0.0.1.evil.example:80/",
            "http://10.0.0.1:80/",
            "http://[::1]:80/",
            "",
        ] {
            assert!(!loopback(bad), "{bad}");
        }
        let api = split(LATEST_API).unwrap();
        assert!(api.secure && api.port == 443 && api.host == "api.github.com");
        assert_eq!(api.path, "/repos/Lianzy-Baimiao/FeatherMD/releases/latest");
        assert!(split("http://user@127.0.0.1:80/").is_none());
        assert!(split("ftp://127.0.0.1:80/").is_none());
        // Non-loopback plain HTTP is refused before any connection is attempted.
        assert_eq!(
            get("http://example.com/", Duration::from_secs(1)).unwrap_err(),
            "更新地址无效"
        );
    }

    #[test]
    fn get_sends_only_public_api_headers_and_returns_the_body() {
        let body =
            br#"{"tag_name":"v9.0.0","name":"FeatherMD 9","published_at":"2027-01-02T03:04:05Z"}"#;
        let (url, server) = serve(reply("200 OK", body), 0);
        let received = get(&url, Duration::from_secs(5)).unwrap();
        assert_eq!(received, body);
        let request = server.join().unwrap().to_ascii_lowercase();
        assert!(
            request.starts_with("get /repos/lianzy-baimiao/feathermd/releases/latest http/1.1\r\n")
        );
        assert!(request.contains(&format!(
            "user-agent: {}\r\n",
            USER_AGENT.to_ascii_lowercase()
        )));
        assert!(request.contains("accept: application/vnd.github+json"));
        assert!(request.contains("x-github-api-version: 2022-11-28"));
        for private in ["cookie", "authorization", "referer"] {
            assert!(!request.contains(&format!("\r\n{private}:")), "{private}");
        }
        assert_eq!(update::parse_latest(&received).unwrap().tag, "v9.0.0");
    }

    #[test]
    fn http_errors_limits_and_timeouts_are_explained() {
        for (status, message) in [
            ("404 Not Found", "没有找到正式发布"),
            ("403 Forbidden", "GitHub 暂时限制了访问频率，请稍后再试"),
            (
                "429 Too Many Requests",
                "GitHub 暂时限制了访问频率，请稍后再试",
            ),
            ("500 Internal Server Error", "GitHub 返回 HTTP 500"),
        ] {
            let (url, server) = serve(reply(status, b"{}"), 0);
            assert_eq!(get(&url, Duration::from_secs(5)).unwrap_err(), message);
            server.join().unwrap();
        }
        let (url, server) = serve(reply("200 OK", &vec![b' '; MAX_RESPONSE + 4096]), 0);
        assert_eq!(
            get(&url, Duration::from_secs(5)).unwrap_err(),
            "更新信息过大"
        );
        server.join().unwrap();
    }

    #[test]
    fn pending_checks_finish_and_stalled_ones_hit_the_deadline() {
        let body = br#"{"tag_name":"v9.1.0"}"#;
        let (url, server) = serve(reply("200 OK", body), 0);
        let pending = start_at(url, Duration::from_secs(5));
        let outcome = loop {
            if let Some(outcome) = pending.poll() {
                break outcome;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(outcome.unwrap().tag, "v9.1.0");
        server.join().unwrap();
        // The server answers after 3 s; the deadline ends the check at about 0.6 s.
        let (url, server) = serve(reply("200 OK", body), 3000);
        let started = Instant::now();
        let pending = start_at(url, Duration::from_millis(600));
        let outcome = loop {
            if let Some(outcome) = pending.poll() {
                break outcome;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(outcome.unwrap_err(), "连接超时，请检查网络或代理");
        assert!(
            started.elapsed() < Duration::from_millis(2500),
            "{:?}",
            started.elapsed()
        );
        drop(pending);
        server.join().unwrap();
    }
}
