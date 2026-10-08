//! Error classification (SSH-05) and small utilities. These tests need no sshd.

mod common;

use std::time::{Duration, Instant};

use common::*;
use hatoba_ssh::{AuthMethod, ConnectConfig, SshError, SshErrorKind, connect, tcp_probe};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

fn cfg(host: &str, port: u16) -> ConnectConfig {
    ConnectConfig::new(
        host,
        port,
        "nobody",
        AuthMethod::Password(Zeroizing::new("pw".into())),
    )
}

async fn fails(cfg: ConnectConfig) -> SshError {
    connect(cfg, Verifier::accepting())
        .await
        .expect_err("connect should fail")
}

#[tokio::test]
async fn connection_refused() {
    let err = fails(cfg("127.0.0.1", free_port())).await;
    assert_eq!(err.kind, SshErrorKind::Refused, "{err}");
    assert!(err.message.contains("127.0.0.1"), "{err}");
}

#[tokio::test]
async fn dns_failure() {
    let started = Instant::now();
    let err = fails(cfg("nonexistent.invalid", 22)).await;
    assert_eq!(err.kind, SshErrorKind::Dns, "{err}");
    assert!(err.message.contains("nonexistent.invalid"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(15));
}

#[tokio::test]
async fn blackholed_address_times_out() {
    // 10.255.255.1 normally swallows SYNs. In a sandbox without a route the kernel may
    // answer "network unreachable" instead, which is also a precise, distinct error.
    let mut c = cfg("10.255.255.1", 22);
    c.connect_timeout = Duration::from_secs(1);
    let started = Instant::now();
    let err = fails(c).await;
    assert!(
        matches!(err.kind, SshErrorKind::Timeout | SshErrorKind::Unreachable),
        "{err}"
    );
    if err.kind == SshErrorKind::Timeout {
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(900) && took < Duration::from_secs(3),
            "{took:?}"
        );
        assert!(err.message.contains("TCP"), "{err}");
    }
}

#[tokio::test]
async fn silent_server_times_out_in_the_handshake() {
    // Accepts the TCP connection but never sends an SSH banner.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = listener.accept().await {
            held.push(s);
        }
    });
    let mut c = cfg("127.0.0.1", port);
    c.connect_timeout = Duration::from_secs(1);
    let started = Instant::now();
    let err = fails(c).await;
    assert_eq!(err.kind, SshErrorKind::Timeout, "{err}");
    assert!(err.message.contains("handshake"), "{err}");
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(900) && took < Duration::from_secs(3),
        "{took:?}"
    );
}

#[tokio::test]
async fn non_ssh_server_is_a_protocol_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = listener.accept().await {
            tokio::spawn(async move {
                let _ = s
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                    .await;
                let mut sink = [0u8; 256];
                let _ = s.read(&mut sink).await;
            });
        }
    });
    let mut c = cfg("127.0.0.1", port);
    c.connect_timeout = Duration::from_secs(3);
    let err = fails(c).await;
    assert!(
        matches!(
            err.kind,
            SshErrorKind::Protocol | SshErrorKind::Disconnected | SshErrorKind::Timeout
        ),
        "{err}"
    );
    eprintln!("non-ssh server -> {err}");
}

#[tokio::test]
async fn server_closing_immediately_is_disconnected() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((s, _)) = listener.accept().await {
            drop(s);
        }
    });
    let err = fails(cfg("127.0.0.1", port)).await;
    assert_eq!(err.kind, SshErrorKind::Disconnected, "{err}");
}

#[tokio::test]
async fn tcp_probe_reports_reachability_only() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { while listener.accept().await.is_ok() {} });
    let ms = tcp_probe("127.0.0.1", port, Duration::from_secs(3)).await;
    assert!(ms.is_some_and(|m| (1..1000).contains(&m)), "{ms:?}");
    assert_eq!(
        tcp_probe("127.0.0.1", free_port(), Duration::from_secs(3)).await,
        None
    );
    assert_eq!(
        tcp_probe("nonexistent.invalid", 22, Duration::from_secs(3)).await,
        None
    );
    let started = Instant::now();
    assert_eq!(
        tcp_probe("10.255.255.1", 22, Duration::from_millis(500)).await,
        None
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn errors_serialize_as_snake_case_kinds() {
    let json = serde_json::to_value(SshError::new(SshErrorKind::HostKeyRejected, "nope")).unwrap();
    assert_eq!(json["kind"], "host_key_rejected");
    assert_eq!(json["message"], "nope");
    for (kind, name) in [
        (SshErrorKind::Dns, "dns"),
        (SshErrorKind::Refused, "refused"),
        (SshErrorKind::Timeout, "timeout"),
        (SshErrorKind::Unreachable, "unreachable"),
        (SshErrorKind::AuthFailed, "auth_failed"),
        (SshErrorKind::HostKeyRejected, "host_key_rejected"),
        (SshErrorKind::KeyParse, "key_parse"),
        (SshErrorKind::Disconnected, "disconnected"),
        (SshErrorKind::Protocol, "protocol"),
        (SshErrorKind::Io, "io"),
        (SshErrorKind::Channel, "channel"),
        (SshErrorKind::Sftp, "sftp"),
        (SshErrorKind::Cancelled, "cancelled"),
        (SshErrorKind::Other, "other"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), name);
        assert_eq!(kind.as_str(), name);
        let back: SshErrorKind = serde_json::from_value(serde_json::json!(name)).unwrap();
        assert_eq!(back, kind);
    }
}

#[test]
fn config_defaults_and_secret_free_debug() {
    let c = ConnectConfig::new(
        "h",
        22,
        "u",
        AuthMethod::Password(Zeroizing::new("hunter2".into())),
    );
    assert_eq!(c.connect_timeout, Duration::from_secs(15));
    assert_eq!(c.keepalive_interval, Duration::from_secs(30));
    assert_eq!(c.keepalive_max, 3);
    assert!(c.jump.is_empty() && c.keyboard_interactive.is_none());
    let debug = format!("{c:?}");
    assert!(!debug.contains("hunter2"), "{debug}");

    let key = AuthMethod::PrivateKey {
        openssh: Zeroizing::new("-----BEGIN OPENSSH PRIVATE KEY-----\nSECRET".into()),
        passphrase: Some(Zeroizing::new("phrase".into())),
    };
    let debug = format!("{key:?}");
    assert!(
        !debug.contains("SECRET") && !debug.contains("phrase"),
        "{debug}"
    );
}
