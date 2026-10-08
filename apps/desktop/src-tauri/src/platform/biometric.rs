//! Windows Hello unlock (SEC-07, P1).
//!
//! Not a mere consent prompt: a Hello key credential (TPM-backed RSA key, `KeyCredentialManager`)
//! signs a fixed challenge. PKCS#1 v1.5 signatures are deterministic, so the signature is a stable
//! secret that only this user can produce after Windows Hello verification. A wrapping key derived
//! from it (HKDF-SHA256) encrypts the vault key, and that envelope is kept in the Windows
//! Credential Manager. This follows the approach of the KeePass Windows Hello plugin.

use hatoba_core::crypto::{Key32, hkdf_sha256, open_json, seal};
use hatoba_core::platform::{SecretStore, secret_keys};
use tauri::AppHandle;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult, ErrorCode};

const AAD: &[u8] = b"hatoba/hello-vault-key/v1";
const HKDF_INFO: &str = "hatoba/hello-wrap/v1";

pub fn enrolled(secrets: &dyn SecretStore) -> bool {
    available()
        && secrets
            .get(secret_keys::BIOMETRIC_VAULT_KEY)
            .ok()
            .flatten()
            .is_some()
}

pub fn remove(secrets: &dyn SecretStore) -> AppResult<()> {
    secrets.delete(secret_keys::BIOMETRIC_VAULT_KEY)?;
    imp::delete_credential();
    Ok(())
}

pub fn available() -> bool {
    imp::available()
}

pub async fn enroll(
    app: &AppHandle,
    secrets: &dyn SecretStore,
    vault_key: &Key32,
) -> AppResult<()> {
    let signature = imp::sign(app, true).await?;
    let wrap = hkdf_sha256(&signature, HKDF_INFO);
    let envelope = seal(&wrap, AAD, vault_key.as_slice())?;
    secrets.set(secret_keys::BIOMETRIC_VAULT_KEY, &envelope.to_json())?;
    Ok(())
}

pub async fn unlock(app: &AppHandle, secrets: &dyn SecretStore) -> AppResult<Key32> {
    let sealed = secrets
        .get(secret_keys::BIOMETRIC_VAULT_KEY)?
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                "Windows Hello is not set up for this vault",
            )
        })?;
    let signature = imp::sign(app, false).await?;
    let wrap = hkdf_sha256(&signature, HKDF_INFO);
    let plain = open_json(&wrap, AAD, &sealed).map_err(|_| {
        AppError::new(
            ErrorCode::WrongPassword,
            "Windows Hello credential changed; unlock with the master password and enable it again",
        )
    })?;
    let bytes: [u8; 32] = plain
        .as_slice()
        .try_into()
        .map_err(|_| AppError::internal("bad biometric envelope"))?;
    Ok(Zeroizing::new(bytes))
}

#[cfg(target_os = "windows")]
mod imp {
    use tauri::AppHandle;
    use windows::Security::Credentials::{
        KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
    };
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::core::{Array, HSTRING};
    use zeroize::Zeroizing;

    use crate::error::{AppError, AppResult, ErrorCode};

    const CREDENTIAL: &str = "app.hatoba.desktop.vault";
    /// Fixed challenge; secrecy comes from the TPM-held private key, not from this value.
    const CHALLENGE: &[u8] = b"hatoba/windows-hello/challenge/v1";

    pub fn available() -> bool {
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|op| op.join())
            .unwrap_or(false)
    }

    pub fn delete_credential() {
        if let Ok(op) = KeyCredentialManager::DeleteAsync(&HSTRING::from(CREDENTIAL)) {
            let _ = op.join();
        }
    }

    fn status_error(status: KeyCredentialStatus) -> AppError {
        if status == KeyCredentialStatus::UserCanceled {
            AppError::new(ErrorCode::Cancelled, "Windows Hello was cancelled")
        } else {
            AppError::new(
                ErrorCode::Internal,
                format!("Windows Hello failed ({status:?})"),
            )
        }
    }

    pub async fn sign(app: &AppHandle, create: bool) -> AppResult<Zeroizing<Vec<u8>>> {
        // Hello's dialog must belong to our foreground window; make sure it is in front.
        if let Some(w) = tauri::Manager::get_webview_window(app, crate::platform::window::MAIN) {
            let _ = w.set_focus();
        }
        tauri::async_runtime::spawn_blocking(move || {
            let name = HSTRING::from(CREDENTIAL);
            let win = |e: windows::core::Error| AppError::internal(format!("Windows Hello: {e}"));
            let result = if create {
                KeyCredentialManager::RequestCreateAsync(
                    &name,
                    KeyCredentialCreationOption::ReplaceExisting,
                )
            } else {
                KeyCredentialManager::OpenAsync(&name)
            }
            .and_then(|op| op.join())
            .map_err(win)?;
            let status = result.Status().map_err(win)?;
            if status != KeyCredentialStatus::Success {
                return Err(status_error(status));
            }
            let credential = result.Credential().map_err(win)?;
            let challenge = CryptographicBuffer::CreateFromByteArray(CHALLENGE).map_err(win)?;
            let signed = credential
                .RequestSignAsync(&challenge)
                .and_then(|op| op.join())
                .map_err(win)?;
            let status = signed.Status().map_err(win)?;
            if status != KeyCredentialStatus::Success {
                return Err(status_error(status));
            }
            let buffer = signed.Result().map_err(win)?;
            let mut bytes = Array::<u8>::new();
            CryptographicBuffer::CopyToByteArray(&buffer, &mut bytes).map_err(win)?;
            Ok(Zeroizing::new(bytes.to_vec()))
        })
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use tauri::AppHandle;
    use zeroize::Zeroizing;

    use crate::error::{AppError, AppResult, ErrorCode};

    pub fn available() -> bool {
        false
    }

    pub fn delete_credential() {}

    pub async fn sign(_app: &AppHandle, _create: bool) -> AppResult<Zeroizing<Vec<u8>>> {
        Err(AppError::new(
            ErrorCode::Internal,
            "biometric unlock is not available on this platform yet",
        ))
    }
}
