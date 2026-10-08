//! PuTTY `.ppk` (v2 and v3) support (WIN-09).
//!
//! The actual container parsing, Argon2 / SHA-1 key derivation, AES-256-CBC
//! decryption and MAC verification is done by `ssh-key` (re-exported through
//! `russh::keys`). This module only inspects the plaintext header so callers
//! can tell whether a passphrase is needed, and maps the opaque `ssh-key`
//! errors onto [`KeyError`].

use russh::keys::ssh_key::{self, PrivateKey};

use crate::keys::KeyError;

const HEADER_PREFIX: &str = "PuTTY-User-Key-File-";

/// What can be learned from a `.ppk` file without decrypting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PpkHeader {
    /// Whether the private part is encrypted with a passphrase.
    pub encrypted: bool,
}

/// `true` if `text` looks like a PuTTY private key file.
pub(crate) fn is_ppk(text: &str) -> bool {
    text.trim_start().starts_with(HEADER_PREFIX)
}

/// Reads the plaintext header fields of a `.ppk` file.
pub(crate) fn inspect(text: &str) -> Result<PpkHeader, KeyError> {
    let mut lines = text.trim_start().lines();
    let first = lines.next().ok_or(KeyError::UnsupportedFormat)?;
    let rest = first
        .strip_prefix(HEADER_PREFIX)
        .ok_or(KeyError::UnsupportedFormat)?;
    let (version, _algorithm) = rest
        .split_once(':')
        .ok_or_else(|| KeyError::Invalid("malformed PuTTY key header".into()))?;
    let version: u8 = version
        .trim()
        .parse()
        .map_err(|_| KeyError::Invalid("malformed PuTTY key header".into()))?;
    if !(2..=3).contains(&version) {
        return Err(KeyError::Invalid(format!(
            "unsupported PuTTY key file version {version}"
        )));
    }
    let mut encrypted = false;
    for line in lines {
        if let Some(value) = line.strip_prefix("Encryption:") {
            encrypted = value.trim() != "none";
            break;
        }
    }
    Ok(PpkHeader { encrypted })
}

/// Decodes a `.ppk` file, decrypting it with `passphrase` if required.
pub(crate) fn decode(text: &str, passphrase: Option<&str>) -> Result<PrivateKey, KeyError> {
    let header = inspect(text)?;
    if header.encrypted && passphrase.is_none() {
        return Err(KeyError::PassphraseRequired);
    }
    let passphrase = if header.encrypted {
        passphrase.map(str::to_owned)
    } else {
        None
    };
    PrivateKey::from_ppk(text.trim_start(), passphrase).map_err(|e| map_error(&e, header.encrypted))
}

fn map_error(e: &ssh_key::Error, encrypted: bool) -> KeyError {
    match e {
        ssh_key::Error::Ppk(inner) => {
            let detail = inner.to_string();
            if detail.contains("incorrect MAC") {
                if encrypted {
                    KeyError::WrongPassphrase
                } else {
                    KeyError::Invalid("PuTTY key file failed its integrity check".into())
                }
            } else if detail.contains("private key is encrypted") {
                KeyError::PassphraseRequired
            } else if detail.starts_with("unsupported encryption")
                || detail.starts_with("unsupported KDF")
            {
                KeyError::UnsupportedAlgorithm(detail)
            } else {
                KeyError::Invalid(detail)
            }
        }
        ssh_key::Error::AlgorithmUnsupported { algorithm } => {
            KeyError::UnsupportedAlgorithm(algorithm.to_string())
        }
        ssh_key::Error::AlgorithmUnknown => KeyError::UnsupportedAlgorithm("unknown".into()),
        other => KeyError::Invalid(other.to_string()),
    }
}
