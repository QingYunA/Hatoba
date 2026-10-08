//! [`WorkerBackend`]: the sync backend for a deployed Hatoba Worker (spec §6.2).
//!
//! All requests are JSON over HTTPS with a 15 s timeout. Secrets cross the wire as standard
//! padded base64 of the raw 32-byte keys (`auth_key`, `recovery_auth`); the Worker stores only
//! their SHA-256. Session tokens travel in `Authorization: Bearer …` and are never logged.

use std::sync::RwLock;

use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Method, StatusCode, Url};
use serde::Deserialize;
use serde_json::json;
use zeroize::Zeroizing;

use crate::crypto::b64_encode;
use crate::error::{Error, Result};
use crate::sync::backend::{
    Change, DeviceLogin, KdfInfo, MAX_CHANGES_PER_PUSH, PullPage, PushResult, Recovered, RemoteDevice, RemoteItem,
    ServerInfo, Session, SyncBackend, VaultInit, VaultMeta, VaultMetaUpdate,
};
use crate::sync::http::{
    build_client, error_code, is_loopback_host, map_transport, read_body, retry_after,
};

/// Normalises a Worker URL typed by the user.
///
/// Trims whitespace, adds `https://` when no scheme is given, strips a trailing slash, and
/// rejects anything but `https` — except `http` to `localhost` / `127.0.0.1` / `[::1]` for local
/// development. Credentials, queries and fragments are refused.
///
/// # Errors
/// [`Error::InvalidUrl`].
pub fn normalize_worker_url(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidUrl("empty URL".into()));
    }
    let with_scheme = if trimmed.contains("://") { trimmed.to_owned() } else { format!("https://{trimmed}") };
    let url = Url::parse(&with_scheme).map_err(|_| Error::InvalidUrl("not a valid URL".into()))?;
    let host = url.host_str().ok_or_else(|| Error::InvalidUrl("missing host".into()))?;
    match url.scheme() {
        "https" => {}
        "http" if is_loopback_host(host) => {}
        "http" => return Err(Error::InvalidUrl("plain http is only allowed for localhost".into())),
        _ => return Err(Error::InvalidUrl("unsupported scheme".into())),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::InvalidUrl("credentials in the URL are not allowed".into()));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::InvalidUrl("query strings and fragments are not allowed".into()));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// How a 401 is interpreted for a given endpoint.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    /// `GET /v1/health`, `GET /v1/prelogin`: no auth.
    Public,
    /// `GET /v1/prelogin`: a bare 404 means "not initialised".
    Prelogin,
    /// `POST /v1/setup`: 401 = bad setup token.
    Setup,
    /// `POST /v1/login`: 401 = wrong password.
    Login,
    /// `POST /v1/recover`: 401 = wrong recovery code.
    Recover,
    /// Everything that needs a session: 401 = session expired.
    Session,
}

/// Talks to a Hatoba Worker over HTTPS.
pub struct WorkerBackend {
    base: String,
    client: reqwest::Client,
    session: RwLock<Option<Session>>,
}

impl WorkerBackend {
    /// Creates a backend for the Worker at `url` (see [`normalize_worker_url`]).
    ///
    /// # Errors
    /// [`Error::InvalidUrl`].
    pub fn new(url: &str) -> Result<Self> {
        let base = normalize_worker_url(url)?;
        let loopback = Url::parse(&base).ok().and_then(|u| u.host_str().map(is_loopback_host)).unwrap_or(false);
        Ok(Self { base, client: build_client(loopback)?, session: RwLock::new(None) })
    }

