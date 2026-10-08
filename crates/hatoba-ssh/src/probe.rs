//! Cheap reachability probe for the host status dot (HOST-10).

use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::net::{ceil_ms, resolve};

/// Opens a TCP connection to `host:port` and immediately closes it; no SSH
/// handshake and no authentication. Returns the connect time in milliseconds,
/// or `None` if the host did not accept a connection within `timeout`
/// (DNS failure, refusal, unreachable or timeout all map to `None`).
pub async fn tcp_probe(host: &str, port: u16, timeout: Duration) -> Option<u32> {
    let attempt = async {
        let addrs = resolve(host, port).await.ok()?;
        for addr in addrs {
            let started = Instant::now();
            if TcpStream::connect(addr).await.is_ok() {
                return Some(ceil_ms(started.elapsed()));
            }
        }
        None
    };
    tokio::time::timeout(timeout, attempt).await.ok().flatten()
}
