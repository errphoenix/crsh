use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use crsh_core::{
    AuthRequest, AuthResult, HistoryQuery, MasterRouter, PollRequest, PollResult, PushRequest,
    SubmitRequest, SubmitResult,
};
use std::error::Error;
use std::fs;
use std::io::{stdout, Write};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

const VER_STR: &str = "v0.1.0-router";

struct StateHandler {
    key: u16,
    router: MasterRouter,
    tokens: Vec<String>,
}

impl StateHandler {
    pub fn new(key: u16, mut router: MasterRouter) -> Self {
        let tokens = {
            match fs::read_to_string("active") {
                Ok(str) => str
                    .lines()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect(),
                Err(_) => {
                    let _ = fs::File::create("active").map_err(|e| {
                        eprintln!("Failed to initialize active tokens storage:\n{e}");
                    });
                    Vec::new()
                }
            }
        };
        router.register_all(&tokens);

        Self {
            key,
            router,
            tokens,
        }
    }

    pub fn write_active(&self) {
        let mut active = String::new();
        for str in &self.tokens {
            active.push_str(str);
            active.push('\n');
        }
        if let Err(e) = fs::write("active", active.as_bytes()) {
            eprintln!("Failed to write to active tokens storage:\n{e}")
        } else {
            println!(
                "Successfully saved active tokens to file: {} bytes written.",
                active.len()
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let addr = {
        let arg_i = std::env::args()
            .enumerate()
            .find(|(_, a)| a.eq("--addr"))
            .map(|(i, _)| i);

        if let Some(i) = arg_i {
            std::env::args()
                .enumerate()
                .find_map(|(j, a)| if i + 1 == j { Some(a) } else { None })
        } else {
            None
        }
    }
    .expect("Invalid address provided.");

    let (router, key) = {
        let key = fs::read("key")
            .ok()
            .and_then(|b| u16::from_str(String::from_utf8(b).ok()?.as_str()).ok());
        MasterRouter::new(key)
    };
    let handler = StateHandler::new(key, router);
    {
        let stdout = stdout();
        let mut lock = stdout.lock();
        writeln!(lock)?;
        writeln!(
            lock,
            "Centralised Remote Shell - tiny remote shell execution protocol"
        )?;
        writeln!(lock, "Version: {VER_STR}")?;
        writeln!(lock, "Author: HerrPhoenix")?;
        writeln!(lock)?;
        writeln!(lock, "Starting server on {addr}")?;
        writeln!(lock, "Initialising router server...")?;
    }

    let cors = CorsLayer::new().allow_origin(Any);
    let app = Router::new()
        .route("/", get(root))
        .route(crsh_core::ROUTER_AUTH, post(hello))
        .route(crsh_core::ROUTER_ASK_RESET, get(must_reset))
        .route(crsh_core::ROUTER_SET_RESET, post(reset))
        .route(crsh_core::ROUTER_POLL, post(poll))
        .route(crsh_core::ROUTER_SUBMIT, post(submit))
        .route(crsh_core::ROUTER_OUT, post(push_out))
        .route(crsh_core::ROUTER_QUERY_OUT, get(query_out))
        .with_state(Arc::new(Mutex::new(handler)))
        .layer(cors);

    println!("KEY: {key}");
    {
        fs::write("key", key.to_string())?;
    }
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> StatusCode {
    StatusCode::OK
}

async fn hello(
    State(state): State<Arc<Mutex<StateHandler>>>,
    Json(payload): Json<AuthRequest>,
) -> (StatusCode, Json<AuthResult>) {
    let key = payload.key;
    print!(
        "Client {} attempting to authenticate with key {key}...",
        payload.client
    );
    if key != state.lock().unwrap().key {
        println!("FAIL");
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthResult::Failure {
                reason: "invalid key provided".to_string(),
            }),
        );
    }
    let cached = payload.token.is_some();
    let id = state.lock().unwrap().router.register(payload.token);
    println!("SUCCESS [{id}, cached={cached}]");
    {
        let mut lock = state.lock().unwrap();
        if !lock.tokens.contains(&id) {
            lock.tokens.push(id.clone());
            lock.write_active();
        }
    }
    (
        StatusCode::OK,
        Json(AuthResult::Success {
            token: id.to_string(),
        }),
    )
}

async fn must_reset(
    State(state): State<Arc<Mutex<StateHandler>>>,
    Json(payload): Json<PollRequest>,
) -> String {
    let mut guard = state.lock().unwrap();
    guard.router.must_reset(&payload.token).to_string()
}

async fn reset(
    State(state): State<Arc<Mutex<StateHandler>>>,
    Json(payload): Json<PollRequest>,
) -> StatusCode {
    let mut guard = state.lock().unwrap();
    let token = payload.token;
    if !guard.router.is_valid(&token) {
        return StatusCode::NO_CONTENT;
    }
    println!("Requested reset for {}", token);
    guard.router.set_reset(token);
    StatusCode::OK
}

async fn poll(
    State(state): State<Arc<Mutex<StateHandler>>>,
    Json(payload): Json<PollRequest>,
) -> (StatusCode, Json<PollResult>) {
    let mut guard = state.lock().unwrap();
    if let Some(cmd) = guard.router.consume(&payload.token) {
        if cmd.is_empty() {
            (StatusCode::OK, Json(PollResult::EmptyQueue))
        } else {
            println!("{} flushed command queue ({})", payload.token, cmd.len());
            (StatusCode::OK, Json(PollResult::Success { queue: cmd }))
        }
    } else {
        (
            StatusCode::OK,
            Json(PollResult::Failure {
                reason: format!("invalid token {} provided", payload.token),
            }),
        )
    }
}

async fn push_out(State(state): State<Arc<Mutex<StateHandler>>>, Json(payload): Json<PushRequest>) {
    let mut guard = state.lock().unwrap();
    if guard.router.is_valid(&payload.token) {
        guard.router.append_history(payload.out)
    }
}

async fn query_out(State(state): State<Arc<Mutex<StateHandler>>>) -> Json<HistoryQuery> {
    let guard = state.lock().unwrap();
    Json(guard.router.query_history())
}

async fn submit(
    State(state): State<Arc<Mutex<StateHandler>>>,
    Json(payload): Json<SubmitRequest>,
) -> (StatusCode, Json<SubmitResult>) {
    match payload {
        SubmitRequest::Broadcast { cmd } => {
            state.lock().unwrap().router.queue_command(cmd);
            (StatusCode::OK, Json(SubmitResult::Sent))
        }
        SubmitRequest::Single { token, cmd } => {
            if let Err(e) = state
                .lock()
                .unwrap()
                .router
                .queue_command_target(cmd, &token)
            {
                println!("error submitting command: {e:?}");
                (StatusCode::OK, Json(SubmitResult::NoTarget))
            } else {
                (StatusCode::OK, Json(SubmitResult::Sent))
            }
        }
    }
}
