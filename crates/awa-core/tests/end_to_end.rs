use std::path::PathBuf;
use std::sync::Arc;

use awa_core::camera::{CameraConfig, CameraSet};
use awa_core::pipeline::align::align_face;
use awa_core::pipeline::arcface::extract_embedding;
use awa_core::pipeline::minifas::liveness_score;
use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore]
fn end_to_end_dual_camera_pipeline() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let root = project_root();

    let rgb_path = std::path::Path::new("/dev/video0");
    let ir_path = std::path::Path::new("/dev/video2");
    let cam_cfg = CameraConfig {
        rgb_path,
        rgb_width: 1280,
        rgb_height: 720,
        ir_path: Some(ir_path),
        ir_width: 640,
        ir_height: 360,
    };
    let cameras = CameraSet::open(&cam_cfg).expect("open cameras");
    println!("ir available: {}", cameras.has_ir());

    let frame = cameras.capture().expect("capture both");
    println!("captured rgb: {}x{}", frame.rgb.width(), frame.rgb.height());
    if let Some(ir) = &frame.ir {
        println!("captured ir:  {}x{}", ir.width(), ir.height());
    }

    let models = root.join("models");
    let paths = ModelPaths {
        scrfd: &models.join("scrfd_10g_bnkps.onnx"),
        arcface: &models.join("arcface_w600k_r50.onnx"),
        minifas: &models.join("minifas_v2.onnx"),
    };
    let pipeline = Pipeline::load(&paths).expect("models load");
    let mut pipe = Arc::try_unwrap(pipeline).ok().expect("single ref");

    let faces = detect(&mut pipe.scrfd, &frame.rgb).expect("detect runs");
    println!("detected {} face(s)", faces.len());
    let face = faces.first().expect("at least one face");

    let aligned = align_face(&frame.rgb, &face.keypoints);
    let emb = extract_embedding(&mut pipe.arcface, &aligned).expect("embed runs");
    let live = liveness_score(&mut pipe.minifas, &frame.rgb, face.bbox).expect("liveness runs");

    println!("score:     {:.3}", face.score);
    println!("bbox:      {:?}", face.bbox);
    println!("liveness:  {:.4}", live);
    println!("emb head:  {:?}", &emb[..5]);

    aligned.save(root.join("test_data/e2e_aligned.png")).ok();
    frame.rgb.save(root.join("test_data/e2e_rgb.jpg")).ok();
    if let Some(ir) = &frame.ir {
        ir.save(root.join("test_data/e2e_ir.png")).ok();
    }

    assert!(face.score > 0.5, "face detection confidence too low");
    assert!(live > 0.5, "liveness too low for live capture");
    assert!(emb.iter().any(|v| v.abs() > 1e-6));
}
