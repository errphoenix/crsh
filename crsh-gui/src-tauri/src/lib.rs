use crsh_core::{
    Command, FileSystemView, FsEstResult, HistoryLn, MasterEndpoint, Remote, SubmitRequest,
};
use std::str::FromStr;
use tauri::async_runtime::Mutex;
use tauri::State;

#[tauri::command]
async fn fs_est(state: State<'_, Mutex<AppState>>, token: &str) -> Result<String, String> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        match master.est_bridge_fs(token).await {
            Ok(r) => match r {
                FsEstResult::Allowed { id } => Ok(id),
                FsEstResult::NotFound => Err("Failed to connect to filesystem".to_string()),
                FsEstResult::Denied => Err("Access to the filesystem is not allowed".to_string()),
            },
            Err(e) => Err(e.to_string()),
        }
    } else {
        Err("You have not bound an endpoint.".to_string())
    }
}

#[tauri::command]
async fn fs_read(
    state: State<'_, Mutex<AppState>>,
    token: &str,
    bridge: &str,
) -> Result<FileSystemView, String> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        master
            .query_filesystem(token, bridge)
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("You have not bound an endpoint.".to_string())
    }
}

/// Just stores remote and keeps it on record. Trusts front-end on assuming it is a valid one.
#[tauri::command]
async fn set_remote(state: State<'_, Mutex<AppState>>, remote: &str) -> Result<(), String> {
    let mut state = state.lock().await;
    let addr = if remote.starts_with("http") {
        remote.to_string()
    } else {
        format!("http://{remote}")
    };
    state.remote = Some(MasterEndpoint::new(
        Remote::from_str(addr.as_str()).map_err(|e| e.to_string())?,
    ));
    Ok(())
}

#[tauri::command]
async fn submit(
    state: State<'_, Mutex<AppState>>,
    broadcast: bool,
    cmd: Command,
    token: Option<&str>,
) -> Result<(), String> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        let req = if broadcast || token.is_none() {
            SubmitRequest::Broadcast { cmd }
        } else {
            SubmitRequest::Single {
                token: token.unwrap().to_string(),
                cmd,
            }
        };
        master.submit(req).await.map_err(|e| e.to_string())
    } else {
        Err("You have not bound an endpoint.".to_string())
    }
}

#[tauri::command]
async fn reset(state: State<'_, Mutex<AppState>>, token: &str) -> Result<(), HistoryLn> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        master
            .reset(token)
            .await
            .map_err(|e| HistoryLn::new_stderr(e.to_string()))
    } else {
        Err(HistoryLn::new_stderr("No remote set.".to_string()))
    }
}

#[tauri::command]
async fn query(state: State<'_, Mutex<AppState>>) -> Result<Vec<HistoryLn>, HistoryLn> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        master
            .query()
            .await
            .map(|r| r.0)
            .map_err(|e| HistoryLn::new_stderr(e.to_string()))
    } else {
        Err(HistoryLn::new_stderr(
            "You have not bound an endpoint.".to_string(),
        ))
    }
}

#[derive(Default)]
pub(crate) struct AppState {
    remote: Option<MasterEndpoint>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            set_remote, submit, reset, query, fs_read, fs_est
        ])
        .manage(Mutex::new(AppState::default()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
