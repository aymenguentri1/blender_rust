//! Walk navigation maths of `view3d.walk` (editors/space_view3d/view3d_navigate_walk.cc): the mouse-look pitch
//! limits and speed (`walkApply()` :1062-1250), the WASD velocity (:1250-1330) and the free-fall / jump
//! height (:1052-1060, :1380-1400). Deviation: floor ray casts, teleport, tablet mode and axis locks are not
//! modelled; time comes from the caller. The pitch limits are Blender's (+85 / -80 degrees), not +-90.

/// `WALK_TOP_LIMIT` / `WALK_BOTTOM_LIMIT` (:1066-1067).
pub const WALK_TOP_LIMIT: f32 = 85.0 * std::f32::consts::PI / 180.0;
pub const WALK_BOTTOM_LIMIT: f32 = -80.0 * std::f32::consts::PI / 180.0;
/// `WALK_ROTATE_CONSTANT_FAC`: radians per pixel (:1065).
pub const WALK_ROTATE_CONSTANT_FAC: f32 = 0.15 * std::f32::consts::PI / 180.0;

/// `WALK_BIT_*` direction flags (view3d_navigate_walk.cc, `eWalkDirectionFlag`).
pub const BIT_FORWARD: u8 = 1;
pub const BIT_BACKWARD: u8 = 2;
pub const BIT_LEFT: u8 = 4;
pub const BIT_RIGHT: u8 = 8;

/// Look up/down: `y` pixels of mouse offset scaled by `mouse_speed`; blocked past the limits. `view_m22` is
/// `rv3d->viewmat[2][2]`. Returns the rotation about the view X axis, the argument of `axis_angle_to_quat(-y)`.
pub fn pitch_delta(moffset_y: f32, mouse_speed: f32, view_m22: f32) -> f32 {
    let mut y = moffset_y * WALK_ROTATE_CONSTANT_FAC * mouse_speed;
    let angle = -view_m22.asin();
    if (angle > WALK_TOP_LIMIT && y > 0.0) || (angle < WALK_BOTTOM_LIMIT && y < 0.0) {
        y = 0.0;
    }
    y
}

/// Look left/right: rotation about the world Z axis; the offset is inverted upside down (`upvec[2] < 0`).
pub fn yaw_delta(moffset_x: f32, mouse_speed: f32, up_z: f32) -> f32 {
    let x = if up_z < 0.0 { -moffset_x } else { moffset_x };
    x * WALK_ROTATE_CONSTANT_FAC * mouse_speed
}

/// WASD translation for one redraw: `mat` is `viewinv` as 3x3 (`mat[2]` the view Z axis, `mat[0]` the right
/// axis), `gravity_mode` flattens it to the ground plane. The result is `normalize(dir) * speed * dt`.
pub fn wasd_dvec(mat: [[f32; 3]; 3], dirs: u8, gravity_mode: bool, speed: f32, dt: f32) -> [f32; 3] {
    let mut dvec = [0.0f32; 3];
    if dirs & (BIT_FORWARD | BIT_BACKWARD) != 0 {
        let d = (dirs & BIT_FORWARD != 0) as i32 - (dirs & BIT_BACKWARD != 0) as i32;
        let mut t: [f32; 3] = mat[2].map(|c| c * d as f32);
        if gravity_mode {
            t[2] = 0.0;
        }
        for i in 0..3 {
            dvec[i] += t[i];
        }
    }
    if dirs & (BIT_LEFT | BIT_RIGHT) != 0 {
        let d = (dirs & BIT_LEFT != 0) as i32 - (dirs & BIT_RIGHT != 0) as i32;
        dvec[0] += d as f32 * mat[0][0];
        dvec[1] += d as f32 * mat[0][1];
    }
    let len = (dvec[0] * dvec[0] + dvec[1] * dvec[1] + dvec[2] * dvec[2]).sqrt();
    if len > 0.0 {
        dvec = dvec.map(|c| c / len * speed * dt);
    }
    dvec
}

/// `walk_calc_free_fall_distance()`.
pub fn free_fall_distance(gravity: f32, time: f32) -> f32 {
    gravity * time * time * 0.5
}

/// `walk_calc_velocity_zero_time()`.
pub fn velocity_zero_time(gravity: f32, velocity: f32) -> f32 {
    velocity / gravity
}

/// `z_new` of the falling / jumping branch (grid-scaled units): `origin_z - free_fall + t * speed_jump`.
pub fn fall_z(origin_z: f32, grid: f32, gravity: f32, t: f32, speed_jump: f32) -> f32 {
    origin_z / grid - free_fall_distance(gravity, t) + t * speed_jump
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_follows_yaw() {
        /* View looking along +X: view Z axis (backwards) is -X, so forward is +X (dvec = mat[2] * +1 in Blender's
         * `copy_v3_fl3(0, 0, direction)` is along +Z of viewinv, i.e. the given axis). */
        let mat = [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
        let d = wasd_dvec(mat, BIT_FORWARD, false, 2.0, 0.5);
        assert!((d[0] - 1.0).abs() < 1e-6 && d[1].abs() < 1e-6 && d[2].abs() < 1e-6);
        let b = wasd_dvec(mat, BIT_BACKWARD, false, 2.0, 0.5);
        assert!((b[0] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn gravity_mode_stays_on_ground_plane_and_no_input_is_zero() {
        let mat = [[1.0, 0.0, 0.0], [0.0, 0.6, 0.8], [0.0, -0.8, 0.6]];
        let d = wasd_dvec(mat, BIT_FORWARD, true, 1.0, 1.0);
        assert_eq!(d[2], 0.0);
        assert!((d[1] + 1.0).abs() < 1e-6);
        assert_eq!(wasd_dvec(mat, 0, true, 1.0, 1.0), [0.0; 3]);
        assert_eq!(wasd_dvec(mat, BIT_LEFT | BIT_RIGHT, false, 1.0, 1.0), [0.0; 3]);
    }

    #[test]
    fn pitch_is_blocked_past_the_limits_only_in_that_direction() {
        let m22 = |deg: f32| (-(deg.to_radians())).sin();
        assert!(pitch_delta(10.0, 1.0, m22(0.0)) > 0.0);
        assert_eq!(pitch_delta(10.0, 1.0, m22(86.0)), 0.0);
        assert!(pitch_delta(-10.0, 1.0, m22(86.0)) < 0.0);
        assert_eq!(pitch_delta(-10.0, 1.0, m22(-81.0)), 0.0);
        assert!(pitch_delta(10.0, 1.0, m22(-81.0)) > 0.0);
    }

    #[test]
    fn yaw_inverts_upside_down() {
        assert_eq!(yaw_delta(4.0, 2.0, 1.0), -yaw_delta(4.0, 2.0, -1.0));
        assert!((yaw_delta(1.0, 1.0, 1.0) - WALK_ROTATE_CONSTANT_FAC).abs() < 1e-9);
    }

    #[test]
    fn gravity_pulls_down_and_jump_lifts() {
        assert!((free_fall_distance(9.8, 1.0) - 4.9).abs() < 1e-6);
        assert!(fall_z(10.0, 1.0, 9.8, 0.5, 0.0) < 10.0);
        assert!(fall_z(10.0, 1.0, 9.8, 0.1, 5.0) > 10.0);
        assert!((velocity_zero_time(10.0, 5.0) - 0.5).abs() < 1e-6);
        assert!((fall_z(20.0, 2.0, 0.0, 3.0, 0.0) - 10.0).abs() < 1e-6);
    }
}
