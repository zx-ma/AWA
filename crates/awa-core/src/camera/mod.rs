pub mod ir;
pub mod rgb;

use std::path::Path;

use image::{GrayImage, RgbImage};

use crate::error::AwaResult;
use ir::IrCamera;
use rgb::RgbCamera;

pub struct CameraSet {
    pub rgb: RgbCamera,
    pub ir: Option<IrCamera>,
}

pub struct CaptureResult {
    pub rgb: RgbImage,
    pub ir: Option<GrayImage>,
}

pub struct CameraConfig<'a> {
    pub rgb_path: &'a Path,
    pub rgb_width: u32,
    pub rgb_height: u32,
    pub ir_path: Option<&'a Path>,
    pub ir_width: u32,
    pub ir_height: u32,
}

impl CameraSet {
    pub fn open(cfg: &CameraConfig) -> AwaResult<Self> {
        let rgb = RgbCamera::open(cfg.rgb_path, cfg.rgb_width, cfg.rgb_height)?;

        let ir = match cfg.ir_path {
            Some(p) => match IrCamera::open(p, cfg.ir_width, cfg.ir_height) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("ir camera open failed, degrading to rgb-only: {}", e);
                    None
                }
            },
            None => None,
        };

        Ok(Self { rgb, ir })
    }

    pub fn capture(&self) -> AwaResult<CaptureResult> {
        let rgb = self.rgb.capture()?;
        let ir = match &self.ir {
            Some(c) => match c.capture() {
                Ok(g) => Some(g),
                Err(e) => {
                    tracing::warn!("ir capture failed: {}", e);
                    None
                }
            },
            None => None,
        };
        Ok(CaptureResult { rgb, ir })
    }

    pub fn has_ir(&self) -> bool {
        self.ir.is_some()
    }
}
