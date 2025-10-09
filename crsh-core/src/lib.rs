pub mod net;

pub use net::*;
use rand::random;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::num::ParseIntError;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};
use uuid::Uuid;

#[derive(Debug)]
pub enum MasterError {
    Connection(ConnectError),
    TargetNotFound(String),
}

#[derive(Debug)]
pub enum ConnectError {
    TimedOut,
    NotFound,
    InternalError,
    Forbidden,
    Other(String),
}

impl Display for ConnectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "connection error: {}",
            match self {
                ConnectError::TimedOut => "timed out",
                ConnectError::NotFound => "404 not found",
                ConnectError::InternalError => "internal server error",
                ConnectError::Forbidden => "forbidden or unauthorized",
                ConnectError::Other(s) => s.as_str(),
            }
        )
    }
}

pub struct PreConnect;
pub struct Connected;
pub struct Invalid;

#[derive(Clone, Debug)]
pub struct Remote {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub enum RemoteAddrParseError {
    InvalidAddr,
    InvalidPort,
    Nan(ParseIntError),
    BadFormatting,
    NoPort,
}

impl Display for RemoteAddrParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteAddrParseError::InvalidAddr => write!(f, "invalid address string"),
            RemoteAddrParseError::InvalidPort => write!(f, "invalid port in string"),
            RemoteAddrParseError::Nan(e) => write!(f, "port is not a number: {e}"),
            RemoteAddrParseError::BadFormatting => {
                write!(f, "bad formatting (ensure 'address:port')")
            }
            RemoteAddrParseError::NoPort => write!(f, "no port included in address"),
        }
    }
}

impl Error for RemoteAddrParseError {}

impl FromStr for Remote {
    type Err = RemoteAddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(RemoteAddrParseError::BadFormatting);
        }
        let addrp = s.rsplit_once(":");
        if !s.contains(":") || addrp.is_none() {
            return Err(RemoteAddrParseError::BadFormatting);
        }

        let (address, port_str) = addrp.unwrap();
        let port_str = if port_str.ends_with("/") {
            port_str.trim_end_matches("/")
        } else {
            port_str
        };

        if address.is_empty() {
            return Err(RemoteAddrParseError::InvalidAddr);
        }
        if port_str.is_empty() {
            return Err(RemoteAddrParseError::NoPort);
        }
        let port = match port_str.parse::<u16>() {
            Ok(port) => port,
            Err(e) => {
                eprintln!("{e}");
                Err(RemoteAddrParseError::Nan(e))?
            }
        };

        let address = address.to_string();
        Ok(Self { address, port })
    }
}

pub type PingResult = Result<u32, ConnectError>;

pub const ROUTER_AUTH: &str = "/hello";
pub const ROUTER_END: &str = "/bye";
pub const ROUTER_POLL: &str = "/poll";
pub const ROUTER_SET_RESET: &str = "/reset";
pub const ROUTER_ASK_RESET: &str = "/amiok";
pub const ROUTER_OUT: &str = "/out";
pub const ROUTER_QUERY_OUT: &str = "/outq";
pub const ROUTER_SUBMIT: &str = "/cmd";

impl Remote {
    pub async fn ping(&self) -> PingResult {
        let time = Instant::now();
        let resp = Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(self.to_string())
            .send()
            .await
            .map_err(|e| ConnectError::Other(e.to_string()))?;
        let time = time.elapsed();

        match resp.status() {
            StatusCode::OK => Ok(time.as_millis() as u32),
            StatusCode::INTERNAL_SERVER_ERROR => Err(ConnectError::InternalError),
            StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => Err(ConnectError::Forbidden),
            StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
                Err(ConnectError::TimedOut)
            }
            StatusCode::NOT_FOUND => Err(ConnectError::NotFound),
            code => Err(ConnectError::Other(code.to_string())),
        }
    }

    fn as_hello_url(&self) -> String {
        format!("{self}{}", ROUTER_AUTH)
    }

    fn as_poll_url(&self) -> String {
        format!("{self}{}", ROUTER_POLL)
    }

    fn as_set_reset_url(&self) -> String {
        format!("{self}{}", ROUTER_SET_RESET)
    }

    fn as_ask_reset_url(&self) -> String {
        format!("{self}{}", ROUTER_ASK_RESET)
    }

    fn as_out_url(&self) -> String {
        format!("{self}{}", ROUTER_OUT)
    }

    fn as_out_query_url(&self) -> String {
        format!("{self}{}", ROUTER_QUERY_OUT)
    }

    fn as_submit_url(&self) -> String {
        format!("{self}{}", ROUTER_SUBMIT)
    }
}

