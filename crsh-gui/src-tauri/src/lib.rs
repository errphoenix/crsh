use crsh_core::{Command, HistoryLn, MasterEndpoint, Remote, SubmitRequest};
use std::str::FromStr;
use tauri::async_runtime::Mutex;
use tauri::State;

/// Just stores remote and keeps it on record. Trusts front-end on assuming it is a valid one.
#[tauri::command]
async fn set_remote(state: State<'_, Mutex<AppState>>, remote: &str) -> Result<(), String> {
    let mut state = state.lock().await;
    state.remote = Some(MasterEndpoint::new(
        Remote::from_str(remote).map_err(|e| e.to_string())?,
    ));
    Ok(())
}

#[tauri::command]
async fn submit(
    state: State<'_, Mutex<AppState>>,
    broadcast: bool,
    cmd: &str,
    token: Option<&str>,
) -> Result<(), String> {
    let state = state.lock().await;
    if let Some(master) = &state.remote {
        let req = if broadcast || token.is_none() {
            SubmitRequest::Broadcast {
                cmd: Command(cmd.to_string()),
            }
        } else {
            SubmitRequest::Single {
                token: token.unwrap().to_string(),
                cmd: Command(cmd.to_string()),
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
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![set_remote, submit, reset, query])
        .manage(Mutex::new(AppState::default()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
