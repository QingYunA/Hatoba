//! Plaintext item structures (spec §5.1).
//!
//! These are what gets JSON-serialised, sealed in an [`Envelope`](crate::crypto::Envelope) and
//! synced. The `type` discriminator lives *inside* the encrypted JSON only; no store ever sees it
//! in the clear.
//!
//! Every struct tolerates unknown fields and missing fields (`#[serde(default)]`) so devices on
//! different app versions can read each other's data. Secret-bearing types implement `Debug`
//! by hand so they can never leak into logs, and wipe themselves when dropped.

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, Result};

/// Fixed id of the single [`Settings`] item.
pub const SETTINGS_ID: &str = "settings";

/// A fresh UUIDv7 item id (time-ordered, so the item map iterates in creation order).
#[must_use]
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// How a host authenticates.
///
/// The password lives in a [`Zeroizing`] string, so it is wiped whenever the value is dropped.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostAuth {
    /// A saved password (secret).
    Password {
        /// The password. Never sent to the front-end.
        password: Zeroizing<String>,
    },
    /// A key from the key store.
    Key {
        /// Id of the [`SshKey`] item.
        key_id: String,
    },
    /// The system SSH agent (P1).
    Agent,
    /// Ask on every connection; nothing is stored.
    #[default]
    Ask,
}

impl HostAuth {
    /// Password authentication with the given password.
    #[must_use]
    pub fn password(password: impl Into<String>) -> Self {
        Self::Password { password: Zeroizing::new(password.into()) }
    }
}

impl fmt::Debug for HostAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password { .. } => f.write_str("Password { password: <redacted> }"),
            Self::Key { key_id } => f.debug_struct("Key").field("key_id", key_id).finish(),
            Self::Agent => f.write_str("Agent"),
            Self::Ask => f.write_str("Ask"),
        }
    }
}

fn default_ssh_port() -> u16 {
    22
}

/// An SSH host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct Host {
    /// Display name, e.g. `prod-api-tokyo`.
    pub name: String,
    /// Domain name or IP address.
    pub address: String,
    /// TCP port, default 22.
    pub port: u16,
    /// Login user.
    pub username: String,
    /// Authentication method.
    pub auth: HostAuth,
    /// Containing [`Group`], if any.
    pub group_id: Option<String>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Favourite flag.
    pub favorite: bool,
    /// ProxyJump host (P1).
    pub jump_host_id: Option<String>,
    /// Free-form note.
    pub note: String,
    /// Last modification, Unix ms. Drives last-writer-wins conflict resolution.
    pub updated_at: i64,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            name: String::new(),
            address: String::new(),
            port: default_ssh_port(),
            username: String::new(),
            auth: HostAuth::default(),
            group_id: None,
            tags: Vec::new(),
            favorite: false,
            jump_host_id: None,
            note: String::new(),
            updated_at: 0,
        }
    }
}

/// A host group (one level of nesting in the MVP).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct Group {
    /// Display name.
    pub name: String,
    /// Parent group.
    pub parent_id: Option<String>,
    /// Sort position.
    pub sort: i64,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

/// Key algorithm of an [`SshKey`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAlgorithm {
    /// Ed25519.
    #[default]
    Ed25519,
    /// ECDSA.
    Ecdsa,
    /// RSA.
    Rsa,
}

/// A private key in the key store. The private key and passphrase are wiped on drop.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct SshKey {
    /// Display name.
    pub name: String,
    /// Key algorithm.
    #[zeroize(skip)]
    pub algorithm: KeyAlgorithm,
    /// Private key in OpenSSH format (secret).
    pub private_key: Zeroizing<String>,
    /// Passphrase protecting the private key, if any (secret).
    pub passphrase: Option<Zeroizing<String>>,
    /// Public key in OpenSSH format.
    pub public_key: String,
    /// `SHA256:...` fingerprint.
    pub fingerprint: String,
    /// Key comment.
    pub comment: String,
    /// Creation time, Unix ms.
    pub created_at: i64,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