    /// The normalised base URL (what `SyncConfig::Worker` stores).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn current_session(&self) -> Option<Session> {
        self.session.read().ok().and_then(|g| g.clone())
    }

    fn store_session(&self, session: Option<Session>) {
        if let Ok(mut guard) = self.session.write() {
            *guard = session;
        }
    }

    /// Sends a request and returns the success body, or the mapped error.
    async fn call(
        &self,
        method: Method,
        path: &str,
        endpoint: Endpoint,
        bearer: Option<&str>,
        body: Option<Zeroizing<Vec<u8>>>,
    ) -> Result<Vec<u8>> {
        let mut req = self.client.request(method, format!("{}{path}", self.base));
        if let Some(token) = bearer {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| Error::Protocol("token is not a valid header value".into()))?;
            value.set_sensitive(true);
            req = req.header(AUTHORIZATION, value);
        } else if endpoint == Endpoint::Session {
            // No session installed: the server would answer 401 anyway.
            return Err(Error::Unauthorized);
        }
        if let Some(body) = body {
            req = req.header(CONTENT_TYPE, "application/json").body(body.to_vec());
        }
        let resp = req.send().await.map_err(|e| map_transport(&e))?;
        let (status, headers, bytes) = read_body(resp).await?;
        if status.is_success() {
            return Ok(bytes);
        }
        Err(map_failure(status, retry_after(&headers), &bytes, endpoint))
    }

    /// Authenticated call with the installed session.
    async fn call_session(&self, method: Method, path: &str, body: Option<Zeroizing<Vec<u8>>>) -> Result<Vec<u8>> {
        let token = self.current_session().map(|s| s.token);
        self.call(method, path, Endpoint::Session, token.as_deref().map(String::as_str), body).await
    }
}

fn map_failure(status: StatusCode, retry_after_secs: Option<u64>, body: &[u8], endpoint: Endpoint) -> Error {
    let code = error_code(body);
    match status {
        StatusCode::UNAUTHORIZED => match endpoint {
            Endpoint::Setup => Error::InvalidSetupToken,
            Endpoint::Login => Error::WrongPassword,
            Endpoint::Recover => Error::WrongRecoveryCode,
            _ => Error::Unauthorized,
        },
        StatusCode::FORBIDDEN => Error::Unauthorized,
        StatusCode::NOT_FOUND => {
            if code.as_deref() == Some("not_initialized") || (endpoint == Endpoint::Prelogin && code.is_none()) {
                Error::RemoteNotInitialized
            } else {
                Error::Server(format!("http 404{}", suffix(code.as_deref())))
            }
        }
        StatusCode::CONFLICT => Error::RemoteInitialized,
        StatusCode::TOO_MANY_REQUESTS => Error::RateLimited { retry_after_secs },
        other => Error::Server(format!("http {}{}", other.as_u16(), suffix(code.as_deref()))),
    }
}

fn suffix(code: Option<&str>) -> String {
    code.map(|c| format!(" {c}")).unwrap_or_default()
}

fn to_json_body(value: &serde_json::Value) -> Result<Zeroizing<Vec<u8>>> {
    Ok(Zeroizing::new(serde_json::to_vec(value)?))
}

fn parse<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| Error::Protocol("malformed response".into()))
}

// ---- wire types -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct HealthWire {
    #[serde(default)]
    service: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    api: u32,
    initialized: bool,
}

#[derive(Deserialize)]
struct PreloginWire {
    kdf_salt: String,
    kdf_params: String,
}

#[derive(Deserialize)]
struct SessionWire {
    session_token: Zeroizing<String>,
    expires_at: i64,
}

#[derive(Deserialize)]
struct RecoverWire {
    recovery_vault_key: String,
    kdf_salt: String,
    kdf_params: String,
    session_token: Zeroizing<String>,
    expires_at: i64,
}

#[derive(Deserialize)]
struct VaultWire {
    schema_version: u32,
    kdf_salt: String,
    kdf_params: String,
    protected_vault_key: String,
    recovery_vault_key: String,
    #[serde(default)]
    seq: u64,
}

#[derive(Deserialize)]
struct ItemWire {
    id: String,
    envelope: Option<String>,
    revision: u64,
    seq: u64,
    deleted: bool,
    #[serde(default)]
    updated_at: i64,
}

#[derive(Deserialize)]
struct PullWire {
    items: Vec<ItemWire>,
    next_since: u64,
    has_more: bool,
}

