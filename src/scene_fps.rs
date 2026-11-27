//! Animation playback frame rate. Ports `ScreenFrameRateInfo`, `ED_scene_fps_average_accumulate()` and
//! `ED_scene_fps_average_calc()` (editors/scene/scene_fps.cc), `get_fps_string()` and `get_low_fps_color()`
//! (editors/space_view3d/view3d_draw.cc). `ED_scene_fps_average_clear()` is dropping the value.

use crate::theme::{self, Rgba};

/// `UserDef::playback_fps_samples` (DNA_userdef_types.h); 0 or less averages over one second.
pub const PLAYBACK_FPS_SAMPLES: i16 = 8;

/// `.wcol_state.error` (userdef_default_theme.c:233), `TH_REDALERT`.
const REDALERT: Rgba = Rgba::hex(0xe6000099);

/// Ring buffer of frame timestamps; FPS is `(samples - 1) / (newest - oldest)`.
#[derive(Clone, Debug, Default)]
pub struct FrameRateInfo {
    fps_target: f32,
    times_index: usize,
    /// Number of `times` set, never exceeds `times.len()`.
    times_num_set: usize,
    times: Vec<f64>,
}

/// `SceneFPS_State`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsState {
    pub fps_average: f32,
    pub fps_target: f32,
    pub fps_target_is_fractional: bool,
}

impl FrameRateInfo {
    /// Records a frame at `ltime` seconds; resets when the target rate or sample count changes.
    pub fn accumulate(&mut self, fps_target: f64, fps_samples: i16, ltime: f64) {
        let fps_target = fps_target as f32;
        /* Averaging intervals needs +1 timestamps. */
        let samples = if fps_samples > 0 { fps_samples as usize } else { (fps_target.ceil() as usize).max(1) };
        let times_num = samples + 1;
        if self.fps_target != fps_target || self.times.len() != times_num {
            *self = FrameRateInfo { fps_target, times: vec![0.0; times_num], ..Default::default() };
        }
        self.times[self.times_index] = ltime;
        self.times_index = (self.times_index + 1) % times_num;
        self.times_num_set = (self.times_num_set + 1).min(times_num);
    }

    /// `None` until two timestamps define an interval.
    pub fn calc(&self) -> Option<FpsState> {
        if self.times_num_set < 2 {
            return None;
        }
        let len = self.times.len();
        let newest = (self.times_index + len - 1) % len;
        let oldest = if self.times_num_set < len { 0 } else { self.times_index };
        let delta = self.times[newest] - self.times[oldest];
        let fps_average = if delta > 0.0 { ((self.times_num_set - 1) as f64 / delta) as f32 } else { 0.0 };
        /* Two decimal places, as displayed. */
        let hundredths = (self.fps_target as f64 * 100.0).round() / 100.0;
        let fps_target_is_fractional = self.fps_target.round() as f64 != hundredths;
        Some(FpsState { fps_average, fps_target: self.fps_target, fps_target_is_fractional })
    }
}

impl FpsState {
    /// More than half a frame behind the target.
    pub fn is_low(&self) -> bool {
        self.fps_average + 0.5 < self.fps_target
    }

    /// `get_fps_string()`: fractional when the target is, or when under performing.
    pub fn string(&self) -> String {
        if self.fps_target_is_fractional || self.is_low() {
            format!("{:.2} fps", self.fps_average)
        } else {
            format!("{} fps", (self.fps_average + 0.5) as i32)
        }
    }
}

/// `get_low_fps_color()`: `TH_TEXT_HI` blended towards `TH_REDALERT` by the alert alpha.
pub fn low_fps_color() -> Rgba {
    let (text, fac) = (theme::SPACE_VIEW3D_TEXT_HI, REDALERT.a as f32 / 255.0);
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - fac) + b as f32 * fac).round() as u8;
    Rgba { r: mix(text.r, REDALERT.r), g: mix(text.g, REDALERT.g), b: mix(text.b, REDALERT.b), a: 255 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed(target: f64, samples: i16, times: &[f64]) -> FrameRateInfo {
        let mut info = FrameRateInfo::default();
        times.iter().for_each(|&t| info.accumulate(target, samples, t));
        info
    }

    #[test]
    fn needs_two_stamps() {
        assert_eq!(FrameRateInfo::default().calc(), None);
        assert_eq!(fed(24.0, 8, &[1.0]).calc(), None);
        assert_eq!(fed(24.0, 8, &[1.0, 1.5]).calc().unwrap().fps_average, 2.0);
    }

    #[test]
    fn the_ring_wraps_and_drops_the_oldest() {
        /* 2 samples keep 3 stamps: the slow first interval falls out. */
        let info = fed(24.0, 2, &[0.0, 10.0, 10.25, 10.5]);
        assert_eq!(info.calc().unwrap().fps_average, 4.0);
    }

    #[test]
    fn auto_samples_cover_one_second_and_changes_reset() {
        let mut info = fed(23.5, 0, &[0.0, 0.1]);
        assert_eq!(info.times.len(), 25);
        info.accumulate(30.0, 0, 0.2);
        assert_eq!((info.times.len(), info.calc()), (31, None));
        info.accumulate(30.0, 8, 0.3);
        assert_eq!((info.times.len(), info.calc()), (9, None));
    }

    #[test]
    fn string_is_whole_on_target_and_fractional_below_or_for_fractional_rates() {
        let state = |average, target, fractional| FpsState { fps_average: average, fps_target: target, fps_target_is_fractional: fractional };
        assert_eq!(state(23.6, 24.0, false).string(), "24 fps");
        assert_eq!(state(12.5, 24.0, false).string(), "12.50 fps");
        let ntsc = 30.0 / 1.001;
        let info = fed(ntsc, 8, &[0.0, 1.0 / ntsc]);
        assert!(info.calc().unwrap().fps_target_is_fractional);
        assert_eq!(info.calc().unwrap().string(), "29.97 fps");
        assert!(!fed(24.0, 8, &[0.0, 1.0]).calc().unwrap().fps_target_is_fractional);
    }

    #[test]
    fn low_rate_colour_is_a_red_tint() {
        assert_eq!(low_fps_color(), Rgba::hex(0xf06666ff));
    }
}
