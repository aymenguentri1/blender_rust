//! Text drawing with the Inter UI font.
//!
//! Ported from Blender:
//!  - `UI_DEFAULT_TEXT_POINTS` (11) and `BLF_size(font, points * dpi_fac)`
//!    (editors/interface/interface_style.cc:80, 524, 591-595)
//!  - the UI font is `release/datafiles/fonts/Inter.woff2` (`assets/Inter.ttf` here is the same font
//!    converted to TrueType, default instance of the variable font)
//!
//! The font is read at run time from `<project>/assets/Inter.ttf`; when the file is missing no text
//! is drawn (widths are 0) and the rest of the UI still works.
//!
//! Glyphs are rasterised with `fontdue` instead of FreeType (`blf_font.cc`, `blf_glyph.cc`), without
//! kerning, hinting or font fallback, and blended in sRGB 8 bit like `widget_draw::draw_round_box()`.
//! Deviation: the horizontal padding of a menu label is `6 * UI_SCALE_FAC`, not read from the
//! `uiBut` layout code.

use std::sync::OnceLock;

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// `UI_DEFAULT_TEXT_POINTS`.
pub const UI_DEFAULT_TEXT_POINTS: f32 = 11.0;

const FONT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Inter.ttf");

fn font() -> Option<&'static fontdue::Font> {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        let bytes = std::fs::read(FONT_PATH).ok()?;
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
    })
    .as_ref()
}

/// Pixel size of the widget font: `points * dpi_fac`.
fn px_size(user: &UserDef) -> f32 {
    UI_DEFAULT_TEXT_POINTS * user.scale_factor
}

/// Width of `s` in pixels (sum of the glyph advances); 0 without the font file.
pub fn string_width(user: &UserDef, s: &str) -> i32 {
    let Some(f) = font() else { return 0 };
    let px = px_size(user);
    s.chars().map(|c| f.metrics(c, px).advance_width).sum::<f32>().round() as i32
}

/// Horizontal padding on each side of a menu label.
fn label_pad(user: &UserDef) -> i32 {
    (6.0 * user.scale_factor) as i32
}

/// Width of a menu label button: text plus padding on both sides.
pub fn label_width(user: &UserDef, s: &str) -> i32 {
    string_width(user, s) + 2 * label_pad(user)
}

/// Draws `s` with its pen starting at `x` on the baseline `baseline` (window pixels, y up).
pub fn draw_text(buf: &mut [u32], width: usize, height: usize, user: &UserDef, x: i32, baseline: i32, s: &str, color: Rgba) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let Some(f) = font() else { return };
    let px = px_size(user);
    let mut pen = x as f32;
    for c in s.chars() {
        let (m, bitmap) = f.rasterize(c, px);
        let gx = pen.round() as i32 + m.xmin;
        for r in 0..m.height {
            let y = baseline + m.ymin + (m.height - 1 - r) as i32;
            if y < 0 || y as usize >= height {
                continue;
            }
            let row = (height - 1 - y as usize) * width;
            for col in 0..m.width {
                let cov = bitmap[r * m.width + col];
                let bx = gx + col as i32;
                if cov == 0 || bx < 0 || bx as usize >= width {
                    continue;
                }
                let alpha = cov as f32 / 255.0 * color.a as f32 / 255.0;
                let px_ref = &mut buf[row + bx as usize];
                *px_ref = blend_px(*px_ref, color, alpha);
            }
        }
        pen += m.advance_width;
    }
}

/// Draws `s` as the label of a button covering `rect` (window pixels, y up): left padded and
/// vertically centred on the font's ascent and descent.
pub fn draw_label(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, s: &str, color: Rgba) {
    draw_label_pad(buf, width, height, user, rect, s, color, label_pad(user));
}

/// Like `draw_label()` with `pad` pixels between the left edge of `rect` and the text.
pub fn draw_label_pad(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, s: &str, color: Rgba, pad: i32) {
    let Some(f) = font() else { return };
    let Some(lm) = f.horizontal_line_metrics(px_size(user)) else { return };
    let rect_h = (rect.size_y() + 1) as f32;
    let baseline = rect.ymin as f32 + (rect_h - (lm.ascent - lm.descent)) / 2.0 - lm.descent;
    draw_text(buf, width, height, user, rect.xmin + pad, baseline.round() as i32, s, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::WCOL_PULLDOWN_TEXT;

    #[test]
    fn width_grows_with_text() {
        if font().is_none() {
            return;
        }
        let user = UserDef::from_window_scale(1.0);
        assert!(string_width(&user, "Object") > string_width(&user, "Add"));
        assert_eq!(label_width(&user, "Add"), string_width(&user, "Add") + 12);
    }

    /* A label drawn into a button rect lightens some pixels of the dark background. */
    #[test]
    fn label_draws_pixels() {
        if font().is_none() {
            return;
        }
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (80usize, 26usize);
        let bg = 0x0030_3030u32;
        let mut buf = vec![bg; w * h];
        let rect = Rcti { xmin: 0, xmax: 79, ymin: 3, ymax: 22 };
        draw_label(&mut buf, w, h, &user, rect, "View", WCOL_PULLDOWN_TEXT);
        assert!(buf.iter().any(|&p| p != bg));
    }
}
