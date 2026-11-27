//! Fly navigation maths of `view3d.fly` (editors/space_view3d/view3d_navigate_fly.cc): the wheel speed
//! steps (`FLY_MODAL_ACCELERATE` / `FLY_MODAL_DECELERATE`, :588-630), the mouse dead zone and curve of
//! `flyApply()` (:828-870) and the normal-operation velocity with directional lag (:900-1050).
//! Deviation: only the translation path is modelled (no pan, free-look, axis locks or camera object);
//! time comes from the caller instead of `BLI_time_now_seconds()`.

/// `FLY_SMOOTH_FAC` (view3d_navigate_fly.cc:799).
const FLY_SMOOTH_FAC: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyState {
    /// `FlyInfo::speed`.
    pub speed: f32,
    /// `FlyInfo::axis`: view axis to move along, `-1` is paused.
    pub axis: i32,
    pub use_precision: bool,
    /// `FlyInfo::grid` (`v3d->grid`).
    pub grid: f32,
    /// `FlyInfo::dvec_prev`.
    pub dvec_prev: [f32; 3],
}

impl FlyState {
    /// `flyInit()` defaults (:368-372): speed 0, moving along the view axis `-Z` is set later (`axis = -1`
    /// starts paused until a key or the wheel picks an axis).
    pub fn new(grid: f32) -> Self {
        Self { speed: 0.0, axis: -1, use_precision: false, grid, dvec_prev: [0.0; 3] }
    }

    /// `time_wheel = 1 + (10 - 20 * min(dt, 0.5))` with `dt` the seconds since the last wheel step.
    fn wheel_step(&self, dt: f32) -> f32 {
        (1.0 + (10.0 - 20.0 * dt.min(0.5))) * self.grid * if self.use_precision { 0.1 } else { 1.0 }
    }

    /// `FLY_MODAL_ACCELERATE`: a reversed speed stops first, else it grows by the wheel step.
    pub fn accelerate(&mut self, dt_wheel: f32) {
        if self.axis == -1 {
            self.axis = 2;
            self.speed = self.speed.abs();
        }
        if self.speed < 0.0 {
            self.speed = 0.0;
        } else {
            self.speed += self.wheel_step(dt_wheel);
        }
    }

    /// `FLY_MODAL_DECELERATE`: mirrored, speed goes negative (flying backwards).
    pub fn decelerate(&mut self, dt_wheel: f32) {
        if self.axis == -1 {
            self.axis = 2;
            self.speed = -self.speed.abs();
        }
        if self.speed > 0.0 {
            self.speed = 0.0;
        } else {
            self.speed -= self.wheel_step(dt_wheel);
        }
    }

    /// One redraw of the normal-operation branch: `mat` is `viewinv` as a 3x3 (`mat[axis]` is the view
    /// axis in world space), `time_redraw` the seconds since the last redraw. Returns the offset delta
    /// added to `rv3d->ofs`.
    pub fn step(&mut self, mat: [[f32; 3]; 3], time_redraw: f32) -> [f32; 3] {
        if self.use_precision {
            self.speed *= 1.0 - time_redraw.min(0.05);
        }
        let mut tmp = [0.0f32; 3];
        if self.axis >= 0 {
            tmp = mat[self.axis as usize];
        }
        for c in &mut tmp {
            *c *= self.speed * time_redraw * 0.25;
        }
        let t = 1.0 / (1.0 + time_redraw * FLY_SMOOTH_FAC);
        let dvec: [f32; 3] = std::array::from_fn(|i| (1.0 - t) * tmp[i] + t * self.dvec_prev[i]);
        self.dvec_prev = dvec;
        dvec
    }
}

/// Mouse offset of `flyApply()` after the safe-area margin (`viewport / 20`) and the squared curve.
/// `mval` and `center` are region pixels, `viewport` the region size.
pub fn mouse_offset(mval: [i32; 2], center: [i32; 2], viewport: [f32; 2]) -> [f32; 2] {
    std::array::from_fn(|i| {
        let margin = viewport[i] / 20.0;
        let mut m = (mval[i] - center[i]) as f32;
        if m > margin {
            m -= margin;
        } else if m < -margin {
            m += margin;
        } else {
            m = 0.0;
        }
        if m != 0.0 {
            m /= viewport[i] - margin * 2.0;
            m *= m.abs();
        }
        m
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn speed_ramps_with_wheel_steps_faster_for_quick_wheeling() {
        let mut s = FlyState::new(1.0);
        s.accelerate(0.5);
        assert_eq!((s.axis, s.speed), (2, 1.0));
        s.accelerate(0.0);
        assert_eq!(s.speed, 12.0);
        s.use_precision = true;
        s.accelerate(0.0);
        assert!((s.speed - 13.1).abs() < 1e-5);
    }

    #[test]
    fn decelerate_stops_forward_motion_then_reverses() {
        let mut s = FlyState::new(1.0);
        s.accelerate(0.5);
        s.decelerate(0.5);
        assert_eq!(s.speed, 0.0);
        s.decelerate(0.5);
        assert_eq!(s.speed, -1.0);
    }

    #[test]
    fn zero_input_leaves_position() {
        let mut s = FlyState::new(1.0);
        assert_eq!(s.step(ID, 0.016), [0.0; 3]);
        s.axis = 2;
        assert_eq!(s.step(ID, 0.016), [0.0; 3]);
    }

    #[test]
    fn forward_key_moves_along_view_axis_with_lag() {
        let mut s = FlyState::new(1.0);
        s.accelerate(0.5);
        let mat = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];
        let d = s.step(mat, 0.1);
        let t = 1.0 / (1.0 + 0.1 * FLY_SMOOTH_FAC);
        assert!((d[1] + (1.0 - t) * 0.025).abs() < 1e-6);
        assert!(d[0] == 0.0 && d[2] == 0.0);
        assert_eq!(s.dvec_prev, d);
    }

    #[test]
    fn mouse_offset_has_dead_zone_and_square_curve() {
        let o = mouse_offset([500 + 40, 300], [500, 300], [1000.0, 600.0]);
        assert_eq!(o, [0.0, 0.0]);
        let o = mouse_offset([500 + 50 + 450, 300 - 30 - 270], [500, 300], [1000.0, 600.0]);
        assert!((o[0] - 0.25).abs() < 1e-6 || (o[0] - 0.25).abs() < 1e-3, "{o:?}");
        assert!(o[1] < 0.0);
    }
}
