//! Keyframe selection for the Graph editor: validation and select callbacks on `BezTriple`s.
//!
//! Ported from `source/blender/editors/animation/keyframes_edit.cc`:
//!  - `handles_visible()` / `keyframe_ok_checks()` (523): key is index 1, handles 0 and 2 only count with
//!    `KEYFRAME_ITER_INCL_HANDLES` and when visible
//!  - `ok_bezier_framerange()` (559), `ok_bezier_valuerange()` (604): strict `>` / `<` on x / y
//!  - `ok_bezier_region()` (615) with `BLI_rctf_isect_pt_v()` (blenlib/intern/rct.cc:134, inclusive bounds)
//!  - `select_bezier_add()` / `_subtract()` / `_invert()` (1532-1585), `ANIM_editkeyframes_select()` (1590)
//!  - `ANIM_fcurve_keyframes_loop()` (63): `curflags` is reset per key and set to the ok result
//! and the per-curve part of `box_select_graphkeys()` (editors/space_graph/graph_select.cc:599).
//! Not ported: lasso / circle regions, exact frame / value tests (`IS_EQF`), NLA and unit mapping, F-curve flags.

pub const BEZT_FLAG_SELECT: u8 = 1 << 0;

pub const KEYFRAME_OK_KEY: u8 = 1 << 0;
pub const KEYFRAME_OK_H1: u8 = 1 << 1;
pub const KEYFRAME_OK_H2: u8 = 1 << 2;

pub const KEYFRAME_ITER_INCL_HANDLES: u8 = 1 << 0;
pub const KEYFRAME_ITER_HANDLES_DEFAULT_INVISIBLE: u8 = 1 << 3;
pub const KEYFRAME_ITER_HANDLES_INVISIBLE: u8 = 1 << 4;

/// Keyframe: `vec[0]` handle 1, `vec[1]` key, `vec[2]` handle 2, each `[frame, value, z]`; `f1..f3` selection flags.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BezTriple {
    pub vec: [[f32; 3]; 3],
    pub f1: u8,
    pub f2: u8,
    pub f3: u8,
}

