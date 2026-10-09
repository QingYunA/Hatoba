//! The `PATH` that MCP stdio servers are looked up on and started with (spec §13.9, AI-32).
//!
//! An app started from Finder or the Dock gets only `/usr/bin:/bin:/usr/sbin:/sbin`, so `npx`,
//! `uvx` or `node` from Homebrew, nvm, mise or Volta are not found. On macOS the login shell's
//! `PATH` is resolved once, in the background: the shell runs once with a short timeout, and
//! when it fails the process `PATH` plus the Homebrew directories is used. Other platforms
//! start the servers with the inherited `PATH`, as before.
//!
//! The value is handed to the servers through their configuration (see
//! `mcp::with_default_path`), never through `std::env::set_var`.

use std::ffi::OsStr;

/// The default `PATH` for MCP stdio servers; `None` where the inherited one is right.
#[cfg(target_os = "macos")]
pub fn default_path() -> Option<&'static OsStr> {
    use std::ffi::OsString;
    use std::sync::OnceLock;

    static PATH: OnceLock<OsString> = OnceLock::new();
    Some(PATH.get_or_init(resolve))
}

#[cfg(not(target_os = "macos"))]
pub fn default_path() -> Option<&'static OsStr> {
    None
}

/// Starts resolving in the background, so the first MCP server start does not wait for the
/// shell. A start that comes first waits for the same result.
pub fn spawn_resolve() {
    if default_path().is_some() {
        std::thread::spawn(|| {
            default_path();
        });
    }
}

#[cfg(target_os = "macos")]
fn resolve() -> std::ffi::OsString {
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(3);
    let shell = login_shell(std::env::var("SHELL").ok());
    let marker = format!("__hatoba_{}__", uuid::Uuid::now_v7().simple());
    match run_login_shell(&shell, &marker, TIMEOUT) {
        Some(path) => {
            tracing::info!(
                entries = path.split(':').count(),
                "MCP default PATH from the login shell"
            );
            tracing::debug!(%path, "MCP default PATH");
            path.into()
        }
        None => {
            let path = fallback_path(std::env::var_os("PATH").as_deref());
            tracing::warn!("login shell PATH unavailable, MCP default PATH is the fallback");
            tracing::debug!(path = %path.to_string_lossy(), "MCP default PATH");
            path
        }
    }
}

/// `$SHELL` when it is an absolute path, else the macOS default.
#[cfg(any(target_os = "macos", test))]
fn login_shell(shell: Option<String>) -> String {
    shell
        .filter(|s| s.starts_with('/'))
        .unwrap_or_else(|| "/bin/zsh".to_owned())
}

/// What `printf` printed between two `marker`s, ignoring whatever the rc files wrote around it.
#[cfg(any(target_os = "macos", test))]
fn parse_marked(output: &str, marker: &str) -> Option<String> {
    let mut parts = output.split(marker);
    parts.next()?;
    let value = parts.next()?.trim();
    // The closing marker must follow, or the shell was cut off.
    parts.next()?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// Runs `shell -ilc` (interactive login, so `.zprofile`, `.zshrc` and the like are read) and
/// returns its `PATH`; `None` when it fails, prints nothing usable, or takes longer than
/// `timeout` (it is then killed). It runs with no stdin and its stderr dropped.
#[cfg(target_os = "macos")]
fn run_login_shell(shell: &str, marker: &str, timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(shell)
        .args([
            "-ilc",
            &format!("printf '%s%s%s' {marker} \"$PATH\" {marker}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    // A reader thread, so the wait below can give up: a shell that leaves a background process
    // holding the pipe would otherwise block the read after the shell itself is gone.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    let output = rx.recv_timeout(timeout).ok();
    if output.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    parse_marked(&String::from_utf8_lossy(&output?), marker)
}

/// The process `PATH` with the Homebrew directories (Apple Silicon, then Intel) added at the end
/// when missing. `split_paths` and `join_paths` use the platform's separator, so the tests that
/// spell out `:` run on Unix only.
#[cfg(any(target_os = "macos", all(test, unix)))]
fn fallback_path(process_path: Option<&OsStr>) -> std::ffi::OsString {
    let mut dirs: Vec<std::path::PathBuf> = process_path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin"] {
        if !dirs.iter().any(|d| d == std::path::Path::new(extra)) {
            dirs.push(extra.into());
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "__hatoba_m__";

    #[test]
    fn the_path_is_read_between_the_markers() {
        let out = format!("Last login: today\nmotd\n{M}/opt/homebrew/bin:/usr/bin{M}\n");
        assert_eq!(
            parse_marked(&out, M),
            Some("/opt/homebrew/bin:/usr/bin".into())
        );
    }

    #[test]
    fn output_without_a_complete_marked_value_is_rejected() {
        assert_eq!(parse_marked("", M), None);
        assert_eq!(parse_marked("/usr/bin", M), None);
        assert_eq!(parse_marked(&format!("{M}/usr/bin"), M), None);
        assert_eq!(parse_marked(&format!("{M}  {M}"), M), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_fallback_adds_the_homebrew_directories_once() {
        let fallback = |p: Option<&str>| fallback_path(p.map(OsStr::new));
        assert_eq!(
            fallback(Some("/usr/bin:/bin")),
            "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin"
        );
        assert_eq!(
            fallback(Some("/usr/local/bin:/usr/bin")),
            "/usr/local/bin:/usr/bin:/opt/homebrew/bin"
        );
        assert_eq!(fallback(None), "/opt/homebrew/bin:/usr/local/bin");
    }

    #[test]
    fn only_an_absolute_shell_path_is_used() {
        assert_eq!(login_shell(Some("/bin/bash".into())), "/bin/bash");
        assert_eq!(login_shell(Some("zsh".into())), "/bin/zsh");
        assert_eq!(login_shell(Some(String::new())), "/bin/zsh");
        assert_eq!(login_shell(None), "/bin/zsh");
    }

    #[cfg(target_os = "macos")]
    mod shell {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, Instant};

        use super::super::run_login_shell;

        /// A script standing in for the login shell; it gets `-ilc <script>` like a real one.
        fn fake_shell(name: &str, body: &str) -> String {
            let path =
                std::env::temp_dir().join(format!("hatoba-test-{name}-{}", std::process::id()));
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.to_string_lossy().into_owned()
        }

        #[test]
        fn the_path_survives_noise_from_the_rc_files() {
            let shell = fake_shell(
                "noisy",
                "echo 'banner'; echo 'oops' >&2; PATH=/opt/x/bin:/usr/bin; eval \"$2\"",
            );
            let path = run_login_shell(&shell, "__m__", Duration::from_secs(5));
            std::fs::remove_file(&shell).unwrap();
            assert_eq!(path, Some("/opt/x/bin:/usr/bin".into()));
        }

        #[test]
        fn a_shell_that_hangs_is_killed_after_the_timeout() {
            let shell = fake_shell("hang", "exec sleep 30");
            let started = Instant::now();
            let path = run_login_shell(&shell, "__m__", Duration::from_millis(300));
            std::fs::remove_file(&shell).unwrap();
            assert_eq!(path, None);
            assert!(started.elapsed() < Duration::from_secs(10));
        }

        #[test]
        fn a_missing_shell_gives_nothing() {
            let path = run_login_shell("/nonexistent/shell", "__m__", Duration::from_secs(1));
            assert_eq!(path, None);
        }

        #[test]
        fn the_real_login_shell_reports_a_path() {
            let path = run_login_shell("/bin/zsh", "__hatoba_real__", Duration::from_secs(10));
            assert!(path.is_some_and(|p| p.contains("/usr/bin")));
        }
    }
}
