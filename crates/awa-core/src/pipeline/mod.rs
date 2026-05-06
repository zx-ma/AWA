use std::path::Path;
use std::sync::Arc;

use ort::session::{Session, builder::GraphOptimizationLevel};

use crate::error::AwaResult;

pub mod align;
pub mod arcface;
// pub mod ir_liveness;  // disabled: linux uvc driver does not expose enough ir control to make this useful on commodity hardware
pub mod minifas;
pub mod scrfd;

pub struct ModelPaths<'a> {
    pub scrfd: &'a Path,
    pub arcface: &'a Path,
    pub minifas: &'a Path,
}

pub struct Pipeline {
    pub scrfd: Session,
    pub arcface: Session,
    pub minifas: Session,
}

impl Pipeline {
    pub fn load(paths: &ModelPaths) -> AwaResult<Arc<Self>> {
        Ok(Arc::new(Self {
            scrfd: load_session(paths.scrfd)?,
            arcface: load_session(paths.arcface)?,
            minifas: load_session(paths.minifas)?,
        }))
    }

    pub fn log_io(&self) {
        log_session_io("scrfd", &self.scrfd);
        log_session_io("arcface", &self.arcface);
        log_session_io("minifas", &self.minifas);
    }
}

fn load_session(path: &Path) -> AwaResult<Session> {
    Ok(Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(2)?
        .commit_from_file(path)?)
}

fn log_session_io(name: &str, session: &Session) {
    for input in session.inputs() {
        tracing::info!("{name}: input {} = {:?}", input.name(), input.dtype());
    }
    for output in session.outputs() {
        tracing::info!("{name}: output {} = {:?}", output.name(), output.dtype());
    }
}
