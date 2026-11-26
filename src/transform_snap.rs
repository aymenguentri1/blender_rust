//! Port of `snap_increment_apply()` (editors/transform/transform_snap.cc:1722), the defaults of
//! `transform_snap_grid_init()` (:873) and `initSnapAngleIncrements()` (:1052), and the scale
//! increments of `initResize()` (transform_mode_resize.cc:333).

/// `t->increment[i]` and `t->increment_precision` of one mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Increment {
    pub increment: f32,
    pub precision: f32,
}

impl Increment {
    /// Translate in the 3D view at grid scale 1: `snap_spatial` 1.0, precision 0.1.
    pub const TRANSLATE: Increment = Increment { increment: 1.0, precision: 0.1 };
    /// `initResize()`: 0.1 with precision 0.1.
    pub const RESIZE: Increment = Increment { increment: 0.1, precision: 0.1 };

    /// `initSnapAngleIncrements()` with the scene defaults (5 degrees, precision 1 degree):
    /// `increment_precision = snap_angle_increment_precision / snap_angle_increment`.
    pub fn angle(increment: f32, precision: f32) -> Increment {
        let p = if increment != 0.0 { (precision as f64 / increment as f64) as f32 } else { 1.0 };
        Increment { increment, precision: p }
    }

    /// The scene default: 5 degrees, 1 degree with Shift.
    pub fn rotate_default() -> Increment {
        Increment::angle(5.0f32.to_radians(), 1.0f32.to_radians())
    }
}

/// `snap_increment_apply()` for one axis: with `use_precision` the step is
/// `increment * increment_precision`; a zero step leaves the value alone.
pub fn snap_increment_apply(value: f32, inc: Increment, use_precision: bool) -> f32 {
    let step = if use_precision { inc.increment * inc.precision } else { inc.increment };
    if step != 0.0 { step * (value / step).round() } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_rounds_to_metres_and_tenths_with_shift() {
        assert_eq!(snap_increment_apply(1.4, Increment::TRANSLATE, false), 1.0);
        assert_eq!(snap_increment_apply(1.6, Increment::TRANSLATE, false), 2.0);
        assert_eq!(snap_increment_apply(-1.6, Increment::TRANSLATE, false), -2.0);
        assert!((snap_increment_apply(1.26, Increment::TRANSLATE, true) - 1.3).abs() < 1e-6);
    }

    #[test]
    fn resize_rounds_to_tenths_and_hundredths_with_shift() {
        assert!((snap_increment_apply(1.26, Increment::RESIZE, false) - 1.3).abs() < 1e-6);
        assert!((snap_increment_apply(1.263, Increment::RESIZE, true) - 1.26).abs() < 1e-6);
    }

    #[test]
    fn rotate_rounds_to_5_degrees_and_1_degree_with_shift() {
        let inc = Increment::rotate_default();
        let deg = |v: f32| v.to_radians();
        assert!((snap_increment_apply(deg(12.0), inc, false) - deg(10.0)).abs() < 1e-5);
        assert!((snap_increment_apply(deg(13.0), inc, false) - deg(15.0)).abs() < 1e-5);
        assert!((snap_increment_apply(deg(12.4), inc, true) - deg(1.0) * 12.0).abs() < 1e-5);
        assert!((inc.precision - 0.2).abs() < 1e-6);
    }

    #[test]
    fn zero_step_is_a_no_op() {
        let inc = Increment { increment: 0.0, precision: 0.1 };
        assert_eq!(snap_increment_apply(1.234, inc, false), 1.234);
        assert_eq!(Increment::angle(0.0, 1.0).precision, 1.0);
    }
}
