use std::path::PathBuf;

use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore]
fn detects_face_in_sample() {
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
    let pipeline = Pipeline::load(&paths).expect("models load");

    let img_path = root.join("test_data/face_sample.jpg");
    let img = image::open(&img_path).expect("open image").to_rgb8();
    println!("image size: {}x{}", img.width(), img.height());

    let mut pipe = std::sync::Arc::try_unwrap(pipeline)
        .ok()
        .expect("pipeline has only one ref");

    let faces = detect(&mut pipe.scrfd, &img).expect("detection runs");

    println!("detected {} face(s)", faces.len());
    for (i, f) in faces.iter().enumerate() {
        println!(
            "  face {}: score={:.3} bbox={:?} kps={:?}",
            i, f.score, f.bbox, f.keypoints
        );
    }

    assert!(!faces.is_empty(), "should detect at least one face");
}
