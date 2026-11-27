//! Ruler measurement (editors/space_view3d/view3d_gizmo_ruler.cc): `ruler_item_as_string()` (:192-219) with the
//! `USER_UNIT_NONE` path. A two-point ruler has `co[0]` and `co[2]` as its ends, an angle ruler (`RULERITEM_USE_ANGLE`)
//! measures at the middle point `co[1]`. Deviation: unit systems (`BKE_unit_value_as_string*`) are not ported.

type V3 = [f32; 3];

/// `len_v3v3(co[0], co[2])`.
pub fn ruler_length(co: &[V3; 3]) -> f32 {
    (0..3).map(|i| (co[0][i] - co[2][i]).powi(2)).sum::<f32>().sqrt()
}

/// `angle_v3v3v3(co[0], co[1], co[2])` in radians: the angle at `co[1]`.
pub fn ruler_angle(co: &[V3; 3]) -> f32 {
    let a: V3 = std::array::from_fn(|i| co[0][i] - co[1][i]);
    let b: V3 = std::array::from_fn(|i| co[2][i] - co[1][i]);
    let (la, lb) = (dot(a, a).sqrt(), dot(b, b).sqrt());
    if la == 0.0 || lb == 0.0 {
        return std::f32::consts::FRAC_PI_2;
    }
    (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos()
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The label text without units: `"%.*f"` of the length, or of the angle in degrees followed by `°`.
pub fn ruler_label(co: &[V3; 3], use_angle: bool, prec: usize) -> String {
    if use_angle {
        format!("{:.*}\u{b0}", prec, ruler_angle(co).to_degrees())
    } else {
        format!("{:.*}", prec, ruler_length(co))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_of_two_points_uses_the_end_points() {
        let co = [[0.0; 3], [100.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        assert_eq!(ruler_length(&co), 5.0);
        assert_eq!(ruler_label(&co, false, 2), "5.00");
    }

    #[test]
    fn right_angle_ruler_measures_ninety_degrees_at_the_middle_point() {
        let co = [[1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0]];
        assert!((ruler_angle(&co) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(ruler_label(&co, true, 2), "90.00\u{b0}");
    }

    #[test]
    fn straight_line_is_180_and_precision_is_applied() {
        let co = [[-1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0]];
        assert_eq!(ruler_label(&co, true, 0), "180\u{b0}");
        assert_eq!(ruler_label(&[[0.0; 3], [0.0; 3], [1.0, 0.0, 0.0]], false, 3), "1.000");
    }
}
