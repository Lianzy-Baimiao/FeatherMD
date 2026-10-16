//! One explicitly requested image, fetched off the UI thread. No cookies,
//! authentication, referer, redirects, disk cache or certificate exceptions.
use super::{Failure, rules};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use windows_sys::Win32::Networking::WinHttp::*;

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn handle(raw: *mut core::ffi::c_void) -> Result<Handle, Failure> {
    if raw.is_null() {
        Err(Failure::Network)
    } else {
        Ok(Handle(raw))
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn alive(cancel: &AtomicBool, deadline: Instant) -> Result<(), Failure> {
    if cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
        Err(Failure::Timeout)
    } else {
        Ok(())
    }
}
unsafe fn option(request: &Handle, name: u32, value: u32) -> Result<(), Failure> {
    if unsafe { WinHttpSetOption(request.0, name, (&value as *const u32).cast(), 4) } == 0 {
        Err(Failure::Network)
    } else {
        Ok(())
    }
}

pub(super) fn get(url: &str, cancel: &AtomicBool, deadline: Instant) -> Result<Vec<u8>, Failure> {
    let target = rules::WebImage::parse(url).ok_or(Failure::Refused)?;
    alive(cancel, deadline)?;
    unsafe {
        let agent = wide(concat!("FeatherMD/", env!("CARGO_PKG_VERSION")));
        let proxy = if target.host == "127.0.0.1" || target.host == "::1" {
            WINHTTP_ACCESS_TYPE_NO_PROXY
        } else {
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
        };
        let session = handle(WinHttpOpen(
            agent.as_ptr(),
            proxy,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ))?;
        if WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000) == 0 {
            return Err(Failure::Network);
        }
        let host = wide(&target.host);
        let connection = handle(WinHttpConnect(session.0, host.as_ptr(), target.port, 0))?;
        let verb = wide("GET");
        let path = wide(&target.resource);
        let request = handle(WinHttpOpenRequest(
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
        ))?;
        option(
            &request,
            WINHTTP_OPTION_DISABLE_FEATURE,
            WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_AUTHENTICATION,
        )?;
        option(
            &request,
            WINHTTP_OPTION_AUTOLOGON_POLICY,
            WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH,
        )?;
        option(&request, WINHTTP_OPTION_RECEIVE_RESPONSE_TIMEOUT, 5000)?;
        let headers = wide(
            "Accept: image/png, image/jpeg, image/gif, image/bmp, image/tiff, image/x-icon\r\n",
        );
        alive(cancel, deadline)?;
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            u32::MAX,
            std::ptr::null(),
            0,
            0,
            0,
        ) == 0
            || WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0
        {
            return Err(if Instant::now() >= deadline {
                Failure::Timeout
            } else {
                Failure::Network
            });
        }
        alive(cancel, deadline)?;
        let mut status = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(Failure::Network);
        }
        match status {
            200 => {}
            300..=399 => return Err(Failure::Redirect),
            _ => return Err(Failure::Network),
        }
        let mut length = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            (&mut length as *mut u32).cast(),
            &mut size,
            std::ptr::null_mut(),
        ) != 0
            && u64::from(length) > rules::MAX_BYTES
        {
            return Err(Failure::TooLarge);
        }
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            alive(cancel, deadline)?;
            let mut read = 0u32;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err(Failure::Network);
            }
            alive(cancel, deadline)?;
            if read == 0 {
                return Ok(bytes);
            }
            if bytes.len() as u64 + u64::from(read) > rules::MAX_BYTES {
                return Err(Failure::TooLarge);
            }
            bytes.extend_from_slice(&chunk[..read as usize]);
            // Reject HTML/SVG/error pages before buffering the whole response.
            if bytes.len() >= 8 && !rules::raster(&bytes) {
                return Err(Failure::Unreadable);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    fn serve(response: Vec<u8>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/image?raw=1&token=a%2Fb",
            listener.local_addr().unwrap()
        );
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 512];
            while !bytes.ends_with(b"\r\n\r\n") {
                let n = stream.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..n]);
            }
            let _ = stream.write_all(&response);
            String::from_utf8(bytes).unwrap()
        });
        (url, worker)
    }
    fn fetch(response: Vec<u8>) -> (Result<Vec<u8>, Failure>, String) {
        let (url, worker) = serve(response);
        let result = get(
            &url,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5),
        );
        (result, worker.join().unwrap())
    }
    #[test]
    fn anonymous_get_preserves_query_and_bounds_or_refuses_responses() {
        let body = b"\x89PNG\r\n\x1a\nfixture";
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        let (result, request) = fetch(response);
        assert_eq!(result.unwrap(), body);
        assert!(
            request.starts_with("GET /image?raw=1&token=a%2Fb HTTP/1.1\r\n"),
            "{request}"
        );
        for name in ["cookie:", "authorization:", "referer:"] {
            assert!(!request.to_ascii_lowercase().contains(name));
        }
        for (response, failure) in [
            (
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\n\r\n",
                Failure::Redirect,
            ),
            (
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
                Failure::Network,
            ),
            (
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=images\r\nContent-Length: 0\r\n\r\n",
                Failure::Network,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 33554433\r\n\r\n",
                Failure::TooLarge,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n<html>oops",
                Failure::Unreadable,
            ),
        ] {
            assert_eq!(fetch(response.as_bytes().to_vec()).0.err(), Some(failure));
        }
        let mut response =
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n\x89PNG\r\n\x1a\n".to_vec();
        response.resize(rules::MAX_BYTES as usize + 100, 0);
        assert_eq!(fetch(response).0.err(), Some(Failure::TooLarge));
    }
    #[test]
    fn cancelled_or_expired_requests_never_connect() {
        let url = "http://127.0.0.1:1/never";
        assert_eq!(
            get(
                url,
                &AtomicBool::new(true),
                Instant::now() + Duration::from_secs(1)
            )
            .err(),
            Some(Failure::Timeout)
        );
        assert_eq!(
            get(url, &AtomicBool::new(false), Instant::now()).err(),
            Some(Failure::Timeout)
        );
    }
}
