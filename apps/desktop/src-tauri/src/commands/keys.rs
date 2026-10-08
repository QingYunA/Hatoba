//! Key vault (spec §8.3): import, generate, rename, delete, copy public key, deploy (ssh-copy-id).

use hatoba_core::model::{HostAuth, Item, KeyAlgorithm as CoreAlg, SshKey};
use hatoba_ssh::{GenerateKind, KeyAlgorithm as SshAlg, ParsedKey};
use tauri::{AppHandle, State};
use zeroize::Zeroizing;

use crate::convert::key_view;
use crate::dto::{GenerateAlgorithm, KeyGenerateInput, KeyImportInput, KeyView};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, blocking, now_ms};
use crate::{ssh, sync};

/// Private key files are small; anything larger is not a key (and must not be slurped into memory).
const MAX_KEY_FILE: u64 = 256 * 1024;

pub fn core_algorithm(alg: SshAlg) -> CoreAlg {
    match alg {
        SshAlg::Ed25519 => CoreAlg::Ed25519,
        SshAlg::Ecdsa => CoreAlg::Ecdsa,
        SshAlg::Rsa => CoreAlg::Rsa,
    }
}

fn store_parsed(
    state: &AppState,
    name: &str,
    parsed: ParsedKey,
    passphrase: Option<String>,
) -> AppResult<KeyView> {
    state.with_unlocked(|v| {
        if let Some((_, existing)) = v
            .keys()
            .into_iter()
            .find(|(_, k)| k.fingerprint == parsed.fingerprint)
        {
            return Err(AppError::invalid(
                "private_key",
                format!("this key is already in the vault as \"{}\"", existing.name),
            ));
        }
        let name = match name.trim() {
            "" if !parsed.comment.is_empty() => parsed.comment.clone(),
            "" => format!("{}-key", format!("{:?}", parsed.algorithm).to_lowercase()),
            n => n.to_owned(),
        };
        let key = SshKey {
            name,
            algorithm: core_algorithm(parsed.algorithm),
            // OpenSSH text as imported (possibly passphrase-encrypted); the vault encrypts it again.
            private_key: parsed.openssh_private.clone(),
            passphrase: if parsed.encrypted {
                passphrase.map(Zeroizing::new)
            } else {
                None
            },
            public_key: parsed.public_openssh.clone(),
            fingerprint: parsed.fingerprint.clone(),
            comment: parsed.comment.clone(),
            created_at: now_ms(),
            updated_at: 0,
        };
        let id = v.put(None, Item::Key(key))?;
        let saved = v
            .get(&id)
            .and_then(Item::as_key)
            .cloned()
            .ok_or_else(|| AppError::not_found("key"))?;
        Ok(key_view(&id, &saved, v))
    })
}

#[tauri::command]
#[specta::specta]
pub fn keys_list(state: State<'_, AppState>) -> AppResult<Vec<KeyView>> {
    state.with_unlocked(|v| {
        let mut keys: Vec<KeyView> = v.keys().iter().map(|(id, k)| key_view(id, k, v)).collect();
        keys.sort_by_cached_key(|k| k.name.to_lowercase());
        Ok(keys)
    })
}

/// KEY-01 / WIN-09: OpenSSH, PEM and PuTTY .ppk, from a file or pasted text, with clear errors.
#[tauri::command]
#[specta::specta]
pub async fn key_import(
    app: AppHandle,
    state: State<'_, AppState>,
    input: KeyImportInput,
) -> AppResult<KeyView> {
    let text = match (&input.path, &input.private_key) {
        (Some(path), _) => {
            let meta = std::fs::metadata(path)?;
            if meta.len() > MAX_KEY_FILE {
                return Err(AppError::key(
                    crate::error::KeyParseErrorKind::UnsupportedFormat,
                    "file is too large to be a private key",
                ));
            }
            Zeroizing::new(std::fs::read_to_string(path).map_err(|_| {
                AppError::key(
                    crate::error::KeyParseErrorKind::UnsupportedFormat,
                    "file is not a text key file",
                )
            })?)
        }
        (None, Some(text)) => Zeroizing::new(text.clone()),
        (None, None) => {
            return Err(AppError::invalid(
                "private_key",
                "paste a key or choose a file",
            ));
        }
    };
    let passphrase = input.passphrase.filter(|p| !p.is_empty());
    let pw = passphrase.clone().map(Zeroizing::new);
    let parsed = blocking(move || {
        Ok(hatoba_ssh::parse_private_key(
            &text,
            pw.as_deref().map(String::as_str),
        )?)
    })
    .await?;
    let view = store_parsed(&state, &input.name, parsed, passphrase)?;
    sync::local_change(&app);
    Ok(view)
}

