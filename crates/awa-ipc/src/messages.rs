// request / response message types
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Authenticate {
        username: String,
    },
    Enroll {
        username: String,
        label: String,
        num_samples: usize,
    },
    Status,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    AuthSuccess {
        similarity: f32,
        liveness_score: f32,
    },
    AuthFailure {
        reason: String,
        best_similarity: f32,
    },
    EnrollSuccess {
        samples_collected: usize,
    },
    EnrollFailure {
        reason: String,
    },
    StatusResponse {
        models_loaded: bool,
        camera_ready: bool,
        has_ir: bool,
    },
    ShutdownAck,
    Error {
        message: String,
    },
}
