use thiserror::Error;

#[derive(Debug, Error)]
pub enum AwaError {
    #[error("model load failed: {0}")]
    ModelLoad(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("no face detected")]
    NoFaceDetected,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Config(String),
}

impl<T> From<ort::Error<T>> for AwaError {
    fn from(e: ort::Error<T>) -> Self {
        AwaError::ModelLoad(e.to_string())
    }
}

pub type AwaResult<T> = Result<T, AwaError>;
