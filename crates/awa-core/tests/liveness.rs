use std::path::Path;
use std::path::PathBuf;

use awa_core::pipeline::minifas::liveness_score;
use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};
use image::RgbImage;
use ort::session::Session;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn score_for(scrfd: &mut Session, minifas: &mut Session, path: &Path) -> f32 {
    let img: RgbImage = image::open(path)
        .unwrap_or_else(|_| panic!("open {}", path.display()))
        .to_rgb8();
    let faces = detect(scrfd, &img).expect("detect runs");
    let face = faces.first().expect("at least one face");
    liveness_score(minifas, &img, face.bbox).expect("liveness runs")
}

#[test]
#[ignore]
fn liveness_distinguishes_real_from_spoof() {
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

    let real = score_for(
        &mut pipe.scrfd,
        &mut pipe.minifas,
        &root.join("test_data/face_sample.jpg"),
    );
    let spoof = score_for(
        &mut pipe.scrfd,
        &mut pipe.minifas,
        &root.join("test_data/face_spoof.jpg"),
    );

    println!("real liveness:  {:.4}", real);
    println!("spoof liveness: {:.4}", spoof);
    println!("gap:            {:.4}", real - spoof);

    assert!(real.is_finite() && spoof.is_finite());
    assert!(real > spoof, "real face should score higher than spoof");
}
