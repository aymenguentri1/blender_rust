//! Keyframe diamonds of the timeline / dope sheet.
//!
//! Ported from `draw_keyframe_shape()` (editors/animation/keyframes_draw.cc:45-165: per-type size factor,
//! fill colour by type and selection, black `keyborder` outline, alpha) and `draw_keyframes()` (editors/space_action/
//! action_draw.cc:311, which draws every key of a channel with that function). Colours are
//! `U_theme_default.common.anim.keyframe*` and `space_action.keyborder*` (userdef_default_theme.c:323-334, 486-487),
//! chosen by `TH_KEYTYPE_*` in resources.cc:488.
//! Not ported: handle-type shapes (circle, square, clipped diamond; only the default `KEYFRAME_HANDLE_FREE`
//! diamond), extreme arrows, key blocks (holds, interpolation), the GPU shader (the pixel size of the shape is
//! taken as the diamond's half diagonal).

use crate::theme::Rgba;
use crate::widget_draw::{blend_px, smoothstep};

/// `eBezTriple_KeyframeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    Keyframe,
    Breakdown,
    MoveHold,
    Extreme,
    Jitter,
    Generated,
}

/// `common.anim.keyframe*` and `space_action.keyborder*`.
pub const KEYBORDER: Rgba = Rgba::hex(0x000000ff);
pub const KEYBORDER_SELECT: Rgba = Rgba::hex(0x000000ff);

/// Size factor `draw_keyframe_shape()` applies for the key type.
pub fn size_factor(t: KeyType) -> f32 {
    match t {
        KeyType::Keyframe => 1.0,
        KeyType::Breakdown => 0.85,
        KeyType::MoveHold => 0.925,
        KeyType::Extreme => 1.2,
        KeyType::Jitter => 0.8,
        KeyType::Generated => 0.75,
    }
}

/// Interior colour (`TH_KEYTYPE_*` / `TH_KEYTYPE_*_SELECT`). `MoveHold` uses `keyframe_moving_hold`.
pub fn fill_color(t: KeyType, sel: bool) -> Rgba {
    let (n, s) = match t {
        KeyType::Keyframe => (0xbfbfbfff, 0xffbe33ff),
        KeyType::Extreme => (0xe8b3ccff, 0xf28080ff),
        KeyType::Breakdown => (0xb3dbe8ff, 0x54bfedff),
        KeyType::Jitter => (0x94e575ff, 0x61c042ff),
        KeyType::MoveHold => (0x808080ff, 0xffaf23ff),
        KeyType::Generated => (0x585858ff, 0xa28962ff),
    };
    Rgba::hex(if sel { s } else { n })
}

/// Outline colour (`TH_KEYBORDER` / `TH_KEYBORDER_SELECT`).
pub fn outline_color(sel: bool) -> Rgba {
    if sel { KEYBORDER_SELECT } else { KEYBORDER }
}

/// Coverage `(fill, outline)` in 0..=1 of the diamond of half diagonal `r` at offset `(dx, dy)` from its
/// centre: the outer 1 px is the outline, the rest the fill, both antialiased over one pixel.
pub fn diamond_coverage(dx: f32, dy: f32, r: f32) -> (f32, f32) {
    let d = dx.abs() + dy.abs();
    let outer = 1.0 - smoothstep(r - 0.5, r + 0.5, d);
    let inner = 1.0 - smoothstep(r - 1.5, r - 0.5, d);
    (inner, (outer - inner).max(0.0))
}

