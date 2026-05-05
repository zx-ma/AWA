use image::RgbImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

use crate::error::{AwaError, AwaResult};

pub const EMBEDDING_DIM: usize = 512;

pub fn extract_embedding(
    session: &mut Session,
    aligned: &RgbImage,
) -> AwaResult<[f32; EMBEDDING_DIM]> {
    assert_eq!(aligned.width(), 112);
    assert_eq!(aligned.height(), 112);

    let mut tensor = Array4::<f32>::zeros((1, 3, 112, 112));
    for (x, y, pixel) in aligned.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        tensor[[0, 0, y, x]] = (pixel[0] as f32 - 127.5) / 127.5;
        tensor[[0, 1, y, x]] = (pixel[1] as f32 - 127.5) / 127.5;
        tensor[[0, 2, y, x]] = (pixel[2] as f32 - 127.5) / 127.5;
    }

    let outputs = session
        .run(ort::inputs![
            TensorRef::from_array_view(&tensor).map_err(|e| AwaError::Inference(e.to_string()))?
        ])
        .map_err(|e| AwaError::Inference(e.to_string()))?;

    let raw = outputs[0]
        .try_extract_array::<f32>()
        .map_err(|e| AwaError::Inference(e.to_string()))?;

    let mut sum_sq = 0.0f32;
    for i in 0..EMBEDDING_DIM {
        let v = raw[[0, i]];
        sum_sq += v * v;
    }
    let norm = sum_sq.sqrt();

    let mut emb = [0.0f32; EMBEDDING_DIM];
    for i in 0..EMBEDDING_DIM {
        emb[i] = raw[[0, i]] / norm;
    }

    Ok(emb)
}

pub fn cosine_similarity(a: &[f32; EMBEDDING_DIM], b: &[f32; EMBEDDING_DIM]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
