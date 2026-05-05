// umeyama alignment
use image::RgbImage;

const ARCFACE_DST: [[f32; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

pub const ALIGN_SIZE: u32 = 112;

#[derive(Debug, Clone, Copy)]
pub struct SimilarityTransform {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub ty: f32,
}

pub fn align_face(img: &RgbImage, keypoints: &[[f32; 2]; 5]) -> RgbImage {
    let t = estimate_similarity(keypoints, &ARCFACE_DST);
    warp(img, &t)
}

fn estimate_similarity(src: &[[f32; 2]; 5], dst: &[[f32; 2]; 5]) -> SimilarityTransform {
    let (mut sx_m, mut sy_m, mut dx_m, mut dy_m) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..5 {
        sx_m += src[i][0] as f64;
        sy_m += src[i][1] as f64;
        dx_m += dst[i][0] as f64;
        dy_m += dst[i][1] as f64;
    }
    sx_m /= 5.0;
    sy_m /= 5.0;
    dx_m /= 5.0;
    dy_m /= 5.0;

    let (mut num_a, mut num_b, mut den) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..5 {
        let sx = src[i][0] as f64 - sx_m;
        let sy = src[i][1] as f64 - sy_m;
        let dx = dst[i][0] as f64 - dx_m;
        let dy = dst[i][1] as f64 - dy_m;
        num_a += dx * sx + dy * sy;
        num_b += dy * sx - dx * sy;
        den += sx * sx + sy * sy;
    }

    let a = num_a / den;
    let b = num_b / den;
    let tx = dx_m - a * sx_m + b * sy_m;
    let ty = dy_m - b * sx_m - a * sy_m;

    SimilarityTransform {
        a: a as f32,
        b: b as f32,
        tx: tx as f32,
        ty: ty as f32,
    }
}

fn warp(img: &RgbImage, t: &SimilarityTransform) -> RgbImage {
    let det = t.a * t.a + t.b * t.b;
    let inv_det = 1.0 / det;
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut out = RgbImage::new(ALIGN_SIZE, ALIGN_SIZE);

    for dy in 0..ALIGN_SIZE {
        for dx in 0..ALIGN_SIZE {
            let dxc = dx as f32 - t.tx;
            let dyc = dy as f32 - t.ty;
            let sx = (t.a * dxc + t.b * dyc) * inv_det;
            let sy = (-t.b * dxc + t.a * dyc) * inv_det;

            let x0 = sx.floor() as i32;
            let y0 = sy.floor() as i32;
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;

            let mut rgb = [0.0f32; 3];
            for (oi, oj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let nx = x0 + oi;
                let ny = y0 + oj;
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let p = img.get_pixel(nx as u32, ny as u32);
                let wx = if oi == 0 { 1.0 - fx } else { fx };
                let wy = if oj == 0 { 1.0 - fy } else { fy };
                let wt = wx * wy;
                rgb[0] += p[0] as f32 * wt;
                rgb[1] += p[1] as f32 * wt;
                rgb[2] += p[2] as f32 * wt;
            }
            out.put_pixel(
                dx,
                dy,
                image::Rgb([
                    rgb[0].clamp(0.0, 255.0) as u8,
                    rgb[1].clamp(0.0, 255.0) as u8,
                    rgb[2].clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    out
}
