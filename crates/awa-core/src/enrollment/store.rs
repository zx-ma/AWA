// enrollment persistence
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;

use crate::error::{AwaError, AwaResult};

use super::{CURRENT_SCHEMA_VERSION, EnrollmentFile};

pub struct EnrollmentStore {
    base_dir: PathBuf,
}

impl EnrollmentStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn path_for(&self, username: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", username))
    }

    pub fn load(&self, username: &str) -> AwaResult<Option<EnrollmentFile>> {
        let path = self.path_for(username);
        match fs::read(&path) {
            Ok(data) => {
                let file: EnrollmentFile = serde_json::from_slice(&data)
                    .map_err(|e| AwaError::Config(format!("parse {}: {}", path.display(), e)))?;
                if file.schema_version != CURRENT_SCHEMA_VERSION {
                    return Err(AwaError::Config(format!(
                        "schema version {} unsupported (expected {})",
                        file.schema_version, CURRENT_SCHEMA_VERSION
                    )));
                }
                Ok(Some(file))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, file: &EnrollmentFile) -> AwaResult<()> {
        fs::create_dir_all(&self.base_dir)?;
        let final_path = self.path_for(&file.username);
        let tmp_path = self
            .base_dir
            .join(format!(".{}.json.tmp.{}", file.username, process::id()));

        let data = serde_json::to_vec_pretty(file)
            .map_err(|e| AwaError::Config(format!("serialize: {}", e)))?;

        let mut tmp = fs::File::create(&tmp_path)?;
        tmp.write_all(&data)?;
        tmp.sync_all()?;
        drop(tmp);

        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))?;
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }
}

use chrono::Utc;

use super::{EnrollmentRecord, MAX_SAMPLES_PER_LABEL, Sample};
use crate::pipeline::arcface::{EMBEDDING_DIM, cosine_similarity};

impl EnrollmentStore {
    pub fn add_sample(
        &self,
        username: &str,
        label: &str,
        embedding: [f32; EMBEDDING_DIM],
        model_version: &str,
    ) -> AwaResult<()> {
        let mut file = self
            .load(username)?
            .unwrap_or_else(|| EnrollmentFile::new(username));

        let now = Utc::now();
        let sample = Sample {
            embedding: embedding.to_vec(),
            captured_at: now,
            model_version: model_version.to_string(),
        };

        let pos = file.records.iter().position(|r| r.label == label);
        let record = match pos {
            Some(i) => &mut file.records[i],
            None => {
                file.records.push(EnrollmentRecord {
                    label: label.to_string(),
                    samples: Vec::new(),
                    created_at: now,
                });
                file.records.last_mut().unwrap()
            }
        };

        record.samples.push(sample);

        if record.samples.len() > MAX_SAMPLES_PER_LABEL {
            let drop = record.samples.len() - MAX_SAMPLES_PER_LABEL;
            record.samples.drain(0..drop);
        }

        self.save(&file)?;
        Ok(())
    }

    pub fn best_similarity(
        &self,
        username: &str,
        query: &[f32; EMBEDDING_DIM],
    ) -> AwaResult<Option<f32>> {
        let file = match self.load(username)? {
            Some(f) => f,
            None => return Ok(None),
        };

        let mut best: Option<f32> = None;
        for record in &file.records {
            for sample in &record.samples {
                if let Some(emb) = sample.embedding_as_array() {
                    let sim = cosine_similarity(&emb, query);
                    best = Some(match best {
                        Some(b) => b.max(sim),
                        None => sim,
                    });
                }
            }
        }

        Ok(best)
    }
}
