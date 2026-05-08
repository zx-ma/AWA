use image::RgbImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

use crate::error::{AwaError, AwaResult};

pub const SCRFD_INPUT_SIZE: u32 = 640;
const STRIDES: [u32; 3] = [8, 16, 32];
const ANCHORS_PER_LOCATION: usize = 2;
const SCORE_THRESHOLD: f32 = 0.5;
const NMS_THRESHOLD: f32 = 0.4;

#[derive(Debug, Clone)]
pub struct DetectedFace {
    pub bbox: [f32; 4],
    pub keypoints: [[f32; 2]; 5],
    pub score: f32,
}

pub struct Preprocessed {
    pub tensor: Array4<f32>,
    pub scale: f32,
}

pub fn preprocess(img: &RgbImage) -> Preprocessed {
    let (w, h) = (img.width(), img.height());
    let scale = SCRFD_INPUT_SIZE as f32 / w.max(h) as f32;
    let new_w = (w as f32 * scale).round() as u32;
    let new_h = (h as f32 * scale).round() as u32;

    let resized = image::imageops::resize(img, new_w, new_h, image::imageops::FilterType::Triangle);

    let mut canvas = RgbImage::new(SCRFD_INPUT_SIZE, SCRFD_INPUT_SIZE);
    image::imageops::overlay(&mut canvas, &resized, 0, 0);

    let size = SCRFD_INPUT_SIZE as usize;
    let mut tensor = Array4::<f32>::zeros((1, 3, size, size));
    for (x, y, pixel) in canvas.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        tensor[[0, 0, y, x]] = (pixel[0] as f32 - 127.5) / 128.0;
        tensor[[0, 1, y, x]] = (pixel[1] as f32 - 127.5) / 128.0;
        tensor[[0, 2, y, x]] = (pixel[2] as f32 - 127.5) / 128.0;
    }

    Preprocessed { tensor, scale }
}

pub fn detect(session: &mut Session, img: &RgbImage) -> AwaResult<Vec<DetectedFace>> {
    let pre = preprocess(img);

    let outputs = session
        .run(ort::inputs![
            TensorRef::from_array_view(&pre.tensor)
                .map_err(|e| AwaError::Inference(e.to_string()))?
        ])
        .map_err(|e| AwaError::Inference(e.to_string()))?;

    let mut candidates = Vec::new();
    for (i, &stride) in STRIDES.iter().enumerate() {
        let scores = outputs[i]
            .try_extract_array::<f32>()
            .map_err(|e| AwaError::Inference(e.to_string()))?;
        let bboxes = outputs[i + 3]
            .try_extract_array::<f32>()
            .map_err(|e| AwaError::Inference(e.to_string()))?;
        let kps = outputs[i + 6]
            .try_extract_array::<f32>()
            .map_err(|e| AwaError::Inference(e.to_string()))?;

        let grid = SCRFD_INPUT_SIZE / stride;
        for gy in 0..grid {
            for gx in 0..grid {
                for a in 0..ANCHORS_PER_LOCATION {
                    let idx = ((gy * grid + gx) as usize) * ANCHORS_PER_LOCATION + a;
                    let score = scores[[idx, 0]];
                    if score < SCORE_THRESHOLD {
                        continue;
                    }
                    let cx = (gx * stride) as f32;
                    let cy = (gy * stride) as f32;
                    let s = stride as f32;
                    let bbox = [
                        cx - bboxes[[idx, 0]] * s,
                        cy - bboxes[[idx, 1]] * s,
                        cx + bboxes[[idx, 2]] * s,
                        cy + bboxes[[idx, 3]] * s,
                    ];
                    let mut keypoints = [[0.0_f32; 2]; 5];
                    for k in 0..5 {
                        keypoints[k][0] = cx + kps[[idx, k * 2]] * s;

                        keypoints[k][1] = cy + kps[[idx, k * 2 + 1]] * s;
                    }

                    candidates.push(DetectedFace {
                        bbox,
                        keypoints,
                        score,
                    });
                }
            }
        }
    }

    let mut faces = nms(candidates, NMS_THRESHOLD);

    let inv_scale = 1.0 / pre.scale;
    for f in &mut faces {
        f.bbox[0] *= inv_scale;
        f.bbox[1] *= inv_scale;
        f.bbox[2] *= inv_scale;
        f.bbox[3] *= inv_scale;
        for k in &mut f.keypoints {
            k[0] *= inv_scale;
            k[1] *= inv_scale;
        }
    }

    Ok(faces)
}

fn nms(mut faces: Vec<DetectedFace>, threshold: f32) -> Vec<DetectedFace> {
    faces.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept = Vec::new();
    while let Some(f) = faces.first().cloned() {
        let bbox = f.bbox;
        kept.push(f);
        faces.retain(|other| iou(&bbox, &other.bbox) < threshold);
    }
    kept
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let area_a = (a[2] - a[0]) * (a[3] - a[1]);
    let area_b = (b[2] - b[0]) * (b[3] - b[1]);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}
