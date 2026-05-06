use std::path::PathBuf;

use awa_core::camera::ir::IrCamera;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore]
fn capture_ir_frame() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let cam = IrCamera::open("/dev/video2", 640, 360).expect("open ir camera");
    let img = cam.capture().expect("capture ir frame");
    println!("captured ir frame: {}x{}", img.width(), img.height());

    let pixels: Vec<u8> = img.pixels().map(|p| p[0]).collect();
    let total: u64 = pixels.iter().map(|&v| v as u64).sum();
    let mean = total as f64 / pixels.len() as f64;
    let min = *pixels.iter().min().unwrap();
    let max = *pixels.iter().max().unwrap();
    println!("ir pixel stats: mean={:.1} min={} max={}", mean, min, max);

    let out_path = project_root().join("test_data/ir_capture.png");
    img.save(&out_path).expect("save ir image");
    println!("saved ir frame to {}", out_path.display());

    assert!(
        mean > 5.0 && mean < 250.0,
        "frame should not be uniformly black or white"
    );
    assert!(max - min > 10, "frame should have meaningful variance");
}
