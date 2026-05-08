use std::path::PathBuf;

use awa_core::pipeline::align::align_face;
use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore]
fn aligns_face_from_sample() {
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

    let img_path = root.join("test_data/face_sample.jpg");
    let img = image::open(&img_path).expect("open image").to_rgb8();

    let faces = detect(&mut pipe.scrfd, &img).expect("detect runs");
    let face = faces.first().expect("at least one face");
    println!("face score={:.3} kps={:?}", face.score, face.keypoints);

    let aligned = align_face(&img, &face.keypoints);

    let out_path = root.join("test_data/face_aligned.png");
    aligned.save(&out_path).expect("save aligned");
    println!("saved aligned face to {}", out_path.display());

    assert_eq!(aligned.width(), 112);
    assert_eq!(aligned.height(), 112);
}
