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
        std::thread::scope(|s| {
            let rgb_handle = s.spawn(|| self.rgb.capture());
            let ir_handle = self.ir.as_ref().map(|c| s.spawn(|| c.capture()));

            let rgb = rgb_handle.join().expect("rgb capture thread panicked")?;

            let ir = match ir_handle {
                Some(h) => match h.join().expect("ir capture thread panicked") {
                    Ok(g) => Some(g),
                    Err(e) => {
                        tracing::warn!("ir capture failed: {}", e);
                        None
                    }
                },
                None => None,
            };

            Ok(CaptureResult { rgb, ir })
        })
    }

    pub fn has_ir(&self) -> bool {
        self.ir.is_some()
    }
}
