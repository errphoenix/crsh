use crate::{Command, HistoryLn};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    pub client: String,
    pub key: u16,
    pub token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum AuthResult {
    Success { token: String },
    Failure { reason: String },
}

impl Display for AuthResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthResult::Success { token } => write!(f, "{token}"),
            AuthResult::Failure { reason } => write!(f, "failed: {reason}"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PollRequest {
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum PollResult {
    Success { queue: Vec<Command> },
    EmptyQueue,
    Failure { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubmitRequest {
    Broadcast { cmd: Command },
    Single { token: String, cmd: Command },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubmitResult {
    Sent,
    NoTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushRequest {
    pub token: String,
    pub out: Vec<HistoryLn>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryQuery(pub Vec<HistoryLn>);
