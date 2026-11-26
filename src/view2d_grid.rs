//! 2D view grid step computation.
//! Ported from source/blender/editors/interface/view2d/view2d_draw.cc
//! (`get_divisor`, `calculate_grid_step`, `calculate_grid_step_fractions`,
//! `get_parallel_lines_draw_steps`).

/// Smallest scale unit displayed: 100 gives 1/100 = 0.01 as the smallest step.
pub const SUBFRAME_RANGE: i32 = 100;

/// Special prime factor decomposition using only 2, 3 and 5; division giving 2 is preferred.
pub fn get_divisor(distance: i32) -> i32 {
    const DIVISORS: [i32; 3] = [2, 3, 5];
    let mut no_rem = [false; 3];
    for (i, &d) in DIVISORS.iter().enumerate() {
        let result = distance / d;
        let has_no_remainder = result * d == distance;
        if has_no_remainder && result == 2 {
            return d;
        }
        no_rem[i] = has_no_remainder;
    }
    for (i, &d) in DIVISORS.iter().enumerate() {
        if no_rem[i] {
            return d;
        }
    }
    distance
}

/// Distance in view units between major lines; lowest value is 1.
pub fn calculate_grid_step(base: i32, pixel_width: f32, view_width: f32, min_distance: f32) -> i32 {
    if view_width == 0.0 {
        return 1;
    }
    let ppu = pixel_width / view_width;
    let mut distance = base.max(1);
    if ppu * distance as f32 > min_distance {
        while distance > 1 {
            let divisor = get_divisor(distance);
            let result = distance / divisor;
            if ppu * (result as f32) < min_distance {
                break;
            }
            distance = result;
        }
    } else {
        while ppu * (distance as f32) < min_distance && distance < (1 << 30) {
            distance *= 2;
        }
    }
    distance
}

/// Like `calculate_grid_step` but can divide into the 0-1 range.
pub fn calculate_grid_step_fractions(
    base: i32,
    pixel_width: f32,
    view_width: f32,
    min_distance: f32,
) -> f32 {
    let distance = calculate_grid_step(base, pixel_width, view_width, min_distance);
    if distance > 1 {
        return distance as f32;
    }
    let sub = calculate_grid_step(
        SUBFRAME_RANGE,
        pixel_width,
        view_width * SUBFRAME_RANGE as f32,
        min_distance,
    );
    sub as f32 / SUBFRAME_RANGE as f32
}

/// First line position and number of lines for parallel lines within `view_bounds` (min, max).
pub fn get_parallel_lines_draw_steps(line_distance: f32, view_bounds: (f32, f32)) -> (f32, u32) {
    if view_bounds.0 >= view_bounds.1 {
        return (0.0, 0);
    }
    let start = (view_bounds.0 / line_distance).ceil() * line_distance;
    if view_bounds.0 <= start && view_bounds.1 >= start {
        let steps = ((view_bounds.1 - start) / line_distance).floor().max(0.0) as u32 + 1;
        (start, steps)
    } else {
        (start, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisor_prefers_two_halves() {
        assert_eq!(get_divisor(4), 2); // 4/2 == 2
        assert_eq!(get_divisor(6), 3); // 6/3 == 2
        assert_eq!(get_divisor(10), 5); // 10/5 == 2
        assert_eq!(get_divisor(9), 3); // first clean divisor
        assert_eq!(get_divisor(7), 7); // prime: full distance
    }

    #[test]
    fn step_zero_width_is_one() {
        assert_eq!(calculate_grid_step(24, 100.0, 0.0, 25.0), 1);
    }

    #[test]
    fn step_shrinks_when_zoomed_in() {
        // 10 px per unit, min 25 px: 24 -> 12 -> 6; 6/3 = 2 gives 20 px < 25, stop
        assert_eq!(calculate_grid_step(24, 1000.0, 100.0, 25.0), 6);
    }

    #[test]
    fn step_grows_when_zoomed_out() {
        // 1 px per unit, min 25: 1,2,4,8,16,32
        assert_eq!(calculate_grid_step(1, 100.0, 100.0, 25.0), 32);
    }

    #[test]
    fn fractions_go_below_one() {
        // 1000 px per unit: whole step stays 1, fractions drop to 0.05 (50 px >= 25)
        let s = calculate_grid_step_fractions(1, 1000.0, 1.0, 25.0);
        assert!(s < 1.0 && s > 0.0);
        assert!((s - 0.05).abs() < 1e-6, "{s}");
    }

    #[test]
    fn parallel_lines() {
        assert_eq!(get_parallel_lines_draw_steps(10.0, (5.0, 35.0)), (10.0, 3));
        assert_eq!(get_parallel_lines_draw_steps(10.0, (10.0, 10.0)), (0.0, 0));
        assert_eq!(get_parallel_lines_draw_steps(10.0, (11.0, 19.0)), (20.0, 0));
    }
}
