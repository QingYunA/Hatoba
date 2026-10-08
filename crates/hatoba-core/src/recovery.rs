//! The recovery code (spec §4.1, VAULT-02).
//!
//! The code is 128 random bits. For display it is extended with a 4-byte checksum
//! (`SHA-256(secret)[0..4]`) so typos are caught locally, then written as 32 Crockford Base32
//! characters in eight groups of four: `K7QF-2M9X-…`.
//!
//! Two independent keys are derived from the secret with HKDF-SHA256:
//! * `recovery_key` (`info = "hatoba/recovery/v1"`) wraps the vault key;
//! * `recovery_auth` (`info = "hatoba/recovery-auth/v1"`) proves possession to the Worker.

use std::fmt;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::crypto::{Key32, hkdf_sha256, random_bytes};
use crate::error::{Error, Result};

/// Length of the random secret in bytes (128 bits).
pub const SECRET_LEN: usize = 16;
const CHECKSUM_LEN: usize = 4;
const CODE_BYTES: usize = SECRET_LEN + CHECKSUM_LEN;
/// Number of Base32 characters in a code: 20 bytes = 160 bits = 32 characters.
pub const CODE_CHARS: usize = 32;
/// Characters per displayed group.
pub const GROUP_LEN: usize = 4;

/// Crockford's alphabet: no `I`, `L`, `O`, `U`.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn checksum(secret: &[u8; SECRET_LEN]) -> [u8; CHECKSUM_LEN] {
    let digest = Sha256::digest(secret);
    let mut out = [0u8; CHECKSUM_LEN];
    out.copy_from_slice(&digest[..CHECKSUM_LEN]);
    out
}

/// Encodes 20 bytes as exactly 32 Crockford Base32 characters (no separators).
fn encode_base32(bytes: &[u8; CODE_BYTES]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(CODE_CHARS));
    let mut acc: u16 = 0;
    let mut bits = 0u32;
    for &byte in bytes {
        acc = (acc << 8) | u16::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[usize::from((acc >> bits) & 0x1f)]));
        }
        acc &= (1 << bits) - 1;
    }
    debug_assert_eq!(bits, 0, "160 bits divide evenly into 5-bit symbols");
    out
}

/// Maps one input character to its 5-bit value, applying Crockford's aliases.
fn symbol_value(c: char) -> Option<u8> {
    let c = match c.to_ascii_uppercase() {
        'I' | 'L' => '1',
        'O' => '0',
        other => other,
    };
    let idx = ALPHABET.iter().position(|&a| char::from(a) == c)?;
    u8::try_from(idx).ok()
}

/// A recovery code. Holds the 128-bit secret and wipes it on drop.
pub struct RecoveryCode {
    secret: Zeroizing<[u8; SECRET_LEN]>,
}

impl RecoveryCode {
    /// Generates a fresh code from the OS CSPRNG.
    ///
    /// # Errors
    /// [`Error::Random`] if the OS generator fails.
    pub fn generate() -> Result<Self> {
        let mut secret = Zeroizing::new([0u8; SECRET_LEN]);
        getrandom::fill(&mut *secret).map_err(|_| Error::Random)?;
        Ok(Self { secret })
    }

    /// Wraps an existing secret (used by tests and by `recover`).
    #[must_use]
    pub fn from_secret(secret: [u8; SECRET_LEN]) -> Self {
        Self { secret: Zeroizing::new(secret) }
    }