impl Default for SshKey {
    fn default() -> Self {
        Self {
            name: String::new(),
            algorithm: KeyAlgorithm::default(),
            private_key: Zeroizing::new(String::new()),
            passphrase: None,
            public_key: String::new(),
            fingerprint: String::new(),
            comment: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

impl fmt::Debug for SshKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshKey")
            .field("name", &self.name)
            .field("algorithm", &self.algorithm)
            .field("private_key", &"<redacted>")
            .field("passphrase", &self.passphrase.as_ref().map(|_| "<redacted>"))
            .field("fingerprint", &self.fingerprint)
            .field("updated_at", &self.updated_at)
            .finish_non_exhaustive()
    }
}

/// A trusted server host key (TOFU).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct KnownHost {
    /// Host name or address the key was seen on.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Key type, e.g. `ssh-ed25519`.
    pub key_type: String,
    /// Public key.
    pub public_key: String,
    /// `SHA256:...` fingerprint.
    pub fingerprint: String,
    /// First time the key was accepted, Unix ms.
    pub first_seen_at: i64,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

/// Direction of a [`PortForward`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardKind {
    /// `-L`.
    #[default]
    Local,
    /// `-R`.
    Remote,
    /// `-D` (SOCKS).
    Dynamic,
}

/// A saved port forward (P1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct PortForward {
    /// The [`Host`] this forward belongs to.
    pub host_id: String,
    /// Direction.
    #[zeroize(skip)]
    pub kind: ForwardKind,
    /// Address to bind.
    pub bind_address: String,
    /// Port to bind.
    pub bind_port: u16,
    /// Destination host (`None` for dynamic).
    pub dest_host: Option<String>,
    /// Destination port (`None` for dynamic).
    pub dest_port: Option<u16>,
    /// Start with the connection.
    pub auto_start: bool,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

/// A saved command snippet (P2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct Snippet {
    /// Display name.
    pub name: String,
    /// The command text.
    pub command: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

/// Terminal colour theme preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Light.
    Light,
    /// Dark.
    Dark,
    /// Follow the OS. Also what an unknown future value decodes to (`serde(other)` must be last).
    #[default]
    #[serde(other)]
    System,
}

/// Terminal cursor shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    /// Vertical bar.
    Bar,
    /// Underline.
    Underline,
    /// Filled block. Also what an unknown future value decodes to.
    #[default]
    #[serde(other)]
    Block,
}

/// Terminal appearance settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct TerminalSettings {
    /// Font family.
    pub font_family: String,
    /// Font size in points.
    pub font_size: u16,
    /// Colour theme.
    #[zeroize(skip)]
    pub theme: ThemeMode,
    /// Cursor shape.
    #[zeroize(skip)]
    pub cursor_style: CursorStyle,
    /// Scrollback lines.
    pub scrollback: u32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_family: "Cascadia Mono".to_owned(),
            font_size: 13,
            theme: ThemeMode::Dark,
            cursor_style: CursorStyle::Block,
            scrollback: 10_000,
        }
    }
}

/// The user's synced settings. There is exactly one, with the fixed id [`SETTINGS_ID`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct Settings {
    /// Terminal appearance.
    pub terminal: TerminalSettings,
    /// Idle minutes before auto-lock (0 disables).
    pub auto_lock_minutes: u32,
    /// Disconnect SSH sessions when the vault locks.
    pub lock_disconnects_sessions: bool,
    /// Last modification, Unix ms.
    pub updated_at: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal: TerminalSettings::default(),
            auto_lock_minutes: 15,
            lock_disconnects_sessions: false,
            updated_at: 0,
        }
    }
}

