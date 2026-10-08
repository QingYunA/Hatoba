#!/usr/bin/env bash
# Runs the end-to-end smoke test on Linux: a throwaway sshd, Xvfb, D-Bus, tauri-driver and a debug
# build of the app. Requires: openssh-server, xvfb, dbus, webkit2gtk-driver, `cargo install tauri-driver`,
# and the frontend dev server (`pnpm dev`, port 1420) because debug builds load the devUrl.
# Must run as root (creates a test user for password authentication).
set -euo pipefail
cd "$(dirname "$0")/../../.."
WORK=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null || true; pkill -x WebKitWebDriver || true; rm -rf "$WORK"' EXIT

id hatobatest >/dev/null 2>&1 || useradd -m hatobatest
echo "hatobatest:hatoba-pw-123" | chpasswd
mkdir -p /run/sshd
ssh-keygen -q -t ed25519 -N "" -f "$WORK/host_key"
cat > "$WORK/sshd_config" <<CFG
Port 2299
ListenAddress 127.0.0.1
HostKey $WORK/host_key
PidFile $WORK/sshd.pid
UsePAM no
PasswordAuthentication yes
Subsystem sftp internal-sftp
UseDNS no
CFG
/usr/sbin/sshd -D -e -f "$WORK/sshd_config" 2>"$WORK/sshd.log" &

CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo build -p hatoba-desktop
BIN=${CARGO_TARGET_DIR:-target}/debug/hatoba-desktop

Xvfb :99 -screen 0 1440x900x24 -nolisten tcp &
export DISPLAY=:99 LANG=en_US.UTF-8
rm -rf "$HOME/.local/share/app.hatoba.desktop"
dbus-run-session -- bash -c "tauri-driver & sleep 2; node apps/desktop/e2e/smoke.mjs '$BIN' '${1:-e2e-shots}'"