impl Display for Remote {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

impl From<&Remote> for Url {
    fn from(value: &Remote) -> Self {
        Url::from_str(format!("{value}").as_str()).unwrap()
    }
}

impl From<Remote> for Url {
    fn from(value: Remote) -> Self {
        (&value).into()
    }
}

pub const HISTORY_LENGTH: usize = 340;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryLn {
    #[serde(rename = "message")]
    pub inner: String,
    #[serde(rename = "stdtype")]
    pub out_type: OutType,
}

impl HistoryLn {
    pub fn new(message: String, out_type: OutType) -> Self {
        Self {
            inner: message,
            out_type,
        }
    }

    pub fn new_stderr(message: String) -> Self {
        Self::new(message, OutType::Err)
    }

    pub fn new_stdout(message: String) -> Self {
        Self::new(message, OutType::Out)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutType {
    Err,
    Out,
}

impl Display for HistoryLn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.out_type, self.inner)
    }
}

impl Display for OutType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OutType::Err => write!(f, "err"),
            OutType::Out => write!(f, "out"),
        }
    }
}

pub struct MasterRouter {
    history: VecDeque<HistoryLn>,
    queue: HashMap<String, Arc<Mutex<Vec<Command>>>>,
    reset: HashSet<String>,
}

const COMMAND_BUFFER_ALLOC: usize = 8;

impl MasterRouter {
    pub fn new(key: Option<u16>) -> (Self, u16) {
        (
            Self {
                history: VecDeque::with_capacity(HISTORY_LENGTH),
                queue: HashMap::new(),
                reset: HashSet::new(),
            },
            key.unwrap_or_else(random::<u16>),
        )
    }

    pub fn set_reset(&mut self, token: String) {
        self.reset.insert(token);
    }

    pub fn must_reset(&mut self, token: &str) -> bool {
        self.reset.remove(token)
    }

    pub fn register_all(&mut self, tokens: &[String]) {
        if tokens.is_empty() {
            return;
        }
        for token in tokens {
            self.queue
                .entry(token.clone())
                .or_insert_with(|| Arc::new(Mutex::new(Vec::with_capacity(COMMAND_BUFFER_ALLOC))));
        }
        println!("Registered {} tokens from storage.", tokens.len());
    }

    pub fn register(&mut self, token: Option<String>) -> String {
        let mut cached = true;
        let token = token.unwrap_or_else(|| {
            cached = false;
            Uuid::new_v4().to_string()
        });
        self.queue
            .entry(token.clone())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::with_capacity(COMMAND_BUFFER_ALLOC))));
        token
    }

    pub fn delete(&mut self, token: &str) {
        self.queue.remove(token);
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.queue.contains_key(token)
    }

    const HISTORY_EVICT_ITER: usize = 72;

    pub fn append_history(&mut self, hist: Vec<HistoryLn>) {
        let len = hist.len();
        if self.history.len() + len >= HISTORY_LENGTH {
            self.history
                .drain(0..Self::HISTORY_EVICT_ITER.min(self.history.len()));
        }
        let hist: Vec<HistoryLn> = hist.iter().take(HISTORY_LENGTH).cloned().collect();
        self.history.extend(hist);
    }

    pub fn query_history(&self) -> HistoryQuery {
        HistoryQuery(self.history.clone().into())
    }

    /// # Return
    /// [`MasterError::TargetNotFound`] if target token is not registered
    pub fn queue_command_target(
        &mut self,
        command: Command,
        token: &str,
    ) -> Result<(), MasterError> {
        if let Some(queue) = self.queue.get_mut(token) {
            queue.lock().unwrap().push(command);
            Ok(())
        } else {
            Err(MasterError::TargetNotFound(token.to_string()))
        }
    }

    pub fn queue_command(&mut self, command: Command) {
        self.queue
            .values_mut()
            .for_each(|v| v.lock().unwrap().push(command.clone()));
    }

    pub fn consume(&'_ mut self, token: &str) -> Option<Vec<Command>> {
        self.queue
            .get(token)
            .map(|v| v.lock().unwrap().drain(..).collect())
    }
}