#[derive(Deserialize)]
struct ServerWire {
    revision: u64,
    seq: u64,
    deleted: bool,
    envelope: Option<String>,
    #[serde(default)]
    updated_at: i64,
}

#[derive(Deserialize)]
struct ResultWire {
    id: String,
    status: String,
    revision: Option<u64>,
    seq: Option<u64>,
    server: Option<ServerWire>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct PushWire {
    results: Vec<ResultWire>,
}

#[derive(Deserialize)]
struct DeviceWire {
    device_id: String,
    device_name: String,
    #[serde(default)]
    created_at: i64,
    #[serde(default)]
    last_seen: i64,
    #[serde(default)]
    expires_at: i64,
    #[serde(default)]
    current: bool,
}

#[derive(Deserialize)]
struct DevicesWire {
    devices: Vec<DeviceWire>,
}

impl From<ItemWire> for RemoteItem {
    fn from(w: ItemWire) -> Self {
        Self {
            id: w.id,
            envelope: w.envelope.filter(|_| !w.deleted),
            revision: w.revision,
            seq: w.seq,
            deleted: w.deleted,
            updated_at: w.updated_at,
        }
    }
}

fn change_json(c: &Change) -> serde_json::Value {
    json!({
        "id": c.id,
        "base_revision": c.base_revision,
        "deleted": c.deleted,
        "envelope": c.envelope,
        "updated_at": c.updated_at,
    })
}

fn push_result(w: ResultWire) -> Result<PushResult> {
    match w.status.as_str() {
        "ok" => Ok(PushResult::Ok {
            id: w.id,
            revision: w.revision.ok_or_else(|| Error::Protocol("ok result without revision".into()))?,
            seq: w.seq.unwrap_or(0),
        }),
        "conflict" => {
            let s = w.server.ok_or_else(|| Error::Protocol("conflict result without server row".into()))?;
            Ok(PushResult::Conflict {
                server: RemoteItem {
                    id: w.id.clone(),
                    envelope: s.envelope.filter(|_| !s.deleted),
                    revision: s.revision,
                    seq: s.seq,
                    deleted: s.deleted,
                    updated_at: s.updated_at,
                },
                id: w.id,
            })
        }
        "error" => Ok(PushResult::Error { id: w.id, error: w.error.unwrap_or_else(|| "error".into()) }),
        other => Err(Error::Protocol(format!("unknown push status {}", other.chars().take(16).collect::<String>()))),
    }
}

#[async_trait]
impl SyncBackend for WorkerBackend {
    async fn health(&self) -> Result<ServerInfo> {
        let bytes = self.call(Method::GET, "/v1/health", Endpoint::Public, None, None).await?;
        let w: HealthWire = parse(&bytes)?;
        Ok(ServerInfo { service: w.service, version: w.version, api: w.api, initialized: w.initialized })
    }

    async fn prelogin(&self) -> Result<KdfInfo> {
        let bytes = self.call(Method::GET, "/v1/prelogin", Endpoint::Prelogin, None, None).await?;
        let w: PreloginWire = parse(&bytes)?;
        Ok(KdfInfo { kdf_salt: w.kdf_salt, kdf_params: w.kdf_params })
    }

    async fn setup(&self, init: VaultInit) -> Result<()> {
        let token = init
            .setup_token
            .as_ref()
            .ok_or(Error::InvalidSetupToken)?
            .clone();
        let body = to_json_body(&json!({
            "schema_version": init.schema_version,
            "kdf_salt": init.kdf_salt,
            "kdf_params": init.kdf_params,
            "auth_key": b64_encode(&*init.auth_key),
            "protected_vault_key": init.protected_vault_key,
            "recovery_vault_key": init.recovery_vault_key,
            "recovery_auth": b64_encode(&*init.recovery_auth),
        }))?;
        self.call(Method::POST, "/v1/setup", Endpoint::Setup, Some(token.as_str()), Some(body)).await?;
        Ok(())
    }

