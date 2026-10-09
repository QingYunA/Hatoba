//! System facts that each platform reports differently: the name this device shows in the sync
//! device list.

/// The first candidate that has text, trimmed.
fn first_non_blank(candidates: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    candidates
        .into_iter()
        .flatten()
        .map(|c| c.trim().to_owned())
        .find(|c| !c.is_empty())
}

/// The name shown in the device list (spec §6): macOS gives the Computer Name from System
/// Settings, Windows `COMPUTERNAME`, Linux `HOSTNAME` or `/etc/hostname`. A GUI app on macOS
/// has none of the environment variables a shell sets, and no `/etc/hostname`.
pub fn device_name() -> String {
    first_non_blank([
        computer_name(),
        std::env::var("COMPUTERNAME").ok(),
        std::env::var("HOSTNAME").ok(),
        std::fs::read_to_string("/etc/hostname").ok(),
    ])
    .unwrap_or_else(|| "Hatoba".to_owned())
}

/// What System Settings → General → About shows as the name, falling back to the Bonjour host
/// name when no Computer Name is set. `scutil` is the supported command line for both and needs
/// no extra dependency.
#[cfg(target_os = "macos")]
fn computer_name() -> Option<String> {
    let scutil = |key: &str| {
        let out = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", key])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    first_non_blank([scutil("ComputerName"), scutil("LocalHostName")])
}

#[cfg(not(target_os = "macos"))]
fn computer_name() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_candidate_with_text_wins() {
        let pick = |c: [Option<&str>; 3]| first_non_blank(c.map(|c| c.map(str::to_owned)));
        assert_eq!(
            pick([Some("Yun's MacBook"), Some("host"), None]),
            Some("Yun's MacBook".into())
        );
        assert_eq!(
            pick([None, Some(" \n"), Some(" host\n")]),
            Some("host".into())
        );
        assert_eq!(pick([Some(""), None, Some("  ")]), None);
    }

    #[test]
    fn a_device_always_has_a_name() {
        assert!(!device_name().trim().is_empty());
    }
}