    /// Parses user input. Case-insensitive; spaces and dashes are ignored; `I`/`L` read as `1`
    /// and `O` as `0`; `U` and any other character are rejected; the checksum must match.
    ///
    /// # Errors
    /// [`Error::WrongRecoveryCode`] for any malformed input or checksum mismatch.
    pub fn parse(input: &str) -> Result<Self> {
        let mut bytes = Zeroizing::new([0u8; CODE_BYTES]);
        let mut acc: u16 = 0;
        let mut bits = 0u32;
        let mut chars = 0usize;
        let mut idx = 0usize;
        for c in input.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            let value = symbol_value(c).ok_or(Error::WrongRecoveryCode)?;
            chars += 1;
            if chars > CODE_CHARS {
                return Err(Error::WrongRecoveryCode);
            }
            acc = (acc << 5) | u16::from(value);
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                // The high bits above `bits` hold one complete byte.
                bytes[idx] = u8::try_from((acc >> bits) & 0xff).map_err(|_| Error::WrongRecoveryCode)?;
                idx += 1;
                acc &= (1 << bits) - 1;
            }
        }
        if chars != CODE_CHARS || idx != CODE_BYTES {
            return Err(Error::WrongRecoveryCode);
        }
        let mut secret = Zeroizing::new([0u8; SECRET_LEN]);
        secret.copy_from_slice(&bytes[..SECRET_LEN]);
        let mut given = [0u8; CHECKSUM_LEN];
        given.copy_from_slice(&bytes[SECRET_LEN..]);
        if !bool::from(checksum(&secret).ct_eq(&given)) {
            return Err(Error::WrongRecoveryCode);
        }
        Ok(Self { secret })
    }

    /// The raw 128-bit secret.
    #[must_use]
    pub fn secret(&self) -> &[u8; SECRET_LEN] {
        &self.secret
    }

    /// The code as shown to the user: eight dash-separated groups of four characters.
    #[must_use]
    pub fn grouped(&self) -> Zeroizing<String> {
        let mut raw = [0u8; CODE_BYTES];
        raw[..SECRET_LEN].copy_from_slice(&*self.secret);
        raw[SECRET_LEN..].copy_from_slice(&checksum(&self.secret));
        let flat = encode_base32(&raw);
        let mut out = Zeroizing::new(String::with_capacity(CODE_CHARS + CODE_CHARS / GROUP_LEN - 1));
        for (i, c) in flat.chars().enumerate() {
            if i > 0 && i % GROUP_LEN == 0 {
                out.push('-');
            }
            out.push(c);
        }
        out
    }

    /// The final group, which the UI asks the user to re-type to confirm they saved the code.
    #[must_use]
    pub fn last_group(&self) -> String {
        let grouped = self.grouped();
        grouped.rsplit('-').next().unwrap_or_default().to_owned()
    }

    /// `HKDF(secret, "hatoba/recovery/v1")`: wraps the vault key.
    #[must_use]
    pub fn recovery_key(&self) -> Key32 {
        hkdf_sha256(&*self.secret, "hatoba/recovery/v1")
    }

    /// `HKDF(secret, "hatoba/recovery-auth/v1")`: authenticates recovery to the Worker.
    #[must_use]
    pub fn recovery_auth(&self) -> Key32 {
        hkdf_sha256(&*self.secret, "hatoba/recovery-auth/v1")
    }
}

impl fmt::Display for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.grouped())
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(<redacted>)")
    }
}

impl PartialEq for RecoveryCode {
    fn eq(&self, other: &Self) -> bool {
        bool::from(self.secret.ct_eq(&*other.secret))
    }
}

impl Eq for RecoveryCode {}

