use std::path::PathBuf;
use std::sync::Arc;

use awa_core::camera::rgb::RgbCamera;
use awa_core::pipeline::scrfd::detect;
use awa_core::pipeline::{ModelPaths, Pipeline};
use image::{Rgb, RgbImage};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn draw_rect(img: &mut RgbImage, x1: i32, y1: i32, x2: i32, y2: i32, color: Rgb<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for x in x1..=x2 {
        for dy in [y1, y2] {
            for d in -1..=1 {
                let yy = dy + d;
                if (0..w).contains(&x) && (0..h).contains(&yy) {
                    img.put_pixel(x as u32, yy as u32, color);
                }
            }
        }
    }
    for y in y1..=y2 {
        for dx in [x1, x2] {
            for d in -1..=1 {
                let xx = dx + d;
                if (0..w).contains(&xx) && (0..h).contains(&y) {
                    img.put_pixel(xx as u32, y as u32, color);
                }
            }
        }
    }
}

fn draw_point(img: &mut RgbImage, cx: i32, cy: i32, color: Rgb<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for dy in -3..=3 {
        for dx in -3..=3 {
            if dx * dx + dy * dy <= 9 {
                let (x, y) = (cx + dx, cy + dy);
                if (0..w).contains(&x) && (0..h).contains(&y) {
                    img.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

#[test]
#[ignore]
fn capture_and_detect_from_camera() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let cam = RgbCamera::open("/dev/video0", 1280, 720).expect("open camera");
    let mut img = cam.capture().expect("capture frame");
    println!("captured frame: {}x{}", img.width(), img.height());

    let root = project_root();
    let models = root.join("models");
    let paths = ModelPaths {
        scrfd: &models.join("scrfd_10g_bnkps.onnx"),
        arcface: &models.join("arcface_w600k_r50.onnx"),
        minifas: &models.join("minifas_v2.onnx"),
    };
    let pipeline = Pipeline::load(&paths).expect("models load");
    let mut pipe = Arc::try_unwrap(pipeline).ok().expect("single ref");

    let faces = detect(&mut pipe.scrfd, &img).expect("detect runs");
    println!("detected {} face(s)", faces.len());

    for f in &faces {
        let bbox = f.bbox.map(|v| v as i32);
        draw_rect(
            &mut img,
            bbox[0],
            bbox[1],
            bbox[2],
            bbox[3],
            Rgb([0, 255, 0]),
        );
        for kp in f.keypoints.iter() {
            draw_point(&mut img, kp[0] as i32, kp[1] as i32, Rgb([255, 0, 0]));
        }
        println!("  score={:.3} bbox={:?}", f.score, f.bbox);
    }

    let out_path = root.join("test_data/camera_capture.jpg");
    img.save(&out_path).expect("save image");
    println!("saved annotated frame to {}", out_path.display());

    assert!(
        !faces.is_empty(),
        "should detect at least one face from live camera"
    );
}
