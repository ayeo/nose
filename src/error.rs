use std::fmt;

#[derive(Debug)]
pub enum AdapterError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    UnsupportedFormat(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::Io(e) => write!(f, "IO error: {}", e),
            AdapterError::Parse(e) => write!(f, "Parse error: {}", e),
            AdapterError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
        }
    }
}

impl std::error::Error for AdapterError {}

impl From<std::io::Error> for AdapterError {
    fn from(e: std::io::Error) -> Self {
        AdapterError::Io(e)
    }
}

impl From<serde_json::Error> for AdapterError {
    fn from(e: serde_json::Error) -> Self {
        AdapterError::Parse(e)
    }
}
