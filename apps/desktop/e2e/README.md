# End-to-end smoke test

`smoke.mjs` drives the real app (Rust backend + WebView) through WebDriver and walks the main
path against a real OpenSSH server:

1. first launch → create a vault, read the recovery code, confirm its last group (VAULT-01/02);
2. create a password-auth host (HOST-01);
3. connect → first-connection host-key dialog (SSH-04, the fingerprint must match the server's)
   → run a command and print UTF-8 text over the binary terminal channel (§10.3, TERM-02);
4. open the SFTP panel (SFTP-01);
5. lock with Ctrl+Shift+L, try a wrong password, unlock (SEC-02, VAULT-03, SEC-03).

Screenshots of every step are written to the output directory.

```sh
pnpm dev &                      # debug builds load http://localhost:1420
sudo apps/desktop/e2e/run-smoke.sh ./e2e-shots
```

Linux only (tauri-driver uses WebKitWebDriver; there is no WebDriver for WKWebView on macOS, and
WebView2 needs `msedgedriver` on Windows). After a run you can also confirm that the vault database
holds no plaintext: `grep -a <host name> ~/.local/share/app.hatoba.desktop/vault.db*` must find nothing.
