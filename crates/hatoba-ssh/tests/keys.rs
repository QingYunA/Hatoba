//! Private key parsing against fixtures generated with ssh-keygen, openssl and puttygen
//! (see `tests/fixtures/generate.sh`). All fixture keys are test-only.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use hatoba_ssh::{
    GenerateKind, KeyAlgorithm, KeyError, fingerprint_sha256, generate_key, parse_private_key,
};

const PASS: &str = "hatoba-test";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/keys")
}

fn read(name: &str) -> String {
    fs::read_to_string(fixtures().join(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn expected_fingerprints() -> HashMap<String, String> {
    read("fingerprints.txt")
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(k, v)| (k.to_owned(), v.trim().to_owned()))
        .collect()
}

struct Case {
    file: &'static str,
    /// Name of the OpenSSH key this fixture was derived from (for the expected fingerprint).
    source: &'static str,
    algorithm: KeyAlgorithm,
    bits: u32,
    encrypted_input: bool,
    /// Whether `openssh_private` of the result still needs the passphrase.
    encrypted_output: bool,
    comment: Option<&'static str>,
}

const fn case(
    file: &'static str,
    source: &'static str,
    algorithm: KeyAlgorithm,
    bits: u32,
    encrypted_input: bool,
    encrypted_output: bool,
    comment: Option<&'static str>,
) -> Case {
    Case {
        file,
        source,
        algorithm,
        bits,
        encrypted_input,
        encrypted_output,
        comment,
    }
}

fn cases() -> Vec<Case> {
    use KeyAlgorithm::{Ecdsa, Ed25519, Rsa};
    vec![
        // OpenSSH format
        case(
            "ed25519",
            "ed25519",
            Ed25519,
            256,
            false,
            false,
            Some("test-ed25519"),
        ),
        case(
            "ed25519_enc",
            "ed25519_enc",
            Ed25519,
            256,
            true,
            true,
            Some("test-ed25519-enc"),
        ),
        case("rsa", "rsa", Rsa, 2048, false, false, Some("test-rsa")),
        case(
            "rsa_enc",
            "rsa_enc",
            Rsa,
            2048,
            true,
            true,
            Some("test-rsa-enc"),
        ),
        case(
            "ecdsa256",
            "ecdsa256",
            Ecdsa,
            256,
            false,
            false,
            Some("test-ecdsa256"),
        ),
        case(
            "ecdsa384",
            "ecdsa384",
            Ecdsa,
            384,
            false,
            false,
            Some("test-ecdsa384"),
        ),
        case(
            "ecdsa521",
            "ecdsa521",
            Ecdsa,
            521,
            false,
            false,
            Some("test-ecdsa521"),
        ),
        case(
            "ecdsa256_enc",
            "ecdsa256_enc",
            Ecdsa,
            256,
            true,
            true,
            Some("test-ecdsa256-enc"),
        ),
        // PEM
        case("rsa_pkcs1", "rsa_pkcs1", Rsa, 2048, false, false, None),
        case(
            "rsa_pkcs1_enc",
            "rsa_pkcs1_enc",
            Rsa,
            2048,
            true,
            false,
            None,
        ),
        case(
            "rsa_pkcs1_aes256",
            "rsa_pkcs1",
            Rsa,
            2048,
            true,
            false,
            None,
        ),
        case("ec_sec1", "ec_sec1", Ecdsa, 256, false, false, None),
        case("ec_sec1_enc", "ec_sec1_enc", Ecdsa, 256, true, false, None),
        case(
            "ed25519_pkcs8",
            "ed25519_pkcs8",
            Ed25519,
            256,
            false,
            false,
            None,
        ),
        case("rsa_pkcs8", "rsa_pkcs8", Rsa, 2048, false, false, None),
        case("ec_pkcs8", "ec_pkcs8", Ecdsa, 256, false, false, None),
        case(
            "ed25519_pkcs8_enc",
            "ed25519_pkcs8_enc",
            Ed25519,
            256,
            true,
            false,
            None,
        ),
        case(
            "rsa_pkcs8_enc",
            "rsa_pkcs8_enc",
            Rsa,
            2048,
            true,
            false,
            None,
        ),
        // PuTTY v2 / v3, converted from the OpenSSH keys above
        case(
            "ed25519_v2.ppk",
            "ed25519",
            Ed25519,
            256,
            false,
            false,
            Some("test-ed25519"),
        ),
        case(
            "ed25519_v3.ppk",
            "ed25519",
            Ed25519,
            256,
            false,
            false,
            Some("test-ed25519"),
        ),
        case(
            "ed25519_v2_enc.ppk",
            "ed25519",
            Ed25519,
            256,
            true,
            false,
            Some("test-ed25519"),
        ),
        case(
            "ed25519_v3_enc.ppk",
            "ed25519",
            Ed25519,
            256,
            true,
            false,
            Some("test-ed25519"),
        ),
        case(
            "rsa_v2.ppk",
            "rsa",
            Rsa,
            2048,
            false,
            false,
            Some("test-rsa"),
        ),
        case(
            "rsa_v3.ppk",
            "rsa",
            Rsa,
            2048,
            false,
            false,
            Some("test-rsa"),
        ),
        case(
            "rsa_v2_enc.ppk",
            "rsa",
            Rsa,
            2048,
            true,
            false,
            Some("test-rsa"),
        ),
        case(
            "rsa_v3_enc.ppk",
            "rsa",
            Rsa,
            2048,
            true,
            false,
            Some("test-rsa"),
        ),
        case(
            "ecdsa256_v2.ppk",
            "ecdsa256",
            Ecdsa,
            256,
            false,
            false,
            Some("test-ecdsa256"),
        ),
        case(
            "ecdsa256_v3.ppk",
            "ecdsa256",
            Ecdsa,
            256,
            false,
            false,
            Some("test-ecdsa256"),
        ),
        case(
            "ecdsa384_v2.ppk",
            "ecdsa384",
            Ecdsa,
            384,
            false,
            false,
            Some("test-ecdsa384"),
        ),
        case(
            "ecdsa384_v3.ppk",
            "ecdsa384",
            Ecdsa,
            384,
            false,
            false,
            Some("test-ecdsa384"),
        ),
        case(
            "ecdsa521_v2.ppk",
            "ecdsa521",
            Ecdsa,
            521,
            false,
            false,
            Some("test-ecdsa521"),
        ),
        case(
            "ecdsa521_v3.ppk",
            "ecdsa521",
            Ecdsa,
            521,
            false,
            false,
            Some("test-ecdsa521"),
        ),
        case(
            "ecdsa256_v2_enc.ppk",
            "ecdsa256",
            Ecdsa,
            256,
            true,
            false,
            Some("test-ecdsa256"),
        ),
        case(
            "ecdsa256_v3_enc.ppk",
            "ecdsa256",
            Ecdsa,
            256,
            true,
            false,
            Some("test-ecdsa256"),
        ),
    ]
}

/// The fingerprint of "ecdsa256_enc" etc. is keyed by its own `.pub`.
fn expected_for<'a>(fps: &'a HashMap<String, String>, c: &Case) -> &'a str {
    fps.get(c.source)
        .unwrap_or_else(|| panic!("no expected fingerprint for {}", c.source))
}

