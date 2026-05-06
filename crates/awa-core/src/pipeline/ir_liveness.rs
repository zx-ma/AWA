
use image::GrayImage;

#[derive(Debug, Clone, Copy)]
pub struct IrLivenessResult {
    pub mean: f32,
    pub stddev: f32,
    pub max: u8,
    pub bright_ratio: f32,
}

pub fn ir_face_brightness(
    ir: &GrayImage,
    rgb_bbox: [f32; 4],
    rgb_size: (u32, u32),
    ir_size: (u32, u32),
) -> IrLivenessResult {
    let scale_x = ir_size.0 as f32 / rgb_size.0 as f32;
    let scale_y = ir_size.1 as f32 / rgb_size.1 as f32;

    let x1 = (rgb_bbox[0] * scale_x).max(0.0) as u32;
    let y1 = (rgb_bbox[1] * scale_y).max(0.0) as u32;
    let x2 = (rgb_bbox[2] * scale_x).min(ir_size.0 as f32 - 1.0) as u32;
    let y2 = (rgb_bbox[3] * scale_y).min(ir_size.1 as f32 - 1.0) as u32;

    let mut sum = 0u64;
    let mut sum_sq = 0u64;
    let mut max_val = 0u8;
    let mut bright_count = 0u64;
    let mut count = 0u64;

    for y in y1..y2 {
        for x in x1..x2 {
            let p = ir.get_pixel(x, y)[0];
            sum += p as u64;
            sum_sq += (p as u64) * (p as u64);
            if p > max_val {
                max_val = p;
            }
            if p > 100 {
                bright_count += 1;
            }
            count += 1;
        }
    }

    if count == 0 {
        return IrLivenessResult {
            mean: 0.0,
            stddev: 0.0,
            max: 0,
            bright_ratio: 0.0,
        };
    }

    let mean = sum as f32 / count as f32;
    let variance = (sum_sq as f32 / count as f32) - mean * mean;
    let stddev = variance.max(0.0).sqrt();
    let bright_ratio = bright_count as f32 / count as f32;

    IrLivenessResult {
        mean,
        stddev,
        max: max_val,
        bright_ratio,
    }
}
