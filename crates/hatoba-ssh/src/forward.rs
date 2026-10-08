//! Local port forwarding, `ssh -L` (FWD-01).

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::error::{SshError, SshErrorKind};
use crate::session::SshSession;

/// A `-L bind_address:bind_port:dest_host:dest_port` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalForward {
    /// Local interface to listen on; empty means `127.0.0.1`.
    pub bind_address: String,
    /// Local port; `0` picks a free port (see [`ForwardHandle::local_port`]).
    pub bind_port: u16,
    /// Destination as seen from the SSH server.
    pub dest_host: String,
    /// Destination port.
    pub dest_port: u16,
}

struct ForwardInner {
    local_addr: SocketAddr,
    cancel: CancellationToken,
}

/// A running local forward. Cheap to clone; dropping a handle does not stop it.
#[derive(Clone)]
pub struct ForwardHandle {
    inner: Arc<ForwardInner>,
}

impl ForwardHandle {
    /// The port the forward is actually listening on.
    pub fn local_port(&self) -> u16 {
        self.inner.local_addr.port()
    }

    /// The local address the forward is listening on.
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    /// `true` until [`stop`](Self::stop) is called or the SSH connection ends.
    pub fn is_running(&self) -> bool {
        !self.inner.cancel.is_cancelled()
    }

    /// Stops listening and closes every forwarded connection.
    pub fn stop(&self) {
        self.inner.cancel.cancel();
    }
}

pub(crate) async fn start(
    session: SshSession,
    fwd: LocalForward,
) -> Result<ForwardHandle, SshError> {
    let bind_host = if fwd.bind_address.trim().is_empty() {
        "127.0.0.1"
    } else {
        fwd.bind_address.trim()
    };
    let listener = TcpListener::bind((bind_host, fwd.bind_port))
        .await
        .map_err(|e| {
            SshError::io_context(&format!("listen on {bind_host}:{}", fwd.bind_port), &e)
        })?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| SshError::io_context("local address", &e))?;

    let cancel = CancellationToken::new();
    tokio::spawn(accept_loop(listener, session, fwd, cancel.clone()));
    Ok(ForwardHandle {
        inner: Arc::new(ForwardInner { local_addr, cancel }),
    })
}

async fn accept_loop(
    listener: TcpListener,
    session: SshSession,
    fwd: LocalForward,
    cancel: CancellationToken,
) {
    let mut closed = session.shared().subscribe();
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            () = async { let _ = closed.wait_for(|c| *c).await; } => {
                cancel.cancel();
                break;
            }
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let session = session.clone();
                    let dest_host = fwd.dest_host.clone();
                    let dest_port = fwd.dest_port;
                    let cancel = cancel.child_token();
                    tokio::spawn(async move {
                        if let Err(e) = pipe(session, stream, peer, &dest_host, dest_port, cancel).await {
                            tracing::debug!("forwarded connection ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    tracing::debug!("accept failed: {e}");
                    // Avoid a hot loop on persistent errors such as EMFILE.
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            },
        }
    }
}

async fn pipe(
    session: SshSession,
    mut tcp: TcpStream,
    peer: SocketAddr,
    dest_host: &str,
    dest_port: u16,
    cancel: CancellationToken,
) -> Result<(), SshError> {
    let _ = tcp.set_nodelay(true);
    let channel = session
        .handle()
        .channel_open_direct_tcpip(
            dest_host,
            u32::from(dest_port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await
        .map_err(SshError::from)?;
    let mut stream = channel.into_stream();
    tokio::select! {
        () = cancel.cancelled() => Ok(()),
        copied = tokio::io::copy_bidirectional(&mut tcp, &mut stream) => copied
            .map(|_| ())
            .map_err(|e| SshError::new(SshErrorKind::Io, e.to_string())),
    }
}