#[derive(Debug, Clone)]
pub struct MasterEndpoint(pub Remote, Client);

impl Display for MasterEndpoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "master={}", self.0)
    }
}

#[derive(Debug)]
pub enum EndpointError {
    ConnFailure { e: ConnectError },
    SubmitFailure(String),
    QueryFailure(String),
    ResetFailure(String),
}

impl From<ConnectError> for EndpointError {
    fn from(value: ConnectError) -> Self {
        Self::ConnFailure { e: value }
    }
}

impl Display for EndpointError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointError::ConnFailure { e } => write!(f, "{e}"),
            EndpointError::SubmitFailure(r) => write!(f, "submit failure: {r}"),
            EndpointError::QueryFailure(r) => write!(f, "query: failure: {r}"),
            EndpointError::ResetFailure(r) => write!(f, "reset failure: {r}"),
        }
    }
}

impl MasterEndpoint {
    pub fn parse(str: &str) -> Result<Self, RemoteAddrParseError> {
        let inner = str.strip_prefix("master=").unwrap_or(str);
        Ok(Self::new(Remote::from_str(inner)?))
    }

    pub fn new(remote: Remote) -> Self {
        Self(remote, Client::builder().no_proxy().build().unwrap())
    }

    pub async fn submit(&self, request: SubmitRequest) -> Result<(), EndpointError> {
        self.0.ping().await?;
        if let Err(e) = self
            .1
            .post(self.0.as_submit_url())
            .json(&request)
            .send()
            .await
        {
            Err(EndpointError::SubmitFailure(e.to_string()))
        } else {
            Ok(())
        }
    }

    pub async fn query(&self) -> Result<HistoryQuery, EndpointError> {
        self.0.ping().await?;
        match self.1.get(self.0.as_out_query_url()).send().await {
            Ok(res) => Ok(res
                .json::<HistoryQuery>()
                .await
                .map_err(|e| EndpointError::QueryFailure(e.to_string()))?),
            Err(e) => Err(EndpointError::QueryFailure(e.to_string())),
        }
    }

    pub async fn reset(&self, token: &str) -> Result<(), EndpointError> {
        self.0.ping().await?;
        let req = PollRequest {
            token: token.to_string(),
        };
        if let Err(e) = self
            .1
            .post(self.0.as_set_reset_url())
            .json(&req)
            .send()
            .await
        {
            Err(EndpointError::ResetFailure(e.to_string()))
        } else {
            Ok(())
        }
    }
}

/// Client-side (receiver) master state
pub struct Agent<Status> {
    pub remote: Remote,
    client: Option<Client>,
    _marker: PhantomData<Status>,
}

impl Agent<PreConnect> {
    pub fn new(remote: Remote) -> Self {
        Self {
            remote,
            client: None,
            _marker: PhantomData::<PreConnect>,
        }
    }
}

impl Agent<Invalid> {
    pub fn reset(self) -> Agent<PreConnect> {
        Agent::<PreConnect> {
            remote: self.remote,
            client: self.client,
            _marker: PhantomData::<PreConnect>,
        }
    }
}

pub type ConnectResult = Result<(AuthResult, Arc<RwLock<Agent<Connected>>>), Agent<Invalid>>;