#[test]
fn every_format_parses_with_correct_metadata() {
    let fps = expected_fingerprints();
    for c in cases() {
        let text = read(c.file);
        let pass = c.encrypted_input.then_some(PASS);
        let key = parse_private_key(&text, pass).unwrap_or_else(|e| panic!("{}: {e}", c.file));
        assert_eq!(key.algorithm, c.algorithm, "{}: algorithm", c.file);
        assert_eq!(key.bits, c.bits, "{}: bits", c.file);
        assert_eq!(key.encrypted, c.encrypted_output, "{}: encrypted", c.file);
        assert_eq!(
            key.fingerprint,
            expected_for(&fps, &c),
            "{}: fingerprint",
            c.file
        );
        if let Some(comment) = c.comment {
            assert_eq!(key.comment, comment, "{}: comment", c.file);
        }
        assert!(
            key.public_openssh.starts_with(match c.algorithm {
                KeyAlgorithm::Ed25519 => "ssh-ed25519 ",
                KeyAlgorithm::Rsa => "ssh-rsa ",
                KeyAlgorithm::Ecdsa => "ecdsa-sha2-nistp",
            }),
            "{}: public line {}",
            c.file,
            key.public_openssh
        );
        // The public line agrees with the fingerprint.
        assert_eq!(
            fingerprint_sha256(&key.public_openssh),
            key.fingerprint,
            "{}",
            c.file
        );
    }
}

#[test]
fn re_encoded_output_is_valid_openssh() {
    let fps = expected_fingerprints();
    for c in cases() {
        let text = read(c.file);
        let pass = c.encrypted_input.then_some(PASS);
        let first = parse_private_key(&text, pass).unwrap();
        assert!(
            first
                .openssh_private
                .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"),
            "{}",
            c.file
        );
        // Parsing the stored form again must work, needing the passphrase only if still encrypted.
        let again_pass = first.encrypted.then_some(PASS);
        let second = parse_private_key(&first.openssh_private, again_pass)
            .unwrap_or_else(|e| panic!("{} (re-parse): {e}", c.file));
        assert_eq!(second.fingerprint, expected_for(&fps, &c), "{}", c.file);
        if first.encrypted {
            assert_eq!(
                parse_private_key(&first.openssh_private, None).unwrap_err(),
                KeyError::PassphraseRequired,
                "{}",
                c.file
            );
        }
    }
}

