//! The splash picture of the splash popup (`wm_block_splash_image`, wm_splash_screen.cc): the image button is drawn
//! with the artwork scaled to `splash_width x splash_height`. `release/datafiles/splash.png` is a Git LFS object that
//! cannot be fetched here, so `assets/splash.rgb` holds the 5.4 artwork (495 x 247, as shown at UI scale 1) without
//! the version text, which `wm_block_splash_add_label` draws on top. Format: width and height as little endian u32,
//! then `width * height` RGB triplets, top row first.

use crate::area::Rcti;

static DATA: &[u8] = include_bytes!("../assets/splash.rgb");

/// The size of the stored artwork in pixels.
pub fn size() -> [i32; 2] {
    let w = u32::from_le_bytes(DATA[0..4].try_into().unwrap());
    let h = u32::from_le_bytes(DATA[4..8].try_into().unwrap());
    [w as i32, h as i32]
}

/// The colour (0x00RRGGBB) of the artwork at the normalised position `(u, v)`, `v` 0 at the top, bilinear filtered.
pub fn sample(u: f32, v: f32) -> u32 {
    let [w, h] = size();
    let x = (u * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = (v * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let px = |xx: i32, yy: i32, c: usize| DATA[8 + (yy * w + xx) as usize * 3 + c] as f32;
    let mut out = 0u32;
    for c in 0..3 {
        let top = px(x0, y0, c) * (1.0 - fx) + px(x1, y0, c) * fx;
        let bot = px(x0, y1, c) * (1.0 - fx) + px(x1, y1, c) * fx;
        out = (out << 8) | (top * (1.0 - fy) + bot * fy).round() as u32;
    }
    out
}

/// Draw the artwork scaled into `rect` (window coordinates, y up) of the `width x height` buffer.
pub fn draw(buf: &mut [u32], width: usize, height: usize, rect: Rcti) {
    let (rw, rh) = (rect.size_x().max(1), rect.size_y().max(1));
    for y in rect.ymin.max(0)..=rect.ymax.min(height as i32 - 1) {
        let row = height - 1 - y as usize;
        let v = (rect.ymax - y) as f32 + 0.5;
        for x in rect.xmin.max(0)..=rect.xmax.min(width as i32 - 1) {
            let u = (x - rect.xmin) as f32 + 0.5;
            buf[row * width + x as usize] = sample(u / rw as f32, v / rh as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_asset_has_the_header_and_all_pixels() {
        let [w, h] = size();
        assert_eq!([w, h], [495, 247]);
        assert_eq!(DATA.len(), 8 + (w * h * 3) as usize);
    }

    #[test]
    fn corners_of_the_scaled_picture_come_from_the_corners_of_the_source() {
        let [w, h] = size();
        let first = |c: usize| DATA[8 + c] as u32;
        let tl = (first(0) << 16) | (first(1) << 8) | first(2);
        assert_eq!(sample(0.0, 0.0), tl);
        let last = 8 + ((h - 1) * w + (w - 1)) as usize * 3;
        let br = ((DATA[last] as u32) << 16) | ((DATA[last + 1] as u32) << 8) | DATA[last + 2] as u32;
        assert_eq!(sample(1.0, 1.0), br);
    }

    #[test]
    fn draw_fills_exactly_the_rect_and_flips_y() {
        let (w, h) = (700usize, 400usize);
        let mut buf = vec![0xdeadbeef_u32; w * h];
        let r = Rcti { xmin: 100, xmax: 594, ymin: 50, ymax: 296 };
        draw(&mut buf, w, h, r);
        assert_eq!(buf[(h - 1 - 296) * w + 100], sample(0.5 / 495.0, 0.5 / 247.0), "top left of the rect");
        assert_eq!(buf[(h - 1 - 50) * w + 594], sample(494.5 / 495.0, 246.5 / 247.0), "bottom right");
        assert_eq!(buf[(h - 1 - 296) * w + 99], 0xdeadbeef, "nothing outside");
        assert_eq!(buf[(h - 1 - 49) * w + 100], 0xdeadbeef);
    }
}