    async fn login(&self, auth_key: &[u8; 32], device: &DeviceLogin) -> Result<Session> {
        let body = to_json_body(&json!({
            "auth_key": b64_encode(auth_key),
            "device_id": device.device_id,
            "device_name": device.sealed_name,
        }))?;
        let bytes = self.call(Method::POST, "/v1/login", Endpoint::Login, None, Some(body)).await?;
        let w: SessionWire = parse(&bytes)?;
        let session = Session { token: w.session_token, expires_at: w.expires_at };
        self.store_session(Some(session.clone()));
        Ok(session)
    }

    async fn recover(&self, recovery_auth: &[u8; 32], device: &DeviceLogin) -> Result<Recovered> {
        let body = to_json_body(&json!({
            "recovery_auth": b64_encode(recovery_auth),
            "device_id": device.device_id,
            "device_name": device.sealed_name,
        }))?;
        let bytes = self.call(Method::POST, "/v1/recover", Endpoint::Recover, None, Some(body)).await?;
        let w: RecoverWire = parse(&bytes)?;
        let session = Session { token: w.session_token, expires_at: w.expires_at };
        self.store_session(Some(session.clone()));
        Ok(Recovered {
            recovery_vault_key: w.recovery_vault_key,
            kdf: KdfInfo { kdf_salt: w.kdf_salt, kdf_params: w.kdf_params },
            session,
        })
    }

    async fn fetch_vault(&self) -> Result<VaultMeta> {
        let bytes = self.call_session(Method::GET, "/v1/vault", None).await?;
        let w: VaultWire = parse(&bytes)?;
        Ok(VaultMeta {
            schema_version: w.schema_version,
            kdf_salt: w.kdf_salt,
            kdf_params: w.kdf_params,
            protected_vault_key: w.protected_vault_key,
            recovery_vault_key: w.recovery_vault_key,
            seq: w.seq,
        })
    }

    async fn pull(&self, since_seq: u64, limit: u32) -> Result<PullPage> {
        let path = format!("/v1/items?since={since_seq}&limit={limit}");
        let bytes = self.call_session(Method::GET, &path, None).await?;
        let w: PullWire = parse(&bytes)?;
        Ok(PullPage { items: w.items.into_iter().map(RemoteItem::from).collect(), next_since: w.next_since, has_more: w.has_more })
    }

    async fn push(&self, changes: Vec<Change>) -> Result<Vec<PushResult>> {
        let mut out = Vec::with_capacity(changes.len());
        for chunk in changes.chunks(MAX_CHANGES_PER_PUSH) {
            let body = to_json_body(&json!({ "changes": chunk.iter().map(change_json).collect::<Vec<_>>() }))?;
            let bytes = self.call_session(Method::POST, "/v1/items", Some(body)).await?;
            let w: PushWire = parse(&bytes)?;
            for r in w.results {
                out.push(push_result(r)?);
            }
        }
        Ok(out)
    }

    async fn update_vault_meta(&self, update: VaultMetaUpdate) -> Result<()> {
        let mut body = json!({
            "kdf_salt": update.kdf_salt,
            "kdf_params": update.kdf_params,
            "auth_key": b64_encode(&*update.auth_key),
            "protected_vault_key": update.protected_vault_key,
        });
        if let Some(recovery) = &update.recovery {
            // The server requires the pair together or not at all.
            body["recovery_vault_key"] = json!(recovery.recovery_vault_key);
            body["recovery_auth"] = json!(b64_encode(&*recovery.recovery_auth));
        }
        self.call_session(Method::PUT, "/v1/vault/password", Some(to_json_body(&body)?)).await?;
        Ok(())
    }

    async fn devices(&self) -> Result<Vec<RemoteDevice>> {
        let bytes = self.call_session(Method::GET, "/v1/devices", None).await?;
        let w: DevicesWire = parse(&bytes)?;
        Ok(w
            .devices
            .into_iter()
            .map(|d| RemoteDevice {
                device_id: d.device_id,
                device_name: d.device_name,
                created_at: d.created_at,
                last_seen: d.last_seen,
                expires_at: d.expires_at,
                current: d.current,
            })
            .collect())
    }