#[derive(Debug)]
pub enum AuthError {
    InvalidBody,
    InvalidKey,
}

impl Agent<PreConnect> {
    pub async fn try_connect(self, request: AuthRequest) -> ConnectResult {
        {
            const RETRY_INTERVAL: Duration = Duration::from_secs(10);
            loop {
                match self.remote.ping().await {
                    Ok(ms) => {
                        println!("Successfully pinged master server in {ms}ms");
                        break;
                    }
                    Err(e) => {
                        eprintln!("Failed to ping remote {}", self.remote);
                        eprintln!("{e}");
                        println!("Retrying in {} seconds...", RETRY_INTERVAL.as_secs());
                        sleep_until(tokio::time::Instant::now() + RETRY_INTERVAL).await;
                    }
                }
            }
            let ms = self.remote.ping().await.map_err(|e| {
                eprintln!("Failed to ping remote {}", self.remote);
                eprintln!("{e}");
                Agent {
                    remote: self.remote.clone(),
                    client: None,
                    _marker: PhantomData::<Invalid>,
                }
            })?;
            println!("Successfully pinged master server in {ms}ms");
        }

        let client = Client::builder().no_proxy().build().unwrap();
        let resp = client
            .post::<String>(self.remote.as_hello_url())
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                eprintln!(
                    "Failed to send connection request to remote {}: {e}",
                    self.remote
                );
                Agent {
                    remote: self.remote.clone(),
                    client: None,
                    _marker: PhantomData::<Invalid>,
                }
            })?;

        let result = resp
            .json::<AuthResult>()
            .await
            .map_err(|e| panic!("Failed to parse authentication result string: {e}"))?;

        match &result {
            AuthResult::Success { token } => {
                println!(
                    "Connected {} to remote {} [token={token}]",
                    request.client, self.remote
                );

                Ok((
                    result,
                    Arc::new(RwLock::new(Agent {
                        remote: self.remote,
                        client: Some(client),
                        _marker: PhantomData::<Connected>,
                    })),
                ))
            }
            AuthResult::Failure { reason } => {
                eprintln!("Failed to register client onto master: {reason}");
                Err(Agent {
                    remote: self.remote.clone(),
                    client: None,
                    _marker: PhantomData::<Invalid>,
                })
            }
        }
    }
}

impl Agent<Connected> {
    pub async fn needs_reset(&self, request: PollRequest) -> bool {
        let client = self.client.as_ref().unwrap();
        let resp_body = client
            .get::<String>(self.remote.as_ask_reset_url())
            .json(&request)
            .send()
            .await;
        if let Ok(resp) = resp_body
            && let Ok(plain) = resp.text().await
        {
            bool::from_str(&plain).unwrap_or_default()
        } else {
            false
        }
    }

    pub async fn poll(&self, request: PollRequest) -> PollResult {
        let client = self.client.as_ref().unwrap();
        let resp_body = client
            .post::<String>(self.remote.as_poll_url())
            .json(&request)
            .send()
            .await;
        if let Err(e) = resp_body {
            return PollResult::Failure {
                reason: format!(
                    "Failed to send poll request to remote {} from client {}: {e}",
                    self.remote, request.token
                ),
            };
        }

        resp_body
            .unwrap()
            .json::<PollResult>()
            .await
            .map_err(|e| PollResult::Failure {
                reason: format!("{e}"),
            })
            .unwrap()
    }

