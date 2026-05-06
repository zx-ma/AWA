use image::RgbImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

use crate::error::{AwaError, AwaResult};

pub const MINIFAS_INPUT_SIZE: u32 = 128;
const BBOX_EXPAND: f32 = 2.7;
const SOFTMAX_TEMPERATURE: f32 = 10.0;

pub fn liveness_score(session: &mut Session, img: &RgbImage, bbox: [f32; 4]) -> AwaResult<f32> {
    let crop = expand_and_crop(img, bbox);

    let mut tensor = Array4::<f32>::zeros((1, 3, 128, 128));
    for (x, y, pixel) in crop.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        tensor[[0, 0, y, x]] = (pixel[0] as f32 - 127.5) / 128.0;
        tensor[[0, 1, y, x]] = (pixel[1] as f32 - 127.5) / 128.0;
        tensor[[0, 2, y, x]] = (pixel[2] as f32 - 127.5) / 128.0;
    }

    let outputs = session
        .run(ort::inputs![
            TensorRef::from_array_view(&tensor).map_err(|e| AwaError::Inference(e.to_string()))?
        ])
        .map_err(|e| AwaError::Inference(e.to_string()))?;

    let raw = outputs[0]
        .try_extract_array::<f32>()
        .map_err(|e| AwaError::Inference(e.to_string()))?;

    let logits = [raw[[0, 0]], raw[[0, 1]]];
    let probs = softmax(logits);
    Ok(probs[1])
}

fn expand_and_crop(img: &RgbImage, bbox: [f32; 4]) -> RgbImage {
    let cx = (bbox[0] + bbox[2]) * 0.5;
    let cy = (bbox[1] + bbox[3]) * 0.5;
    let half = (bbox[2] - bbox[0]).max(bbox[3] - bbox[1]) * 0.5 * BBOX_EXPAND;

    let (img_w, img_h) = (img.width() as f32, img.height() as f32);
    let x1 = (cx - half).clamp(0.0, img_w - 1.0) as u32;
    let y1 = (cy - half).clamp(0.0, img_h - 1.0) as u32;
    let x2 = (cx + half).clamp(0.0, img_w) as u32;
    let y2 = (cy + half).clamp(0.0, img_h) as u32;

    let cropped = image::imageops::crop_imm(img, x1, y1, x2 - x1, y2 - y1).to_image();

    image::imageops::resize(
        &cropped,
        MINIFAS_INPUT_SIZE,
        MINIFAS_INPUT_SIZE,
        image::imageops::FilterType::Triangle,
    )
}

fn softmax(logits: [f32; 2]) -> [f32; 2] {
    let scaled = [
        logits[0] / SOFTMAX_TEMPERATURE,
        logits[1] / SOFTMAX_TEMPERATURE,
    ];
    let max = scaled[0].max(scaled[1]);
    let e0 = (scaled[0] - max).exp();
    let e1 = (scaled[1] - max).exp();
    let sum = e0 + e1;
    [e0 / sum, e1 / sum]
}
