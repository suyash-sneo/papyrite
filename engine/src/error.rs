use std::fmt::{self, Display, Formatter};
use std::io;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum DbError {
    Io(io::Error),
    Json(serde_json::Error),
    Utf8(Utf8Error),
    InvalidFormat(String),
    InvalidData(String),
    InvalidRootDocument,
    MissingId,
    DuplicateId(String),
    NotFound(String),
    UnsupportedQuery(String),
    TypeMismatch(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

impl Display for DbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
            Self::Utf8(err) => write!(f, "UTF-8 error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Self::InvalidRootDocument => write!(f, "root document must be an object"),
            Self::MissingId => write!(f, "document is missing required string _id"),
            Self::DuplicateId(id) => write!(f, "duplicate _id: {id}"),
            Self::NotFound(id) => write!(f, "document not found: {id}"),
            Self::UnsupportedQuery(msg) => write!(f, "unsupported query: {msg}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<io::Error> for DbError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for DbError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<Utf8Error> for DbError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8(value)
    }
}
