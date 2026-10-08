//! `qobuzdlx --healthcheck`: asks the running server whether it is alive.
//!
//! The container image has no `curl`, and the orchestrator needs a command that exits 0 or 1.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);

/// The address to ask: the port the server listens on, reached through loopback.
#[must_use]
pub fn target(bind: &str) -> Option<SocketAddr> {
    let bind: SocketAddr = bind.parse().ok()?;
    Some(SocketAddr::from(([127, 0, 0, 1], bind.port())))
}

/// Whether the server at `address` answers `/healthz` with a 200.
#[must_use]
pub fn is_alive(address: SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let request = "GET /healthz HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut answer = String::new();
    let _ = stream.take(512).read_to_string(&mut answer);
    answer.starts_with("HTTP/1.0 200") || answer.starts_with("HTTP/1.1 200")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn serve(status_line: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut request = [0u8; 256];
                let _ = socket.read(&mut request);
                let _ = socket.write_all(format!("{status_line}\r\n\r\n{{}}").as_bytes());
            }
        });
        address
    }

    #[test]
    fn a_server_that_answers_200_is_alive() {
        assert!(is_alive(serve("HTTP/1.1 200 OK")));
    }

    #[test]
    fn a_server_that_answers_something_else_is_not() {
        assert!(!is_alive(serve("HTTP/1.1 503 Service Unavailable")));
    }

    #[test]
    fn nothing_listening_is_not_alive() {
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();

        assert!(!is_alive(address));
    }

    #[test]
    fn the_port_to_ask_is_the_one_the_server_binds() {
        assert_eq!(
            target("0.0.0.0:9000"),
            Some(SocketAddr::from(([127, 0, 0, 1], 9000)))
        );
        assert_eq!(target("not an address"), None);
    }
}