/// Fresh random secret bytes (exposed for callers that need a one-off random block).
///
/// # Errors
/// [`Error::Random`] if the OS generator fails.
pub fn random_secret() -> Result<[u8; SECRET_LEN]> {
    random_bytes::<SECRET_LEN>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed() -> RecoveryCode {
        RecoveryCode::from_secret(std::array::from_fn(|i| i as u8))
    }

    #[test]
    fn formats_as_eight_groups_of_four() {
        let code = RecoveryCode::generate().unwrap().to_string();
        assert_eq!(code.len(), 8 * 4 + 7);
        let groups: Vec<&str> = code.split('-').collect();
        assert_eq!(groups.len(), 8);
        assert!(groups.iter().all(|g| g.len() == 4));
        assert!(code.chars().all(|c| c == '-' || ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn known_encoding() {
        // secret 00 01 .. 0F followed by SHA-256(secret)[..4]; expected value computed
        // independently with a few lines of Python.
        let code = fixed();
        assert_eq!(code.grouped().as_str(), "000G-40R4-0M30-E209-185G-R38E-1YZ4-BJS6");
        assert_eq!(code.last_group(), "BJS6");
        assert_eq!(RecoveryCode::parse("000G-40R4-0M30-E209-185G-R38E-1YZ4-BJS6").unwrap(), code);
    }

    #[test]
    fn round_trip() {
        for _ in 0..50 {
            let code = RecoveryCode::generate().unwrap();
            let parsed = RecoveryCode::parse(&code.to_string()).unwrap();
            assert_eq!(parsed, code);
            assert_eq!(parsed.secret(), code.secret());
        }
    }

    #[test]
    fn parse_is_forgiving_about_case_spacing_and_separators() {
        let code = fixed();
        let shown = code.to_string();
        assert_eq!(RecoveryCode::parse(&shown.to_lowercase()).unwrap(), code);
        assert_eq!(RecoveryCode::parse(&shown.replace('-', " ")).unwrap(), code);
        assert_eq!(RecoveryCode::parse(&shown.replace('-', "")).unwrap(), code);
        assert_eq!(RecoveryCode::parse(&format!("  {shown}\n")).unwrap(), code);
    }

    #[test]
    fn crockford_aliases() {
        let code = fixed();
        let shown = code.to_string();
        // Replace every '1' by 'l' / 'I' and every '0' by 'o' / 'O': must still parse.
        let aliased = shown.replace('1', "l").replace('0', "O");
        assert_ne!(aliased, shown);
        assert_eq!(RecoveryCode::parse(&aliased).unwrap(), code);
        let aliased = shown.replace('1', "I").replace('0', "o");
        assert_eq!(RecoveryCode::parse(&aliased).unwrap(), code);
    }

    #[test]
    fn rejects_u_and_invalid_characters() {
        let shown = fixed().to_string();
        let mut chars: Vec<char> = shown.chars().collect();
        chars[0] = 'U';
        assert!(matches!(RecoveryCode::parse(&chars.iter().collect::<String>()), Err(Error::WrongRecoveryCode)));
        chars[0] = '!';
        assert!(matches!(RecoveryCode::parse(&chars.iter().collect::<String>()), Err(Error::WrongRecoveryCode)));
    }

    #[test]
    fn rejects_wrong_length() {
        let shown = fixed().to_string();
        assert!(RecoveryCode::parse(&shown[..shown.len() - 1]).is_err());
        assert!(RecoveryCode::parse(&format!("{shown}0")).is_err());
        assert!(RecoveryCode::parse("").is_err());
    }

    #[test]
    fn detects_every_single_character_typo() {
        let code = fixed();
        let shown: Vec<char> = code.to_string().chars().collect();
        let mut checked = 0;
        for (i, &orig) in shown.iter().enumerate() {
            if orig == '-' {
                continue;
            }
            for &replacement in ALPHABET {
                let replacement = char::from(replacement);
                if replacement == orig {
                    continue;
                }
                let mut mutated = shown.clone();
                mutated[i] = replacement;
                let text: String = mutated.into_iter().collect();
                // A single wrong symbol changes at least one bit; with a 32-bit checksum the
                // chance of an accidental pass is 2^-32, so none may parse back to the same code.
                match RecoveryCode::parse(&text) {
                    Err(Error::WrongRecoveryCode) => {}
                    Ok(other) => panic!("typo accepted as a different valid code: {:?}", other == code),
                    Err(e) => panic!("unexpected error {e}"),
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 32 * 31);
    }

    #[test]
    fn last_group_is_the_final_four_characters() {
        let code = RecoveryCode::generate().unwrap();
        let shown = code.to_string();
        assert_eq!(code.last_group(), shown[shown.len() - 4..]);
        assert_eq!(code.last_group().len(), 4);
    }

    #[test]
    fn derived_keys_are_distinct_and_match_vectors() {
        let code = fixed();
        let to_hex = |k: &Key32| crate::crypto::hex_encode(&**k);
        assert_eq!(to_hex(&code.recovery_key()), "0dca153820188c14ba27d5f245b93895304e977e0d2a883c0ccec8c35d16f53d");
        assert_eq!(to_hex(&code.recovery_auth()), "2f339a3148c9d4fbec4874321938875d31fa98c0aed7340e83d757b7dacbcf6c");
    }

    #[test]
    fn debug_does_not_leak() {
        let code = RecoveryCode::generate().unwrap();
        assert!(!format!("{code:?}").contains(&code.last_group()));
    }
}