/// KEY-02: Ed25519 by default, RSA 4096 optional.
#[tauri::command]
#[specta::specta]
pub async fn key_generate(
    app: AppHandle,
    state: State<'_, AppState>,
    input: KeyGenerateInput,
) -> AppResult<KeyView> {
    if input.name.trim().is_empty() {
        return Err(AppError::invalid("name", "name is required"));
    }
    let kind = match input.algorithm {
        GenerateAlgorithm::Ed25519 => GenerateKind::Ed25519,
        GenerateAlgorithm::Rsa => GenerateKind::Rsa4096,
    };
    let comment = input.comment.trim().to_owned();
    let passphrase = input.passphrase.filter(|p| !p.is_empty());
    let pw = passphrase.clone().map(Zeroizing::new);
    let parsed = blocking(move || {
        Ok(hatoba_ssh::generate_key(
            kind,
            &comment,
            pw.as_deref().map(String::as_str),
        )?)
    })
    .await?;
    let view = store_parsed(&state, &input.name, parsed, passphrase)?;
    sync::local_change(&app);
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub fn key_rename(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<KeyView> {
    let view = state.with_unlocked(|v| {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::invalid("name", "name is required"));
        }
        let mut key = v
            .get(&id)
            .and_then(Item::as_key)
            .cloned()
            .ok_or_else(|| AppError::not_found("key"))?;
        key.name = name.to_owned();
        v.put(Some(&id), Item::Key(key))?;
        let saved = v
            .get(&id)
            .and_then(Item::as_key)
            .cloned()
            .ok_or_else(|| AppError::not_found("key"))?;
        Ok(key_view(&id, &saved, v))
    })?;
    sync::local_change(&app);
    Ok(view)
}

/// KEY-05: the UI lists the hosts using the key before calling this; they fall back to "ask".
#[tauri::command]
#[specta::specta]
pub fn key_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_unlocked(|v| {
        v.get(&id)
            .and_then(Item::as_key)
            .ok_or_else(|| AppError::not_found("key"))?;
        for (host_id, mut host) in v.hosts() {
            if matches!(&host.auth, HostAuth::Key { key_id } if *key_id == id) {
                host.auth = HostAuth::Ask;
                v.put(Some(&host_id), Item::Host(host))?;
            }
        }
        v.delete(&id)?;
        Ok(())
    })?;
    sync::local_change(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn key_public(state: State<'_, AppState>, id: String) -> AppResult<String> {
    state.with_unlocked(|v| {
        v.get(&id)
            .and_then(Item::as_key)
            .map(|k| k.public_key.clone())
            .ok_or_else(|| AppError::not_found("key"))
    })
}

/// KEY-06: appends the public key to `~/.ssh/authorized_keys` on a host (like `ssh-copy-id`).
/// The key travels on stdin, so nothing from it is interpreted by the remote shell.
#[tauri::command]
#[specta::specta]
pub async fn key_deploy(
    app: AppHandle,
    state: State<'_, AppState>,
    key_id: String,
    host_id: String,
) -> AppResult<()> {
    let public = key_public(state.clone(), key_id)?;
    let line = public.lines().next().unwrap_or_default().trim().to_owned();
    let session = ssh::connect_for_task(&app, &state, &host_id, None).await?;
    const SCRIPT: &str = "umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && read -r key && \
        (grep -qxF \"$key\" ~/.ssh/authorized_keys || printf '%s\\n' \"$key\" >> ~/.ssh/authorized_keys)";
    let result = session
        .exec(
            &format!("sh -c '{}'", SCRIPT.replace('\'', "'\\''")),
            Some(format!("{line}\n").into_bytes()),
        )
        .await;
    session.disconnect().await;
    match result? {
        (0, _) => Ok(()),
        (status, _) => Err(AppError::ssh(
            crate::error::SshErrorKind::Other,
            format!("remote command exited with status {status}"),
        )),
    }
}