impl BezTriple {
    /// `BEZT_ISSEL_ANY`.
    pub fn is_sel_any(&self) -> bool {
        (self.f1 | self.f2 | self.f3) & BEZT_FLAG_SELECT != 0
    }
    pub fn sel_all(&mut self) {
        self.f1 |= BEZT_FLAG_SELECT;
        self.f2 |= BEZT_FLAG_SELECT;
        self.f3 |= BEZT_FLAG_SELECT;
    }
    pub fn desel_all(&mut self) {
        self.f1 &= !BEZT_FLAG_SELECT;
        self.f2 &= !BEZT_FLAG_SELECT;
        self.f3 &= !BEZT_FLAG_SELECT;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rctf {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}

impl Rctf {
    /// `BLI_rctf_isect_pt_v()`: the border counts as inside.
    pub fn isect_pt_v(&self, xy: [f32; 2]) -> bool {
        !(xy[0] < self.xmin || xy[0] > self.xmax || xy[1] < self.ymin || xy[1] > self.ymax)
    }
}

/// The parts of `KeyframeEditData` these callbacks use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyframeEditData {
    pub iterflags: u8,
    pub curflags: u8,
    pub f1: f32,
    pub f2: f32,
    /// `ked->data` for `BEZT_OK_REGION`: `None` accepts nothing.
    pub rect: Option<Rctf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validate {
    FrameRange,
    ValueRange,
    Region,
}

/// `eEditKeyframes_Select`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    Replace,
    Add,
    Subtract,
    Invert,
}

fn handles_visible(ked: &KeyframeEditData, bezt: &BezTriple) -> bool {
    if ked.iterflags & KEYFRAME_ITER_HANDLES_INVISIBLE != 0 {
        return false;
    }
    if ked.iterflags & KEYFRAME_ITER_HANDLES_DEFAULT_INVISIBLE != 0 {
        bezt.is_sel_any()
    } else {
        true
    }
}

fn keyframe_ok_checks(ked: &KeyframeEditData, bezt: &BezTriple, check: impl Fn(usize) -> bool) -> u8 {
    let mut ok = 0;
    if check(1) {
        ok |= KEYFRAME_OK_KEY;
    }
    if ked.iterflags & KEYFRAME_ITER_INCL_HANDLES != 0 && handles_visible(ked, bezt) {
        if check(0) {
            ok |= KEYFRAME_OK_H1;
        }
        if check(2) {
            ok |= KEYFRAME_OK_H2;
        }
    }
    ok
}

/// `ANIM_editkeyframes_ok()` for the frame range, value range and region modes: `KEYFRAME_OK_*` bits.
pub fn keyframe_ok(mode: Validate, ked: &KeyframeEditData, bezt: &BezTriple) -> u8 {
    match mode {
        Validate::FrameRange => keyframe_ok_checks(ked, bezt, |i| bezt.vec[i][0] > ked.f1 && bezt.vec[i][0] < ked.f2),
        Validate::ValueRange => keyframe_ok_checks(ked, bezt, |i| bezt.vec[i][1] > ked.f1 && bezt.vec[i][1] < ked.f2),
        Validate::Region => match ked.rect {
            None => 0,
            Some(r) => keyframe_ok_checks(ked, bezt, |i| r.isect_pt_v([bezt.vec[i][0], bezt.vec[i][1]])),
        },
    }
}

/// `ANIM_editkeyframes_select()`; `Replace` acts as `Add` (the caller clears first).
pub fn keyframe_select(mode: SelectMode, ked: &KeyframeEditData, bezt: &mut BezTriple) {
    match mode {
        SelectMode::Add | SelectMode::Replace => {
            if ked.iterflags & KEYFRAME_ITER_INCL_HANDLES != 0 && handles_visible(ked, bezt) {
                if ked.curflags & KEYFRAME_OK_KEY != 0 {
                    bezt.f2 |= BEZT_FLAG_SELECT;
                }
                if ked.curflags & KEYFRAME_OK_H1 != 0 {
                    bezt.f1 |= BEZT_FLAG_SELECT;
                }
                if ked.curflags & KEYFRAME_OK_H2 != 0 {
                    bezt.f3 |= BEZT_FLAG_SELECT;
                }
            } else {
                bezt.sel_all();
            }
        }
        SelectMode::Subtract => {
            if ked.iterflags & KEYFRAME_ITER_INCL_HANDLES != 0 && handles_visible(ked, bezt) {
                if ked.curflags & KEYFRAME_OK_KEY != 0 {
                    bezt.f2 &= !BEZT_FLAG_SELECT;
                }
                if ked.curflags & KEYFRAME_OK_H1 != 0 {
                    bezt.f1 &= !BEZT_FLAG_SELECT;
                }
                if ked.curflags & KEYFRAME_OK_H2 != 0 {
                    bezt.f3 &= !BEZT_FLAG_SELECT;
                }
            } else {
                bezt.desel_all();
            }
        }
        SelectMode::Invert => {
            bezt.f2 ^= BEZT_FLAG_SELECT;
            if bezt.f2 & BEZT_FLAG_SELECT != 0 {
                bezt.f1 |= BEZT_FLAG_SELECT;
                bezt.f3 |= BEZT_FLAG_SELECT;
            } else {
                bezt.f1 &= !BEZT_FLAG_SELECT;
                bezt.f3 &= !BEZT_FLAG_SELECT;
            }
        }
    }
}

/// The per-curve step of `box_select_graphkeys()`: sets `ked.f1/f2` (x range, or y range for `ValueRange`) from
/// `rect`, and when some key is hit (`ANIM_fcurve_keyframes_loop` with only the ok callback) selects the hit keys
/// (the loop with ok + select callbacks). Returns whether any key was hit.
pub fn box_select_keys(
    ked: &mut KeyframeEditData,
    keys: &mut [BezTriple],
    rect: Rctf,
    mode: Validate,
    select: SelectMode,
) -> bool {
    ked.rect = Some(rect);
    if mode == Validate::ValueRange {
        ked.f1 = rect.ymin;
        ked.f2 = rect.ymax;
    } else {
        ked.f1 = rect.xmin;
        ked.f2 = rect.xmax;
    }
    ked.curflags = 0;
    if !keys.iter().any(|b| keyframe_ok(mode, ked, b) != 0) {
        return false;
    }
    for b in keys.iter_mut() {
        ked.curflags = 0;
        let ok = keyframe_ok(mode, ked, b);
        if ok != 0 {
            ked.curflags = ok;
            keyframe_select(select, ked, b);
        }
    }
    ked.curflags = 0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(frame: f32, value: f32) -> BezTriple {
        BezTriple { vec: [[frame - 1.0, value, 0.0], [frame, value, 0.0], [frame + 1.0, value, 0.0]], ..Default::default() }
    }
    fn rect(x0: f32, x1: f32, y0: f32, y1: f32) -> Rctf {
        Rctf { xmin: x0, xmax: x1, ymin: y0, ymax: y1 }
    }

    #[test]
    fn region_includes_border_but_frame_range_does_not() {
        let mut ked = KeyframeEditData { rect: Some(rect(0.0, 10.0, 0.0, 5.0)), f1: 0.0, f2: 10.0, ..Default::default() };
        assert_eq!(keyframe_ok(Validate::Region, &ked, &key(10.0, 5.0)), KEYFRAME_OK_KEY);
        assert_eq!(keyframe_ok(Validate::FrameRange, &ked, &key(10.0, 5.0)), 0);
        assert_eq!(keyframe_ok(Validate::FrameRange, &ked, &key(9.0, 5.0)), KEYFRAME_OK_KEY);
        ked.rect = None;
        assert_eq!(keyframe_ok(Validate::Region, &ked, &key(5.0, 1.0)), 0);
    }

    #[test]
    fn handles_only_count_when_included_and_visible() {
        let mut ked = KeyframeEditData { rect: Some(rect(4.5, 5.5, 0.0, 9.0)), ..Default::default() };
        let b = key(5.0, 1.0); /* handles at 4 and 6 lie outside, key inside */
        assert_eq!(keyframe_ok(Validate::Region, &ked, &b), KEYFRAME_OK_KEY);
        ked.iterflags = KEYFRAME_ITER_INCL_HANDLES;
        ked.rect = Some(rect(3.5, 4.5, 0.0, 9.0)); /* only handle 1 inside */
        assert_eq!(keyframe_ok(Validate::Region, &ked, &b), KEYFRAME_OK_H1);
        ked.iterflags |= KEYFRAME_ITER_HANDLES_INVISIBLE;
        assert_eq!(keyframe_ok(Validate::Region, &ked, &b), 0);
        ked.iterflags = KEYFRAME_ITER_INCL_HANDLES | KEYFRAME_ITER_HANDLES_DEFAULT_INVISIBLE;
        assert_eq!(keyframe_ok(Validate::Region, &ked, &b), 0);
        let mut sel = b;
        sel.f2 = BEZT_FLAG_SELECT;
        assert_eq!(keyframe_ok(Validate::Region, &ked, &sel), KEYFRAME_OK_H1);
    }

    #[test]
    fn select_add_subtract_invert() {
        let mut b = key(1.0, 1.0);
        let mut ked = KeyframeEditData::default();
        keyframe_select(SelectMode::Add, &ked, &mut b);
        assert!(b.f1 & 1 == 1 && b.f2 & 1 == 1 && b.f3 & 1 == 1);
        keyframe_select(SelectMode::Subtract, &ked, &mut b);
        assert!(!b.is_sel_any());
        keyframe_select(SelectMode::Invert, &ked, &mut b);
        assert!(b.f1 & 1 == 1 && b.f2 & 1 == 1 && b.f3 & 1 == 1);
        keyframe_select(SelectMode::Invert, &ked, &mut b);
        assert!(!b.is_sel_any());
        /* With handles included only the hit parts change. */
        ked.iterflags = KEYFRAME_ITER_INCL_HANDLES;
        ked.curflags = KEYFRAME_OK_H2;
        keyframe_select(SelectMode::Add, &ked, &mut b);
        assert_eq!((b.f1, b.f2, b.f3), (0, 0, 1));
        keyframe_select(SelectMode::Subtract, &ked, &mut b);
        assert!(!b.is_sel_any());
    }

    #[test]
    fn box_select_selects_hit_keys_only() {
        let mut keys = [key(1.0, 1.0), key(5.0, 2.0), key(9.0, 9.0)];
        let mut ked = KeyframeEditData::default();
        assert!(box_select_keys(&mut ked, &mut keys, rect(0.0, 6.0, 0.0, 3.0), Validate::Region, SelectMode::Add));
        assert_eq!(keys.map(|k| k.f2 & 1), [1, 1, 0]);
        assert_eq!(keys[0].f1 & 1, 1, "without handles the whole triple is selected");
        assert!(!box_select_keys(&mut ked, &mut keys, rect(20.0, 30.0, 0.0, 3.0), Validate::Region, SelectMode::Add));
        assert!(box_select_keys(&mut ked, &mut keys, rect(0.0, 6.0, 1.5, 3.0), Validate::ValueRange, SelectMode::Subtract));
        assert_eq!(keys.map(|k| k.f2 & 1), [1, 0, 0]);
    }
}
