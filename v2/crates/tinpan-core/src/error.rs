use thiserror::Error;

/// A failure the app can say in one sentence. The message is already translated.
#[derive(Debug, Error)]
pub enum Error {
    /// Suno no longer accepts the sign-in (it said so; a network failure isn't this).
    #[error("{0}")]
    SignInRequired(String),

    /// Suno, or the network on the way, turned the request down. `status` is the HTTP status, if there was one.
    #[error("{message}")]
    Api { message: String, status: Option<u16> },

    #[error("{0}")]
    Io(String),

    /// The caller cancelled the work (moved on to another song, closed a window).
    #[error("cancelled")]
    Cancelled,
}

impl Error {
    pub fn api(message: impl Into<String>) -> Self {
        Error::Api { message: message.into(), status: None }
    }

    pub fn api_status(message: impl Into<String>, status: u16) -> Self {
        Error::Api { message: message.into(), status: Some(status) }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } => *status,
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
