use std::fmt;

/// Typed failures from the Tangle API. Never contains the access token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TangleError {
    InvalidToken,
    BackupRequired,
    RateLimited { retry_after_s: u64 },
    ListNotFound,
    ItemNotFound,
    Network(String),
    Server(u16),
    Other { status: u16, code: String, request_id: Option<String> },
}

impl fmt::Display for TangleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken => write!(f, "Tangle rejected the access token"),
            Self::BackupRequired => write!(f, "Tangle requires a backup before this action"),
            Self::RateLimited { retry_after_s } => {
                write!(f, "Tangle rate limit hit, retry in {retry_after_s}s")
            }
            Self::ListNotFound => write!(f, "Tangle list not found"),
            Self::ItemNotFound => write!(f, "Tangle item not found"),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::Server(status) => write!(f, "Tangle server error ({status})"),
            Self::Other { status, code, request_id } => {
                write!(f, "Tangle error {status} ({code})")?;
                if let Some(id) = request_id {
                    write!(f, " request_id={id}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for TangleError {}
