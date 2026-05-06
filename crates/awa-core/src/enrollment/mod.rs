pub mod store;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_SAMPLES_PER_LABEL: usize = 5;
pub const EMBEDDING_DIM: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub embedding: Vec<f32>,
    pub captured_at: DateTime<Utc>,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRecord {
    pub label: String,
    pub samples: Vec<Sample>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentFile {
    pub schema_version: u32,
    pub username: String,
    pub records: Vec<EnrollmentRecord>,
}

impl EnrollmentFile {
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            username: username.into(),
            records: Vec::new(),
        }
    }
}

impl Sample {
    pub fn embedding_as_array(&self) -> Option<[f32; EMBEDDING_DIM]> {
        self.embedding.as_slice().try_into().ok()
    }
}