    pub async fn push(&mut self, request: PushRequest) {
        let client = self.client.as_ref().unwrap();
        let _ = client
            .post::<String>(self.remote.as_out_url())
            .json(&request)
            .send()
            .await;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Command(pub String);

impl Deref for Command {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Command {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

pub struct ServingClient {
    pub interval: Duration,
    pub name: &'static str,

    master: Arc<RwLock<Agent<Connected>>>,
    token: String,

    handle: ClientSyncHandle,
    reset_handle: JoinHandle<()>,
    must_reset: Arc<Mutex<bool>>,
}

const DEFAULT_INTERVAL_MS: u64 = 500;
const RESET_QUERY_INTERVAL_MS: u64 = 1000;

/// Takes care of synchronising client with master.
/// Polling commands & pushing outputs.
struct ClientSyncHandle {
    cmd_rx: Option<Receiver<Command>>,
    out_tx: Sender<Vec<HistoryLn>>,
    sync_thread: JoinHandle<()>,
    push_thread: JoinHandle<()>,
    recv_thread: Option<JoinHandle<()>>,
}

impl Drop for ClientSyncHandle {
    fn drop(&mut self) {
        if let Some(recv_thread) = &self.recv_thread {
            recv_thread.abort()
        }
        self.sync_thread.abort();
        self.push_thread.abort();
        println!("Killed all synchronisation threads.");
    }
}

impl Drop for ServingClient {
    fn drop(&mut self) {
        self.reset_handle.abort();
    }
}

impl ServingClient {
    fn init_sync_thread(
        master: Arc<RwLock<Agent<Connected>>>,
        token: String,
        interval: Duration,
    ) -> ClientSyncHandle {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let (out_tx, out_rx) = mpsc::channel::<Vec<HistoryLn>>();

        let sync_thread = {
            let master = master.clone();
            let token = token.clone();
            let out_tx = out_tx.clone();
            tokio::spawn(async move {
                loop {
                    sleep_until(Instant::now() + interval).await;
                    match master
                        .read()
                        .await
                        .poll(PollRequest {
                            token: token.clone(),
                        })
                        .await
                    {
                        PollResult::Success { queue } => {
                            queue
                                .iter()
                                .for_each(|c| cmd_tx.send(Command::from_str(c).unwrap()).unwrap());
                        }
                        PollResult::Failure { reason } => {
                            let _ = out_tx.send(vec![HistoryLn::new_stderr(format!(
                                "Client failed to poll commands: {reason}"
                            ))]);
                            eprintln!("[!] {reason}");
                        }
                        PollResult::EmptyQueue => {}
                    }
                }
            })
        };
        let push_thread = {
            tokio::spawn(async move {
                loop {
                    sleep_until(Instant::now() + interval).await;
                    if let Ok(mut msg) = out_rx.try_recv()
                        && !msg.is_empty()
                    {
                        master
                            .write()
                            .await
                            .push(PushRequest {
                                token: token.clone(),
                                out: std::mem::take(&mut msg),
                            })
                            .await;
                    }
                }
            })
        };

        ClientSyncHandle {
            cmd_rx: Some(cmd_rx),
            out_tx,
            sync_thread,
            push_thread,
            recv_thread: None,
        }
    }

    pub fn new(
        master: Arc<RwLock<Agent<Connected>>>,
        token: String,
        interval: Option<Duration>,
        name: &'static str,
    ) -> Self {
        let interval = interval.unwrap_or_else(|| Duration::from_millis(DEFAULT_INTERVAL_MS));
        let handle = Self::init_sync_thread(master.clone(), token.clone(), interval);
        let must_reset = Arc::new(Mutex::new(false));

        let reset_handle = {
            let must_reset = must_reset.clone();
            let master = master.clone();
            let token = token.clone();
            tokio::spawn(async move {
                let interval = Duration::from_millis(RESET_QUERY_INTERVAL_MS);
                loop {
                    sleep_until(Instant::now() + interval).await;
                    if master
                        .read()
                        .await
                        .needs_reset(PollRequest {
                            token: token.clone(),
                        })
                        .await
                    {
                        *must_reset.lock().unwrap() = true;
                    }
                }
            })
        };

        Self {
            interval,
            name,
            master,
            token,

            handle,
            reset_handle,
            must_reset,
        }
    }

    pub fn recv_handle(&mut self) -> &mut Option<JoinHandle<()>> {
        &mut self.handle.recv_thread
    }

    pub fn sync_handle(&mut self) -> &mut JoinHandle<()> {
        &mut self.handle.sync_thread
    }

    pub async fn run_recv(&mut self) {
        let rx = self.handle.cmd_rx.take();
        if rx.is_none() {
            eprintln!("Broken RX state. Resetting synchronisation handle...");
            self.reset().await;
        }
        let rx = rx.unwrap();
        let out_tx = self.handle.out_tx.clone();
        let master = self.master.clone();
        let token = self.token.clone();
        let interval = self.interval;
        println!("Initialising recv thread...");
        self.handle.recv_thread = Some(tokio::spawn(async move {
            loop {
                sleep_until(Instant::now() + interval).await;
                match rx.try_recv() {
                    Ok(msg) => {
                        let mut w: Vec<String> =
                            msg.0.split_whitespace().map(|s| s.to_string()).collect();
                        let out_tx = out_tx.clone();
                        thread::spawn(move || {
                            let out = std::process::Command::new(w[0].clone())
                                .args(w.drain(1..))
                                .output()
                                .map_err(|e| {
                                    let _ = out_tx.send(vec![
                                        HistoryLn::new_stderr(format!(
                                            "Failed to run command: {msg}"
                                        )),
                                        HistoryLn::new_stderr(format!("{e}")),
                                    ]);
                                    eprintln!("Failed to run command: {msg}");
                                    eprintln!("{e}")
                                });
                            if let Ok(out) = out {
                                if let Ok(out) = String::from_utf8(out.stdout) {
                                    let _ = out_tx.send(
                                        out.lines()
                                            .map(|s| HistoryLn::new_stdout(s.to_string()))
                                            .collect(),
                                    );
                                }
                                if let Ok(err) = String::from_utf8(out.stderr) {
                                    let _ = out_tx.send(
                                        err.lines()
                                            .map(|s| HistoryLn::new_stderr(s.to_string()))
                                            .collect(),
                                    );
                                }
                            }
                        });
                    }
                    Err(TryRecvError::Empty) => {
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error whilst reading from command buffer: {e}");
                        eprintln!("Recv thread has been aborted.");
                        eprintln!(
                            "Note: this action is not performed automatically, but it may be in the future."
                        );
                        {
                            master.write().await.push(PushRequest {
                                token: token.clone(),
                                out: vec![
                                    HistoryLn::new_stderr(format!("Error whilst reading from command buffer: {e}")),
                                    HistoryLn::new_stderr("The RECV thread has been aborted. A reset is necessary to recover the client.".to_string()),
                                    HistoryLn::new_stderr("Note: this action is not performed automatically, but it may be in the future.".to_string()),
                                ]
                            }).await;
                        }
                        break;
                    }
                }
            }
        }));
        println!("Finished initialising working threads.");
    }

    pub async fn handle_reset(&mut self) {
        let interval = Duration::from_millis(RESET_QUERY_INTERVAL_MS);
        loop {
            sleep_until(Instant::now() + interval).await;
            let reset = {
                let mut guard = self.must_reset.lock().unwrap();
                let value = *guard;
                *guard = false;
                value
            };
            if reset && self.reset().await {
                self.run_recv().await;
            }
        }
    }

    /// # Return
    /// `true` if threads were running before reset
    pub async fn reset(&mut self) -> bool {
        let was_running = self.handle.cmd_rx.is_none();
        self.handle =
            Self::init_sync_thread(self.master.clone(), self.token.clone(), self.interval);
        eprintln!("[!] Requested synchronisation handle(s) reset [was_running={was_running}]");
        let _ = self.handle.out_tx.send(vec![HistoryLn::new_stdout(format!(
            "[!] Requested synchronisation handle(s) reset [was_running={was_running}]"
        ))]);
        if was_running {
            println!("[!] Restoring session...");
            let _ = self.handle.out_tx.send(vec![HistoryLn::new_stdout(
                "[!] Restoring session...".to_string(),
            )]);
        } else {
            println!("Synchronisation handle(s) restored.");
            let _ = self.handle.out_tx.send(vec![HistoryLn::new_stdout(
                "Synchronisation handle(s) restored.".to_string(),
            )]);
        }
        was_running
    }
}
