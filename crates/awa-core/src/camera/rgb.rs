use std::path::{Path, PathBuf};

use image::RgbImage;
use v4l::buffer::Type;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::{Format, FourCC};

use crate::error::AwaResult;

pub struct RgbCamera {
    device: Device,
    path: PathBuf,
    width: u32,
    height: u32,
}

impl RgbCamera {
    pub fn open(path: impl AsRef<Path>, width: u32, height: u32) -> AwaResult<Self> {
        let path = path.as_ref().to_path_buf();
        let device = Device::with_path(&path)?;
        let format = Format::new(width, height, FourCC::new(b"MJPG"));
        Capture::set_format(&device, &format)?;
        Ok(Self {
            device,
            path,
            width,
            height,
        })
    }

    pub fn capture(&self) -> AwaResult<RgbImage> {
        let mut stream = MmapStream::with_buffers(&self.device, Type::VideoCapture, 4)?;
        for _ in 0..3 {
            stream.next()?;
        }
        let (buf, _meta) = stream.next()?;
        let img = image::load_from_memory(buf)?;
        Ok(img.to_rgb8())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
