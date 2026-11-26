//! Scene frame state rules: `RenderData::sfra/efra` through `rna_Scene_start_frame_set` / `rna_Scene_end_frame_set`
//! (rna_scene.cc:1082-1103), the limits `MINFRAME`, `MAXFRAME` (DNA_scene_types.h:3065), `rna_Scene_frame_current_set`
//! (rna_scene.cc:1051) and `frame_offset_exec()` (screen_ops.cc:3894, delta -1 / +1 on LEFT / RIGHT_ARROW,
//! blender_default.py:3834). The current frame is clamped with `FRAMENUMBER_MIN_CLAMP` (0, no negative frames);
//! wrapping (`SCER_WRAP_TIMELINE_NAVIGATION`) is off in the startup file and not modelled.

pub const MINFRAME: i32 = 0;
pub const MAXFRAME: i32 = 1_048_574;

/// `Scene::r.sfra` and `r.efra`; the startup scene is 1 to 250.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRange {
    pub start: i32,
    pub end: i32,
}

impl Default for FrameRange {
    fn default() -> Self {
        FrameRange { start: 1, end: 250 }
    }
}

impl FrameRange {
    /// `rna_Scene_start_frame_set`: clamped to `MINFRAME..=MAXFRAME`; a start past the end pushes the end along.
    pub fn set_start(&mut self, value: i32) {
        let value = value.clamp(MINFRAME, MAXFRAME);
        self.start = value;
        if value > self.end {
            self.end = value.min(MAXFRAME);
        }
    }

    /// `rna_Scene_end_frame_set`: clamped to `MINFRAME..=MAXFRAME`; an end before the start pulls the start along.
    pub fn set_end(&mut self, value: i32) {
        let value = value.clamp(MINFRAME, MAXFRAME);
        self.end = value;
        if self.start > value {
            self.start = value.max(MINFRAME);
        }
    }

    /// `screen.frame_jump` (Shift + LEFT / RIGHT_ARROW): the start or the end of the range.
    pub fn jump(&self, to_end: bool) -> i32 {
        if to_end { self.end } else { self.start }
    }
}

/// `rna_Scene_frame_current_set`: no negative frames.
pub fn set_current(value: i32) -> i32 {
    value.max(MINFRAME)
}

/// `frame_offset_exec()` without wrapping: `cfra += delta`, then no negative frames.
pub fn offset(cfra: i32, delta: i32) -> i32 {
    set_current(cfra.saturating_add(delta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_range_is_1_to_250() {
        assert_eq!(FrameRange::default(), FrameRange { start: 1, end: 250 });
    }

    #[test]
    fn start_past_end_pushes_the_end() {
        let mut r = FrameRange::default();
        r.set_start(300);
        assert_eq!(r, FrameRange { start: 300, end: 300 });
        r.set_start(-5);
        assert_eq!(r.start, MINFRAME);
        r.set_start(i32::MAX);
        assert_eq!(r, FrameRange { start: MAXFRAME, end: MAXFRAME });
    }

    #[test]
    fn end_before_start_pulls_the_start() {
        let mut r = FrameRange::default();
        r.set_end(0);
        assert_eq!(r, FrameRange { start: 0, end: 0 });
        r.set_end(-9);
        assert_eq!(r.end, MINFRAME);
        r.set_end(5_000_000);
        assert_eq!(r.end, MAXFRAME);
        assert_eq!(r.start, 0, "a later end leaves the start alone");
    }

    #[test]
    fn frame_offset_clamps_at_zero() {
        assert_eq!(offset(1, 1), 2);
        assert_eq!(offset(1, -1), 0);
        assert_eq!(offset(0, -1), 0);
        assert_eq!(offset(i32::MAX, 1), i32::MAX);
        assert_eq!(set_current(-3), 0);
    }

    #[test]
    fn jump_goes_to_the_range_ends() {
        let mut r = FrameRange::default();
        r.set_start(10);
        r.set_end(20);
        assert_eq!((r.jump(false), r.jump(true)), (10, 20));
    }
}