    async fn revoke_device(&self, device_id: &str) -> Result<()> {
        // Device ids are UUIDs; refuse anything that could alter the path.
        if device_id.is_empty() || !device_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err(Error::InvalidUrl("invalid device id".into()));
        }
        self.call_session(Method::DELETE, &format!("/v1/devices/{device_id}"), None).await?;
        Ok(())
    }

    fn set_session(&self, session: Option<Session>) {
        self.store_session(session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalisation() {
        let ok = |s: &str| normalize_worker_url(s).unwrap();
        assert_eq!(ok("sync.example.workers.dev"), "https://sync.example.workers.dev");
        assert_eq!(ok("  https://sync.example.workers.dev/  "), "https://sync.example.workers.dev");
        assert_eq!(ok("https://sync.example.workers.dev///"), "https://sync.example.workers.dev");
        assert_eq!(ok("https://example.com/hatoba/"), "https://example.com/hatoba");
        assert_eq!(ok("http://localhost:8787/"), "http://localhost:8787");
        assert_eq!(ok("http://127.0.0.1:8787"), "http://127.0.0.1:8787");
        assert_eq!(ok("localhost:8787"), "https://localhost:8787");
        assert_eq!(ok("HTTPS://Example.COM"), "https://example.com");
    }

    #[test]
    fn url_rejections() {
        for bad in [
            "",
            "   ",
            "http://example.com",
            "http://127.0.0.1.evil.com",
            "ftp://example.com",
            "https://user:pw@example.com",
            "https://example.com/?token=1",
            "https://example.com/#frag",
            "https://",
            "not a url at all",
        ] {
            assert!(matches!(normalize_worker_url(bad), Err(Error::InvalidUrl(_))), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn failure_mapping() {
        let code = br#"{"error":"not_initialized"}"#;
        let m = |status: u16, ep: Endpoint, body: &[u8]| {
            map_failure(StatusCode::from_u16(status).unwrap(), Some(7), body, ep)
        };
        assert!(matches!(m(401, Endpoint::Session, b""), Error::Unauthorized));
        assert!(matches!(m(401, Endpoint::Setup, b""), Error::InvalidSetupToken));
        assert!(matches!(m(401, Endpoint::Login, b""), Error::WrongPassword));
        assert!(matches!(m(401, Endpoint::Recover, b""), Error::WrongRecoveryCode));
        assert!(matches!(m(403, Endpoint::Session, br#"{"error":"insufficient_scope"}"#), Error::Unauthorized));
        assert!(matches!(m(404, Endpoint::Login, code), Error::RemoteNotInitialized));
        assert!(matches!(m(404, Endpoint::Prelogin, b""), Error::RemoteNotInitialized));
        assert!(matches!(m(404, Endpoint::Session, b"<html>"), Error::Server(_)));
        assert!(matches!(m(409, Endpoint::Setup, b""), Error::RemoteInitialized));
        assert!(matches!(m(429, Endpoint::Login, b""), Error::RateLimited { retry_after_secs: Some(7) }));
        let Error::Server(msg) = m(503, Endpoint::Public, br#"{"error":"database_unavailable"}"#) else { panic!() };
        assert_eq!(msg, "http 503 database_unavailable");
    }

    #[test]
    fn debug_of_secrets_is_redacted() {
        let init = VaultInit {
            schema_version: 1,
            kdf_salt: String::new(),
            kdf_params: String::new(),
            auth_key: Zeroizing::new([7u8; 32]),
            protected_vault_key: String::new(),
            recovery_vault_key: String::new(),
            recovery_auth: Zeroizing::new([8u8; 32]),
            setup_token: Some(Zeroizing::new("tok".into())),
        };
        assert!(!format!("{init:?}").contains('7'));
        let s = Session { token: Zeroizing::new("secret-token".into()), expires_at: 1 };
        assert!(!format!("{s:?}").contains("secret-token"));
    }
}
