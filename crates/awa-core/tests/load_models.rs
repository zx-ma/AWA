use std::path::PathBuf;

use awa_core::pipeline::{ModelPaths, Pipeline};

fn models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models")
}

#[test]
#[ignore]
fn loads_all_three_models() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let dir = models_dir();
    let paths = ModelPaths {
        scrfd: &dir.join("scrfd_10g_bnkps.onnx"),
        arcface: &dir.join("arcface_w600k_r50.onnx"),
        minifas: &dir.join("minifas_v2.onnx"),
    };

    let pipeline = Pipeline::load(&paths).expect(
        "models should     
  load",
    );
    pipeline.log_io();
}
