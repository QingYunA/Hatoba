//! System sleep detection for auto-lock (SEC-02).
//!
//! Windows registers a suspend/resume callback so the vault is locked *before* the machine
//! sleeps or hibernates. Every platform also runs a wall-clock gap detector as a fallback: if the
//! process was frozen for much longer than its tick, the system was asleep.

use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

/// Starts sleep detection; the returned channel yields once per detected suspend.
pub fn watch_sleep() -> mpsc::UnboundedReceiver<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    #[cfg(target_os = "windows")]
    windows_impl::register(tx.clone());
    tauri::async_runtime::spawn(gap_detector(tx));
    rx
}

async fn gap_detector(tx: mpsc::UnboundedSender<()>) {
    const TICK: Duration = Duration::from_secs(5);
    const SLACK: Duration = Duration::from_secs(30);
    let mut last = SystemTime::now();
    loop {
        tokio::time::sleep(TICK).await;
        let now = SystemTime::now();
        if now.duration_since(last).unwrap_or_default() > TICK + SLACK && tx.send(()).is_err() {
            return;
        }
        last = now;
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use tokio::sync::mpsc::UnboundedSender;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Power::{
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, PowerRegisterSuspendResumeNotification,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DEVICE_NOTIFY_CALLBACK, PBT_APMSUSPEND};

    static SENDER: OnceLock<UnboundedSender<()>> = OnceLock::new();
    static mut PARAMS: DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS =
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS { Callback: Some(callback), Context: std::ptr::null_mut() };

    unsafe extern "system" fn callback(_ctx: *const c_void, kind: u32, _setting: *const c_void) -> u32 {
        if kind == PBT_APMSUSPEND
            && let Some(tx) = SENDER.get()
        {
            let _ = tx.send(());
        }
        0
    }

    pub fn register(tx: UnboundedSender<()>) {
        if SENDER.set(tx).is_err() {
            return;
        }
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: PARAMS lives for the whole process; Windows only reads it. The registration is
        // intentionally never removed (it lasts as long as the app).
        let result = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(std::ptr::addr_of_mut!(PARAMS).cast()),
                &mut handle,
            )
        };
        if result.is_err() {
            tracing::warn!("suspend notification registration failed: {result:?}");
        }
    }
}
