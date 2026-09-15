use crate::replay::FailureRecord;

/// A tagged payload did not decode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("unknown type `{0}`")]
    UnknownType(String),
    #[error("malformed: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueryError {
    #[error("unknown query type `{0}`")]
    UnknownType(String),
    #[error("malformed query: {0}")]
    Malformed(String),
    #[error("query rejected: {0}")]
    Rejected(String),
}

/// Returned to the bot, which may act again. Never forfeits by itself.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionError {
    #[error("unknown action type `{0}`")]
    UnknownType(String),
    #[error("malformed action: {0}")]
    Malformed(String),
    #[error("invalid action: {0}")]
    Invalid(String),
    #[error("the set is already over")]
    SetOver,
}

impl From<DecodeError> for ActionError {
    fn from(e: DecodeError) -> Self {
        match e {
            DecodeError::UnknownType(t) => ActionError::UnknownType(t),
            DecodeError::Malformed(m) => ActionError::Malformed(m),
        }
    }
}

impl From<DecodeError> for QueryError {
    fn from(e: DecodeError) -> Self {
        match e {
            DecodeError::UnknownType(t) => QueryError::UnknownType(t),
            DecodeError::Malformed(m) => QueryError::Malformed(m),
        }
    }
}

/// A bot's runtime failed. The engine drops the runtime and tells the game, which
/// decides what it means for the set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotFailure {
    /// The team's code raised, or could not be loaded for this bot.
    Exception {
        kind: String,
        message: String,
        traceback: String,
    },
    /// The bot's host (thread, process) went away.
    Crash(String),
}

impl BotFailure {
    pub fn exception(kind: impl Into<String>, message: impl Into<String>) -> Self {
        BotFailure::Exception {
            kind: kind.into(),
            message: message.into(),
            traceback: String::new(),
        }
    }

    pub fn with_traceback(mut self, traceback: impl Into<String>) -> Self {
        if let BotFailure::Exception { traceback: tb, .. } = &mut self {
            *tb = traceback.into();
        }
        self
    }

    pub fn detail(&self) -> String {
        match self {
            BotFailure::Exception { kind, message, .. } if message.is_empty() => kind.clone(),
            BotFailure::Exception { kind, message, .. } => format!("{kind}: {message}"),
            BotFailure::Crash(m) => format!("crash: {m}"),
        }
    }

    pub fn record(&self) -> FailureRecord {
        match self {
            BotFailure::Exception {
                kind,
                message,
                traceback,
            } => FailureRecord::new(kind, message, traceback),
            BotFailure::Crash(m) => FailureRecord::new("Crash", m, ""),
        }
    }
}

/// Aborts the whole match.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("invalid match configuration: {0}")]
    Config(String),
    #[error("unknown game `{0}`")]
    UnknownGame(String),
    #[error("game error: {0}")]
    Game(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A caller-supplied hook failed.
    #[error("callback failed: {0}")]
    Callback(#[source] Box<dyn std::error::Error + Send + Sync>),
}
