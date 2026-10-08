//! Local port forwarding (FWD-01) and auto-start with the connection (FWD-02).

use hatoba_core::model::{ForwardKind, Item, PortForward};
use hatoba_ssh::LocalForward;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::dto::{ForwardInput, ForwardState, ForwardStatusEvent, ForwardView};
use crate::error::{AppError, AppResult};
use crate::ssh::LiveSession;
use crate::state::{AppState, state as app_state};
use crate::sync;

fn view(id: &str, f: &PortForward) -> ForwardView {
    ForwardView {
        id: id.to_owned(),
        host_id: f.host_id.clone(),
        bind_address: f.bind_address.clone(),
        bind_port: f.bind_port,
        dest_host: f.dest_host.clone().unwrap_or_default(),
        dest_port: f.dest_port.unwrap_or_default(),
        auto_start: f.auto_start,
    }
}

#[tauri::command]
#[specta::specta]
pub fn forwards_list(state: State<'_, AppState>, host_id: String) -> AppResult<Vec<ForwardView>> {
    state.with_unlocked(|v| {
        let mut list: Vec<ForwardView> = v
            .forwards()
            .iter()
            .filter(|(_, f)| f.host_id == host_id && f.kind == ForwardKind::Local)
            .map(|(id, f)| view(id, f))
            .collect();
        list.sort_by_key(|f| (f.bind_port, f.dest_port));
        Ok(list)
    })
}

#[tauri::command]
#[specta::specta]
pub fn forward_save(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ForwardInput,
) -> AppResult<ForwardView> {
    let bind_address = match input.bind_address.trim() {
        "" => "127.0.0.1".to_owned(),
        a => a.to_owned(),
    };
    if bind_address.chars().any(char::is_whitespace) {
        return Err(AppError::invalid("bind_address", "invalid bind address"));
    }
    let dest_host = input.dest_host.trim().to_owned();
    if dest_host.is_empty() || dest_host.chars().any(char::is_whitespace) {
        return Err(AppError::invalid(
            "dest_host",
            "destination host is required",
        ));
    }
    if input.dest_port == 0 {
        return Err(AppError::invalid(
            "dest_port",
            "port must be between 1 and 65535",
        ));
    }
    let saved = state.with_unlocked(|v| {
        v.get(&input.host_id)
            .and_then(Item::as_host)
            .ok_or_else(|| AppError::not_found("host"))?;
        let forward = PortForward {
            host_id: input.host_id.clone(),
            kind: ForwardKind::Local,
            bind_address,
            bind_port: input.bind_port,
            dest_host: Some(dest_host),
            dest_port: Some(input.dest_port),
            auto_start: input.auto_start,
            updated_at: 0,
        };
        let id = v.put(input.id.as_deref(), Item::Forward(forward))?;
        let f = v
            .get(&id)
            .and_then(Item::as_forward)
            .cloned()
            .ok_or_else(|| AppError::not_found("forward"))?;
        Ok(view(&id, &f))
    })?;
    sync::local_change(&app);
    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub fn forward_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_unlocked(|v| {
        v.get(&id)
            .and_then(Item::as_forward)
            .ok_or_else(|| AppError::not_found("forward"))?;
        v.delete(&id)?;
        Ok(())
    })?;
    sync::local_change(&app);
    Ok(())
}

fn emit(
    app: &AppHandle,
    session_id: &str,
    forward_id: &str,
    state: ForwardState,
    local_port: Option<u16>,
    error: Option<String>,
) {
    let _ = ForwardStatusEvent {
        session_id: session_id.to_owned(),
        forward_id: forward_id.to_owned(),
        state,
        local_port,
        error,
    }
    .emit(app);
}

/// Starts one saved forward on a live session and reports its state.
pub async fn start(
    app: &AppHandle,
    session_id: &str,
    live: &LiveSession,
    forward_id: &str,
    f: &PortForward,
) -> AppResult<u16> {
    let spec = LocalForward {
        bind_address: f.bind_address.clone(),
        bind_port: f.bind_port,
        dest_host: f.dest_host.clone().unwrap_or_default(),
        dest_port: f.dest_port.unwrap_or_default(),
    };
    match live.session.local_forward(spec).await {
        Ok(handle) => {
            let port = handle.local_port();
            live.add_forward(forward_id, handle);
            emit(
                app,
                session_id,
                forward_id,
                ForwardState::Running,
                Some(port),
                None,
            );
            Ok(port)
        }
        Err(e) => {
            emit(
                app,
                session_id,
                forward_id,
                ForwardState::Failed,
                None,
                Some(e.message.clone()),
            );
            Err(e.into())
        }
    }
}

/// FWD-02: called after a session connects.
pub async fn start_auto(app: &AppHandle, session_id: &str, live: &LiveSession, host_id: &str) {
    let auto: Vec<(String, PortForward)> = app_state(app)
        .with_unlocked(|v| {
            Ok(v.forwards()
                .into_iter()
                .filter(|(_, f)| {
                    f.host_id == host_id && f.auto_start && f.kind == ForwardKind::Local
                })
                .collect())
        })
        .unwrap_or_default();
    for (id, f) in auto {
        if let Err(e) = start(app, session_id, live, &id, &f).await {
            tracing::warn!(forward_id = %id, "auto-start forward failed: {}", e.detail);
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn forward_start(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    forward_id: String,
) -> AppResult<u16> {
    let live = state.ssh.get(&session_id)?;
    let f = state.with_unlocked(|v| {
        v.get(&forward_id)
            .and_then(Item::as_forward)
            .cloned()
            .ok_or_else(|| AppError::not_found("forward"))
    })?;
    if let Some(port) = live.forward_port(&forward_id) {
        return Ok(port);
    }
    start(&app, &session_id, &live, &forward_id, &f).await
}

#[tauri::command]
#[specta::specta]
pub fn forward_stop(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    forward_id: String,
) -> AppResult<()> {
    let live = state.ssh.get(&session_id)?;
    live.stop_forward(&forward_id);
    emit(
        &app,
        &session_id,
        &forward_id,
        ForwardState::Stopped,
        None,
        None,
    );
    Ok(())
}

/// Forwards currently running on a session: `(forward_id, local_port)`.
#[tauri::command]
#[specta::specta]
pub fn forwards_active(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<Vec<(String, u16)>> {
    Ok(state.ssh.get(&session_id)?.active_forwards())
}
