use thiserror::Error;

#[derive(Debug, Error)]
pub enum HIP4Error {
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, HIP4Error>;