#[test]
fn openssh_input_is_kept_verbatim() {
    for name in ["ed25519", "ed25519_enc", "rsa"] {
        let text = read(name);
        let pass = (name == "ed25519_enc").then_some(PASS);
        let key = parse_private_key(&text, pass).unwrap();
        assert_eq!(key.openssh_private.as_str(), text, "{name}");
    }
}

#[test]
fn missing_passphrase_is_reported() {
    for c in cases().into_iter().filter(|c| c.encrypted_input) {
        let text = read(c.file);
        assert_eq!(
            parse_private_key(&text, None).unwrap_err(),
            KeyError::PassphraseRequired,
            "{}",
            c.file
        );
        // An empty passphrase counts as none.
        assert_eq!(
            parse_private_key(&text, Some("")).unwrap_err(),
            KeyError::PassphraseRequired,
            "{} (empty)",
            c.file
        );
    }
}

#[test]
fn wrong_passphrase_is_reported() {
    for c in cases().into_iter().filter(|c| c.encrypted_input) {
        let text = read(c.file);
        assert_eq!(
            parse_private_key(&text, Some("definitely-wrong")).unwrap_err(),
            KeyError::WrongPassphrase,
            "{}",
            c.file
        );
    }
}

#[test]
fn unprotected_keys_ignore_a_passphrase() {
    for c in cases().into_iter().filter(|c| !c.encrypted_input) {
        let text = read(c.file);
        parse_private_key(&text, Some("not needed")).unwrap_or_else(|e| panic!("{}: {e}", c.file));
    }
}

#[test]
fn crlf_line_endings_are_accepted() {
    for name in ["ed25519", "rsa_pkcs1", "ed25519_v3.ppk"] {
        let text = read(name).replace('\n', "\r\n");
        let key = parse_private_key(&text, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!key.openssh_private.contains('\r'), "{name}");
    }
}

#[test]
fn surrounding_whitespace_is_accepted() {
    let text = format!("\n\n  {}\n\n", read("ed25519").trim());
    parse_private_key(&text, None).unwrap();
}

#[test]
fn unsupported_and_broken_input() {
    assert_eq!(
        parse_private_key("hello world", None).unwrap_err(),
        KeyError::UnsupportedFormat
    );
    assert_eq!(
        parse_private_key("", None).unwrap_err(),
        KeyError::UnsupportedFormat
    );
    assert_eq!(
        parse_private_key(&read("ed25519.pub"), None).unwrap_err(),
        KeyError::UnsupportedFormat
    );
    assert_eq!(
        parse_private_key(
            "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n",
            None
        )
        .unwrap_err(),
        KeyError::UnsupportedFormat
    );
    assert!(matches!(
        parse_private_key(
            "-----BEGIN DSA PRIVATE KEY-----\nAAAA\n-----END DSA PRIVATE KEY-----\n",
            None
        ),
        Err(KeyError::UnsupportedAlgorithm(_))
    ));

    // Truncated OpenSSH, PEM and PPK bodies are "invalid", not a panic.
    let openssh = read("ed25519");
    let cut = &openssh[..openssh.len() / 2];
    assert!(matches!(
        parse_private_key(cut, None),
        Err(KeyError::Invalid(_))
    ));
    let pem = read("rsa_pkcs1");
    let cut = &pem[..pem.len() / 2];
    assert!(parse_private_key(cut, None).is_err());
    let ppk = read("ed25519_v3.ppk");
    let cut = &ppk[..ppk.len() / 2];
    assert!(matches!(
        parse_private_key(cut, None),
        Err(KeyError::Invalid(_))
    ));
}

#[test]
fn tampered_ppk_fails_integrity_check() {
    let ppk = read("ed25519_v3.ppk").replace("Comment: test-ed25519", "Comment: tampered");
    assert!(matches!(
        parse_private_key(&ppk, None),
        Err(KeyError::Invalid(msg)) if msg.contains("integrity")
    ));
}

#[test]
fn parsed_key_debug_hides_the_private_key() {
    let key = parse_private_key(&read("ed25519"), None).unwrap();
    let debug = format!("{key:?}");
    assert!(!debug.contains("BEGIN OPENSSH"));
    assert!(debug.contains("redacted"));
}

