use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

impl Error {
    pub fn msg<S: Into<String>>(s: S) -> Self { Self::Msg(s.into()) }
}

#[derive(Debug, Serialize)]
pub struct ErrorBody { pub message: String }

impl From<Error> for ErrorBody {
    fn from(e: Error) -> Self { Self { message: e.to_string() } }
}

pub type Result<T> = std::result::Result<T, Error>;
