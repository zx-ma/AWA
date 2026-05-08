use std::path::PathBuf;

use awa_core::pipeline::align::align_face;
use awa_core::pipeline::arcface::{cosine_similarity, extract_embedding};
use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore]
fn embeds_face_from_sample() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let root = project_root();
    let models = root.join("models");
    let paths = ModelPaths {
        scrfd: &models.join("scrfd_10g_bnkps.onnx"),
        arcface: &models.join("arcface_w600k_r50.onnx"),
        minifas: &models.join("minifas_v2.onnx"),
    };
    let mut pipe = Pipeline::load(&paths).expect("models load");

    let img = image::open(root.join("test_data/face_sample.jpg"))
        .expect("open image")
        .to_rgb8();

    let faces = detect(&mut pipe.scrfd, &img).expect("detect runs");
    let face = faces.first().expect("at least one face");
    let aligned = align_face(&img, &face.keypoints);

    let emb_a = extract_embedding(&mut pipe.arcface, &aligned).expect("embed runs");
    let emb_b = extract_embedding(&mut pipe.arcface, &aligned).expect("embed runs");

    let norm_sq: f32 = emb_a.iter().map(|v| v * v).sum();
    let norm = norm_sq.sqrt();
    println!("embedding L2 norm: {:.6}", norm);
    assert!((norm - 1.0).abs() < 1e-4, "embedding must be unit vector");

    let self_sim = cosine_similarity(&emb_a, &emb_b);
    println!("self-similarity: {:.6}", self_sim);
    assert!(
        (self_sim - 1.0).abs() < 1e-4,
        "same input must give same embedding"
    );

    let any_nonzero = emb_a.iter().any(|&v| v.abs() > 1e-6);
    let no_nan = emb_a.iter().all(|v| v.is_finite());
    assert!(any_nonzero, "embedding must not be all zeros");
    assert!(no_nan, "embedding must not contain NaN/Inf");

    println!("first 5 dims: {:?}", &emb_a[..5]);
}
