//! ssh-agent access (SSH-09).
//!
//! Unix (Linux/macOS): the Unix socket named by `SSH_AUTH_SOCK`.
//! Windows: the OpenSSH agent named pipe `\\.\pipe\openssh-ssh-agent`
//! (or the pipe named by `SSH_AUTH_SOCK` if it points at one).

use russh::keys::agent::client::{AgentClient, AgentStream};

use crate::error::SshError;

/// A connected agent, with the stream type erased so the platform paths agree.
pub(crate) type Agent = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

/// Name of the Win32-OpenSSH agent pipe.
#[cfg(windows)]
const WINDOWS_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

#[cfg(unix)]
pub(crate) async fn connect() -> Result<Agent, SshError> {
    let client = AgentClient::connect_env()
        .await
        .map_err(|e| SshError::other(format!("ssh-agent is not available: {e}")))?;
    Ok(client.dynamic())
}

#[cfg(windows)]
pub(crate) async fn connect() -> Result<Agent, SshError> {
    let pipe = std::env::var("SSH_AUTH_SOCK")
        .ok()
        .filter(|p| p.starts_with(r"\\.\pipe\"))
        .unwrap_or_else(|| WINDOWS_AGENT_PIPE.to_owned());
    let client = AgentClient::connect_named_pipe(&pipe).await.map_err(|e| {
        SshError::other(format!(
            "ssh-agent is not available (is the \"OpenSSH Authentication Agent\" service running?): {e}"
        ))
    })?;
    Ok(client.dynamic())
}

#[cfg(not(any(unix, windows)))]
pub(crate) async fn connect() -> Result<Agent, SshError> {
    Err(SshError::other(
        "ssh-agent is not supported on this platform",
    ))
}
