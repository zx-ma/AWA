use std::time::{Duration, Instant};

use crate::camera::{CameraConfig, CameraSet};
use crate::config::Config;
use crate::enrollment::store::EnrollmentStore;
use crate::error::AwaResult;
use crate::pipeline::align::align_face;
use crate::pipeline::arcface::extract_embedding;
use crate::pipeline::minifas::liveness_score;
use crate::pipeline::scrfd::detect;
use crate::pipeline::{ModelPaths as PipelineModelPaths, Pipeline};

#[derive(Debug, Clone)]
pub struct AuthReport {
    pub user: String,
    pub face_score: Option<f32>,
    pub similarity: Option<f32>,
    pub liveness_score: Option<f32>,
    pub attempts: usize,
    pub pass_match: bool,
    pub pass_liveness: bool,
    pub pass: bool,
    pub reason: Option<String>,
    pub elapsed: Duration,
}

pub struct AuthEngine {
    cfg: Config,
    pipe: Pipeline,
    cameras: CameraSet,
    store: EnrollmentStore,
}

impl AuthEngine {
    pub fn new(cfg: Config) -> AwaResult<Self> {
        let model_paths = PipelineModelPaths {
            scrfd: &cfg.models.scrfd,
            arcface: &cfg.models.arcface,
            minifas: &cfg.models.minifas,
        };
        let pipe = Pipeline::load(&model_paths)?;

        let cam_cfg = CameraConfig {
            rgb_path: &cfg.camera.rgb_device,
            rgb_width: cfg.camera.rgb_width,
            rgb_height: cfg.camera.rgb_height,
            ir_path: cfg.camera.ir_device.as_deref(),
            ir_width: cfg.camera.ir_width,
            ir_height: cfg.camera.ir_height,
        };
        let cameras = CameraSet::open(&cam_cfg)?;
        let store = EnrollmentStore::new(&cfg.store.base_dir);

        Ok(Self {
            cfg,
            pipe,
            cameras,
            store,
        })
    }

    pub fn authenticate(&mut self, user: &str) -> AwaResult<AuthReport> {
        let started = Instant::now();

        if self.store.load(user)?.is_none() {
            return Ok(AuthReport::failure(
                user,
                format!("user '{user}' not enrolled"),
                0,
                started.elapsed(),
            ));
        }

        let max_attempts = self.cfg.auth.max_samples.max(1);
        let mut best: Option<AuthReport> = None;

        for attempt in 1..=max_attempts {
            let report = self.authenticate_once(user, attempt, started)?;
            tracing::debug!(
                "auth attempt user={} attempt={} pass={} face_score={:.4} similarity={:.4} liveness={:.4} reason={}",
                report.user,
                report.attempts,
                report.pass,
                report.face_score.unwrap_or(0.0),
                report.similarity.unwrap_or(0.0),
                report.liveness_score.unwrap_or(0.0),
                report.reason.as_deref().unwrap_or("ok"),
            );
            if report.pass {
                return Ok(report);
            }
            if best
                .as_ref()
                .is_none_or(|current| report_failure_score(&report) > report_failure_score(current))
            {
                best = Some(report);
            }
        }

        Ok(best.unwrap_or_else(|| {
            AuthReport::failure(user, "authentication failed", 0, started.elapsed())
        }))
    }

    pub fn has_ir(&self) -> bool {
        self.cameras.has_ir()
    }

    fn authenticate_once(
        &mut self,
        user: &str,
        attempt: usize,
        started: Instant,
    ) -> AwaResult<AuthReport> {
        let frame = self.cameras.capture()?;
        let faces = detect(&mut self.pipe.scrfd, &frame.rgb)?;
        let face = match faces.first() {
            Some(face) => face,
            None => {
                return Ok(AuthReport::failure(
                    user,
                    "no face detected",
                    attempt,
                    started.elapsed(),
                ));
            }
        };

        let aligned = align_face(&frame.rgb, &face.keypoints);
        let embedding = extract_embedding(&mut self.pipe.arcface, &aligned)?;
        let liveness = liveness_score(&mut self.pipe.minifas, &frame.rgb, face.bbox)?;
        let similarity = self.store.best_similarity(user, &embedding)?.unwrap_or(0.0);

        let pass_match = similarity >= self.cfg.auth.threshold;
        let pass_liveness = liveness >= self.cfg.auth.liveness_threshold;
        let pass = pass_match && pass_liveness;
        let reason = if pass {
            None
        } else if !pass_match {
            Some("face did not match enrollment".to_string())
        } else {
            Some("liveness check failed".to_string())
        };

        Ok(AuthReport {
            user: user.to_string(),
            face_score: Some(face.score),
            similarity: Some(similarity),
            liveness_score: Some(liveness),
            attempts: attempt,
            pass_match,
            pass_liveness,
            pass,
            reason,
            elapsed: started.elapsed(),
        })
    }
}

impl AuthReport {
    fn failure(user: &str, reason: impl Into<String>, attempts: usize, elapsed: Duration) -> Self {
        Self {
            user: user.to_string(),
            face_score: None,
            similarity: None,
            liveness_score: None,
            attempts,
            pass_match: false,
            pass_liveness: false,
            pass: false,
            reason: Some(reason.into()),
            elapsed,
        }
    }
}

fn report_failure_score(report: &AuthReport) -> f32 {
    let similarity = report.similarity.unwrap_or(0.0);
    let liveness = report.liveness_score.unwrap_or(0.0);
    let face_score = report.face_score.unwrap_or(0.0);
    similarity + liveness + face_score * 0.1
}
