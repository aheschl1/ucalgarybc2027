use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtoError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    #[error("invalid entity id: {0}")]
    InvalidEntityId(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ProtoError>;