#[test]
fn fingerprint_accepts_line_and_blob() {
    let fps = expected_fingerprints();
    let line = read("ed25519.pub");
    let blob = line.split_whitespace().nth(1).unwrap();
    assert_eq!(fingerprint_sha256(&line), fps["ed25519"]);
    assert_eq!(fingerprint_sha256(line.trim()), fps["ed25519"]);
    assert_eq!(fingerprint_sha256(blob), fps["ed25519"]);
    let rsa = read("rsa.pub");
    assert_eq!(fingerprint_sha256(&rsa), fps["rsa"]);
    assert_eq!(fingerprint_sha256("not a key"), "");
    assert_eq!(fingerprint_sha256(""), "");
}

#[test]
fn generated_ed25519_round_trips() {
    let key = generate_key(GenerateKind::Ed25519, "me@laptop", None).unwrap();
    assert_eq!(key.algorithm, KeyAlgorithm::Ed25519);
    assert_eq!(key.bits, 256);
    assert!(!key.encrypted);
    assert_eq!(key.comment, "me@laptop");
    assert!(key.public_openssh.starts_with("ssh-ed25519 "));
    assert!(key.public_openssh.ends_with(" me@laptop"));
    assert!(key.fingerprint.starts_with("SHA256:"));
    assert!(!key.fingerprint.ends_with('='));
    let parsed = parse_private_key(&key.openssh_private, None).unwrap();
    assert_eq!(parsed.fingerprint, key.fingerprint);
    assert_eq!(parsed.comment, "me@laptop");
}

#[test]
fn generated_key_with_passphrase_is_encrypted() {
    let key = generate_key(GenerateKind::Ed25519, "enc", Some("s3cret pass")).unwrap();
    assert!(key.encrypted);
    assert_eq!(key.comment, "enc");
    assert!(key.public_openssh.ends_with(" enc"));
    assert_eq!(
        parse_private_key(&key.openssh_private, None).unwrap_err(),
        KeyError::PassphraseRequired
    );
    assert_eq!(
        parse_private_key(&key.openssh_private, Some("nope")).unwrap_err(),
        KeyError::WrongPassphrase
    );
    let parsed = parse_private_key(&key.openssh_private, Some("s3cret pass")).unwrap();
    assert_eq!(parsed.fingerprint, key.fingerprint);
    assert_eq!(parsed.comment, "enc");
    // Two generations never collide.
    let other = generate_key(GenerateKind::Ed25519, "enc", Some("s3cret pass")).unwrap();
    assert_ne!(other.fingerprint, key.fingerprint);
}

#[test]
fn generated_rsa_4096() {
    let key = generate_key(GenerateKind::Rsa4096, "rsa-test", None).unwrap();
    assert_eq!(key.algorithm, KeyAlgorithm::Rsa);
    assert_eq!(key.bits, 4096);
    assert!(key.public_openssh.starts_with("ssh-rsa "));
    let parsed = parse_private_key(&key.openssh_private, None).unwrap();
    assert_eq!(parsed.fingerprint, key.fingerprint);
}

/// The fingerprint and public key we compute must match what OpenSSH computes for the same file.
#[test]
fn generated_keys_agree_with_ssh_keygen() {
    if std::process::Command::new("ssh-keygen")
        .arg("-?")
        .output()
        .is_err()
    {
        eprintln!("ssh-keygen not available: skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for (kind, passphrase) in [
        (GenerateKind::Ed25519, None),
        (GenerateKind::Ed25519, Some("pw-123")),
    ] {
        let key = generate_key(kind, "cross-check", passphrase).unwrap();
        let path = dir.path().join("k");
        fs::write(&path, key.openssh_private.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        // `-y` derives the public key from the private file (decrypting with -P if needed).
        let mut cmd = std::process::Command::new("ssh-keygen");
        cmd.args(["-y", "-P", passphrase.unwrap_or(""), "-f"])
            .arg(&path);
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "ssh-keygen -y: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let derived = String::from_utf8(out.stdout).unwrap();
        let ours = key
            .public_openssh
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        let theirs = derived
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(ours, theirs);

        let pub_path = dir.path().join("k.pub");
        fs::write(&pub_path, &key.public_openssh).unwrap();
        let out = std::process::Command::new("ssh-keygen")
            .args(["-l", "-E", "sha256", "-f"])
            .arg(&pub_path)
            .output()
            .unwrap();
        let line = String::from_utf8(out.stdout).unwrap();
        assert_eq!(line.split_whitespace().nth(1).unwrap(), key.fingerprint);
    }
}