/// Draws one keyframe diamond centred at pixel `(x, y)` (y down) into a `0x00RRGGBB` buffer.
/// `size` is the base size before the key-type factor; `alpha` is the channel alpha (muted/protected greying).
#[allow(clippy::too_many_arguments)]
pub fn draw_keyframe(buf: &mut [u32], width: usize, height: usize, x: f32, y: f32, size: f32, sel: bool, t: KeyType, alpha: f32) {
    let r = size * size_factor(t);
    let fill = fill_color(t, sel);
    let outline = outline_color(sel);
    let x0 = (x - r - 1.0).floor().max(0.0) as usize;
    let x1 = ((x + r + 1.0).ceil().max(0.0) as usize).min(width.saturating_sub(1));
    let y0 = (y - r - 1.0).floor().max(0.0) as usize;
    let y1 = ((y + r + 1.0).ceil().max(0.0) as usize).min(height.saturating_sub(1));
    if width == 0 || height == 0 {
        return;
    }
    for py in y0..=y1 {
        for px in x0..=x1 {
            let (f, o) = diamond_coverage(px as f32 + 0.5 - x, py as f32 + 0.5 - y, r);
            let i = py * width + px;
            if f > 0.0 {
                buf[i] = blend_px(buf[i], fill, f * alpha * fill.a as f32 / 255.0);
            }
            if o > 0.0 {
                buf[i] = blend_px(buf[i], outline, o * alpha * outline.a as f32 / 255.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_factors_match_blender() {
        assert_eq!(size_factor(KeyType::Keyframe), 1.0);
        assert_eq!(size_factor(KeyType::Breakdown), 0.85);
        assert_eq!(size_factor(KeyType::MoveHold), 0.925);
        assert_eq!(size_factor(KeyType::Extreme), 1.2);
        assert_eq!(size_factor(KeyType::Jitter), 0.8);
        assert_eq!(size_factor(KeyType::Generated), 0.75);
    }

    #[test]
    fn theme_colours() {
        assert_eq!(fill_color(KeyType::Keyframe, false), Rgba::hex(0xbfbfbfff));
        assert_eq!(fill_color(KeyType::Keyframe, true), Rgba::hex(0xffbe33ff));
        assert_eq!(fill_color(KeyType::Extreme, true), Rgba::hex(0xf28080ff));
        assert_eq!(fill_color(KeyType::Breakdown, false), Rgba::hex(0xb3dbe8ff));
        assert_eq!(fill_color(KeyType::Jitter, true), Rgba::hex(0x61c042ff));
        assert_eq!(fill_color(KeyType::MoveHold, false), Rgba::hex(0x808080ff));
        assert_eq!(fill_color(KeyType::Generated, true), Rgba::hex(0xa28962ff));
        assert_eq!(outline_color(false), Rgba::hex(0x000000ff));
    }

    #[test]
    fn diamond_shape() {
        let r = 5.0;
        let (f, o) = diamond_coverage(0.0, 0.0, r);
        assert!(f > 0.99 && o < 0.01);
        // On the edge: outline, no fill. Outside: nothing.
        let (f, o) = diamond_coverage(r - 0.5, 0.0, r);
        assert!(f < 0.1 && o > 0.4);
        assert_eq!(diamond_coverage(r, r, r), (0.0, 0.0));
        // Diamond, not square: the corner of the bounding box is empty.
        assert_eq!(diamond_coverage(r * 0.8, r * 0.8, r), (0.0, 0.0));
        // Symmetric.
        assert_eq!(diamond_coverage(2.0, -1.0, r), diamond_coverage(-2.0, 1.0, r));
    }

    #[test]
    fn draws_filled_diamond_with_outline() {
        let (w, h) = (20, 20);
        let mut buf = vec![0x0020_2020u32; w * h];
        draw_keyframe(&mut buf, w, h, 10.0, 10.0, 6.0, true, KeyType::Keyframe, 1.0);
        assert_eq!(buf[10 * w + 10], fill_color(KeyType::Keyframe, true).to_xrgb());
        assert_eq!(buf[0], 0x0020_2020, "outside untouched");
        // Bounding-box corner stays background (diamond), edge pixel is darkened by the black outline.
        assert_eq!(buf[5 * w + 5], 0x0020_2020);
        assert!(buf[10 * w + 4] < 0x0020_2020 || buf[10 * w + 15] < 0x0020_2020);
    }

    #[test]
    fn alpha_fades_and_clips_at_buffer_edge() {
        let (w, h) = (8, 8);
        let mut buf = vec![0u32; w * h];
        draw_keyframe(&mut buf, w, h, 0.0, 0.0, 6.0, false, KeyType::Keyframe, 0.0);
        assert!(buf.iter().all(|&p| p == 0));
        draw_keyframe(&mut buf, w, h, 0.0, 0.0, 6.0, false, KeyType::Extreme, 1.0);
        assert_ne!(buf[0], 0);
        draw_keyframe(&mut buf, 0, 0, 1.0, 1.0, 3.0, false, KeyType::Keyframe, 1.0);
    }
}
