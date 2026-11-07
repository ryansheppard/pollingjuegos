use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub enum Callback {
    Code(String),
    Denied,
}

/// Bind before opening the browser so the port in the login URL is already live.
pub fn bind() -> io::Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

pub fn wait_for_callback(
    listener: &TcpListener,
    timeout: Duration,
) -> io::Result<Option<Callback>> {
    // std has no accept timeout, so poll rather than block forever if the user
    // abandons the browser tab.
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                // Favicon requests and dropped connections shouldn't end the wait.
                if let Ok(Some(cb)) = handle(stream) {
                    return Ok(Some(cb));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

fn handle(stream: TcpStream) -> io::Result<Option<Callback>> {
    // Accepted sockets inherit O_NONBLOCK on macOS (not Linux).
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;

    let target = line.split_whitespace().nth(1).unwrap_or("");
    let Some(query) = target.strip_prefix("/callback?") else {
        (&stream).write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(None);
    };

    let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap_or_default();
    let result = match (params.get("code"), params.get("error")) {
        (Some(code), _) => Callback::Code(code.clone()),
        (None, Some(_)) => Callback::Denied,
        (None, None) => return Ok(None),
    };
    let body = match result {
        Callback::Code(_) => "Signed in. You can close this tab.",
        Callback::Denied => "Sign-in cancelled. You can close this tab.",
    };
    write!(
        &stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;

    Ok(Some(result))
}
