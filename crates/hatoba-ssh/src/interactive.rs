//! keyboard-interactive authentication callbacks (SSH-08).

use async_trait::async_trait;
use zeroize::Zeroizing;

/// One prompt of a keyboard-interactive round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Text to show to the user, e.g. `Verification code: `.
    pub text: String,
    /// Whether the answer may be echoed while typing (`false` for passwords/OTP).
    pub echo: bool,
}

/// A keyboard-interactive info request sent by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRequest {
    /// Host (as configured for the hop) that is asking.
    pub host: String,
    /// Optional title chosen by the server (often empty).
    pub name: String,
    /// Optional instructions chosen by the server (often empty).
    pub instructions: String,
    /// Prompts to answer, in order. Never empty when passed to the callback.
    pub prompts: Vec<Prompt>,
}

/// Answers keyboard-interactive prompts (2FA / OTP / PAM challenges).
///
/// The connect timeout is paused while the callback is running.
#[async_trait]
pub trait KeyboardInteractive: Send + Sync {
    /// Returns exactly one answer per prompt, or `None` to abort the login
    /// (the connection then fails with `SshErrorKind::Cancelled`).
    async fn respond(&self, request: PromptRequest) -> Option<Vec<Zeroizing<String>>>;
}