/// Any syncable item. Serialises with `"type": "host" | "group" | "key" | "known_host" |
/// "forward" | "snippet" | "settings"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    /// An SSH host.
    Host(Host),
    /// A host group.
    Group(Group),
    /// A private key.
    Key(SshKey),
    /// A trusted host key.
    KnownHost(KnownHost),
    /// A saved port forward.
    Forward(PortForward),
    /// A command snippet.
    Snippet(Snippet),
    /// The settings singleton.
    Settings(Settings),
}

impl Item {
    /// The `type` string as it appears in the plaintext JSON.
    #[must_use]
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Host(_) => "host",
            Self::Group(_) => "group",
            Self::Key(_) => "key",
            Self::KnownHost(_) => "known_host",
            Self::Forward(_) => "forward",
            Self::Snippet(_) => "snippet",
            Self::Settings(_) => "settings",
        }
    }

    /// Last modification time (Unix ms).
    #[must_use]
    pub fn updated_at(&self) -> i64 {
        match self {
            Self::Host(i) => i.updated_at,
            Self::Group(i) => i.updated_at,
            Self::Key(i) => i.updated_at,
            Self::KnownHost(i) => i.updated_at,
            Self::Forward(i) => i.updated_at,
            Self::Snippet(i) => i.updated_at,
            Self::Settings(i) => i.updated_at,
        }
    }

    /// Sets the modification time (Unix ms).
    pub fn set_updated_at(&mut self, at: i64) {
        match self {
            Self::Host(i) => i.updated_at = at,
            Self::Group(i) => i.updated_at = at,
            Self::Key(i) => i.updated_at = at,
            Self::KnownHost(i) => i.updated_at = at,
            Self::Forward(i) => i.updated_at = at,
            Self::Snippet(i) => i.updated_at = at,
            Self::Settings(i) => i.updated_at = at,
        }
    }

    /// A human-readable label for lists and conflict screens. Never a secret.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Host(i) => i.name.clone(),
            Self::Group(i) => i.name.clone(),
            Self::Key(i) => i.name.clone(),
            Self::KnownHost(i) => format!("{}:{}", i.host, i.port),
            Self::Forward(i) => format!("{}:{}", i.bind_address, i.bind_port),
            Self::Snippet(i) => i.name.clone(),
            Self::Settings(_) => "settings".to_owned(),
        }
    }

    /// Whether this is an SSH key (keys are never silently dropped by conflict resolution).
    #[must_use]
    pub fn is_key(&self) -> bool {
        matches!(self, Self::Key(_))
    }

    /// The host, if this is one.
    #[must_use]
    pub fn as_host(&self) -> Option<&Host> {
        if let Self::Host(h) = self { Some(h) } else { None }
    }

    /// The group, if this is one.
    #[must_use]
    pub fn as_group(&self) -> Option<&Group> {
        if let Self::Group(g) = self { Some(g) } else { None }
    }

    /// The key, if this is one.
    #[must_use]
    pub fn as_key(&self) -> Option<&SshKey> {
        if let Self::Key(k) = self { Some(k) } else { None }
    }

    /// The known host, if this is one.
    #[must_use]
    pub fn as_known_host(&self) -> Option<&KnownHost> {
        if let Self::KnownHost(k) = self { Some(k) } else { None }
    }

    /// The port forward, if this is one.
    #[must_use]
    pub fn as_forward(&self) -> Option<&PortForward> {
        if let Self::Forward(f) = self { Some(f) } else { None }
    }

    /// The settings, if this is the settings item.
    #[must_use]
    pub fn as_settings(&self) -> Option<&Settings> {
        if let Self::Settings(s) = self { Some(s) } else { None }
    }

    /// Returns a copy whose name carries `suffix` (used for conflict copies). Types without a
    /// name field are returned unchanged.
    #[must_use]
    pub fn with_name_suffix(&self, suffix: &str) -> Self {
        let mut copy = self.clone();
        match &mut copy {
            Self::Host(i) => i.name.push_str(suffix),
            Self::Group(i) => i.name.push_str(suffix),
            Self::Key(i) => i.name.push_str(suffix),
            Self::Snippet(i) => i.name.push_str(suffix),
            Self::KnownHost(_) | Self::Forward(_) | Self::Settings(_) => {}
        }
        copy
    }

    /// Serialises to the plaintext JSON that gets sealed. The buffer is wiped on drop.
    ///
    /// # Errors
    /// [`Error::Json`] if serialisation fails (cannot happen for these types in practice).
    pub fn to_plaintext(&self) -> Result<Zeroizing<Vec<u8>>> {
        // Pre-size so typical items (even a 4096-bit RSA key) never reallocate, which would
        // leave an un-wiped copy of the plaintext in freed memory.
        let mut buf = Zeroizing::new(Vec::with_capacity(16 * 1024));
        serde_json::to_writer(&mut *buf, self)?;
        Ok(buf)
    }

    /// Parses plaintext JSON.
    ///
    /// # Errors
    /// [`Error::Format`] for invalid JSON or an unknown `type` (e.g. written by a newer app).
    pub fn from_plaintext(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|_| Error::Format("unreadable item".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_host() -> Host {
        Host {
            name: "prod-api-tokyo".into(),
            address: "10.0.0.7".into(),
            username: "deploy".into(),
            auth: HostAuth::password("hunter2"),
            tags: vec!["production".into(), "tokyo".into()],
            favorite: true,
            updated_at: 1_700_000_000_000,
            ..Host::default()
        }
    }

    #[test]
    fn host_json_matches_the_spec_shape() {
        let value = serde_json::to_value(Item::Host(sample_host())).unwrap();
        assert_eq!(
            value,
            json!({
                "type": "host",
                "name": "prod-api-tokyo",
                "address": "10.0.0.7",
                "port": 22,
                "username": "deploy",
                "auth": { "kind": "password", "password": "hunter2" },
                "group_id": null,
                "tags": ["production", "tokyo"],
                "favorite": true,
                "jump_host_id": null,
                "note": "",
                "updated_at": 1_700_000_000_000_i64
            })
        );
    }

    #[test]
    fn every_type_tag_round_trips() {
        let items = [
            (Item::Host(Host::default()), "host"),
            (Item::Group(Group::default()), "group"),
            (Item::Key(SshKey::default()), "key"),
            (Item::KnownHost(KnownHost::default()), "known_host"),
            (Item::Forward(PortForward::default()), "forward"),
            (Item::Snippet(Snippet::default()), "snippet"),
            (Item::Settings(Settings::default()), "settings"),
        ];
        for (item, tag) in items {
            assert_eq!(item.kind_str(), tag);
            let value = serde_json::to_value(&item).unwrap();
            assert_eq!(value["type"], tag);
            let back = Item::from_plaintext(&item.to_plaintext().unwrap()).unwrap();
            assert_eq!(back, item);
        }
    }

    #[test]
    fn auth_variants_use_the_kind_tag() {
        let cases = [
            (HostAuth::password("p"), json!({"kind":"password","password":"p"})),
            (HostAuth::Key { key_id: "k".into() }, json!({"kind":"key","key_id":"k"})),
            (HostAuth::Agent, json!({"kind":"agent"})),
            (HostAuth::Ask, json!({"kind":"ask"})),
        ];
        for (auth, expected) in cases {
            assert_eq!(serde_json::to_value(&auth).unwrap(), expected);
            assert_eq!(serde_json::from_value::<HostAuth>(expected).unwrap(), auth);
        }
    }

    #[test]
    fn settings_defaults_match_the_spec() {
        let s = Settings::default();
        assert_eq!(s.terminal.font_family, "Cascadia Mono");
        assert_eq!(s.terminal.font_size, 13);
        assert_eq!(s.terminal.theme, ThemeMode::Dark);
        assert_eq!(s.terminal.cursor_style, CursorStyle::Block);
        assert_eq!(s.terminal.scrollback, 10_000);
        assert_eq!(s.auto_lock_minutes, 15);
        assert!(!s.lock_disconnects_sessions);
        let value = serde_json::to_value(Item::Settings(s)).unwrap();
        assert_eq!(value["terminal"]["theme"], "dark");
        assert_eq!(value["terminal"]["cursor_style"], "block");
    }

    #[test]
    fn tolerant_of_unknown_and_missing_fields() {
        let item: Item = serde_json::from_value(json!({
            "type": "host",
            "name": "x",
            "address": "example.org",
            "from_the_future": { "nested": [1, 2, 3] }
        }))
        .unwrap();
        let host = item.as_host().unwrap();
        assert_eq!(host.port, 22);
        assert_eq!(host.auth, HostAuth::Ask);
        assert!(host.tags.is_empty());
        assert_eq!(host.group_id, None);

        // Unknown enum values degrade gracefully instead of failing the whole item.
        let item: Item =
            serde_json::from_value(json!({"type":"settings","terminal":{"theme":"solarized","cursor_style":"beam"}}))
                .unwrap();
        let s = item.as_settings().unwrap();
        assert_eq!(s.terminal.theme, ThemeMode::System);
        assert_eq!(s.terminal.cursor_style, CursorStyle::Block);
        assert_eq!(s.auto_lock_minutes, 15);
    }

    #[test]
    fn unknown_type_is_a_format_error() {
        let err = Item::from_plaintext(br#"{"type":"teleporter","name":"x"}"#);
        assert!(matches!(err, Err(Error::Format(_))));
        assert!(matches!(Item::from_plaintext(b"{}"), Err(Error::Format(_))));
    }

    #[test]
    fn updated_at_and_names() {
        let mut item = Item::Host(sample_host());
        assert_eq!(item.updated_at(), 1_700_000_000_000);
        item.set_updated_at(5);
        assert_eq!(item.updated_at(), 5);
        assert_eq!(item.display_name(), "prod-api-tokyo");
        let key = Item::Key(SshKey { name: "laptop".into(), ..SshKey::default() });
        assert_eq!(key.with_name_suffix(" (copy)").display_name(), "laptop (copy)");
        assert!(key.is_key());
        let kh = Item::KnownHost(KnownHost { host: "h".into(), port: 2222, ..KnownHost::default() });
        assert_eq!(kh.display_name(), "h:2222");
    }

    #[test]
    fn ids_are_uuid_v7_and_time_ordered() {
        let a = new_id();
        let b = new_id();
        let parsed = uuid::Uuid::parse_str(&a).unwrap();
        assert_eq!(parsed.get_version_num(), 7);
        assert_ne!(a, b);
    }

    #[test]
    fn debug_never_prints_secrets() {
        let host = Item::Host(sample_host());
        assert!(!format!("{host:?}").contains("hunter2"));
        let key = Item::Key(SshKey {
            private_key: Zeroizing::new("-----BEGIN OPENSSH PRIVATE KEY-----SECRETBODY".into()),
            passphrase: Some(Zeroizing::new("pass-phrase-secret".into())),
            ..SshKey::default()
        });
        let dbg = format!("{key:?}");
        assert!(!dbg.contains("SECRETBODY"));
        assert!(!dbg.contains("pass-phrase-secret"));
    }

    #[test]
    fn zeroize_wipes_secret_fields() {
        let mut item = Item::Key(SshKey {
            private_key: Zeroizing::new("PRIVATE".into()),
            passphrase: Some(Zeroizing::new("PASS".into())),
            name: "n".into(),
            ..SshKey::default()
        });
        item.zeroize();
        let key = item.as_key().unwrap();
        assert!(key.private_key.is_empty());
        assert!(key.passphrase.is_none());
        assert!(key.name.is_empty());
    }
}
