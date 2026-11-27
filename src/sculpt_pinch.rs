//! Pinch brush (`mesh/brushes/pinch.cc`: `calc_translations`, `do_pinch_brush`; strength from `brush_strength()` in
//! sculpt.cc). Vertices move toward the brush centre in the plane spanned by the stroke direction and the surface
//! normal; the component along the brush Y axis (`normal x stroke`'s own normal) is removed.
//! Deviations: the Tube falloff projection and the cube-tip factors are not ported; overlap and pressure are 1.

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

/// `brush_strength()` for `SCULPT_BRUSH_TYPE_PINCH` with overlap and pressure 1: `alpha²`, a quarter of it when inverted
/// (`flip < 0`), negative then.
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    let a = alpha * alpha;
    if invert {
        -0.25 * a
    } else {
        a
    }
}

/// `stroke_xz` of `do_pinch_brush()`: `x = normalize(normal x delta)`, `z = normalize(normal)`. `None` when `delta` is zero
/// (the brush does nothing then). If `delta` is parallel to the normal the x axis is zero, as `math::normalize` gives.
pub fn stroke_xz(plane_normal: V3, delta: V3) -> Option<[V3; 2]> {
    if dot(delta, delta) == 0.0 {
        return None;
    }
    let x = cross(plane_normal, delta);
    let len = dot(x, x).sqrt();
    let x = if len > 0.0 { scale(x, 1.0 / len) } else { [0.0; 3] };
    Some([x, normalized(plane_normal)])
}

/// `calc_translations()`: the vertex to centre displacement projected on the stroke X and surface Z axes.
pub fn translation(position: V3, location: V3, xz: &[V3; 2]) -> V3 {
    let d = sub(location, position);
    add(scale(xz[0], dot(d, xz[0])), scale(xz[1], dot(d, xz[1])))
}

/// One dab: `falloff` maps the distance in brush radii (0..1) to the factor. `bstrength` comes from `brush_strength`.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], plane_normal: V3, center: V3, delta: V3, radius: f32, bstrength: f32, falloff: impl Fn(f32) -> f32) {
    let Some(xz) = stroke_xz(plane_normal, delta) else { return };
    for (i, v) in verts.iter_mut().enumerate() {
        let d = sub(*v, center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let m = (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0);
        let factor = falloff(dist / radius) * m * bstrength;
        if factor == 0.0 {
            continue;
        }
        *v = add(*v, scale(translation(*v, center, &xz), factor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: V3 = [0.0, 0.0, 1.0];
    const DELTA: V3 = [1.0, 0.0, 0.0];

    #[test]
    fn strength_is_alpha_squared_and_a_quarter_negative_when_inverted() {
        assert_eq!(brush_strength(0.5, false), 0.25);
        assert_eq!(brush_strength(0.5, true), -0.0625);
    }

    #[test]
    fn zero_delta_does_nothing() {
        let mut v = vec![[0.1, 0.0, 0.0]];
        dab(&mut v, &[], N, [0.0; 3], [0.0; 3], 1.0, 1.0, |_| 1.0);
        assert_eq!(v[0], [0.1, 0.0, 0.0]);
    }

    #[test]
    fn frame_is_perpendicular_to_the_stroke_and_normal() {
        let xz = stroke_xz(N, DELTA).unwrap();
        /* normal x delta = z x x = +y */
        assert_eq!(xz[0], [0.0, 1.0, 0.0]);
        assert_eq!(xz[1], N);
    }

    #[test]
    fn pulls_across_the_stroke_and_along_the_normal_but_not_along_it() {
        let mut v = vec![[0.4, 0.3, 0.2]];
        dab(&mut v, &[], N, [0.0; 3], DELTA, 1.0, 1.0, |_| 1.0);
        /* The displacement to the centre is (-0.4, -0.3, -0.2); the stroke axis x of the world is not in the frame, so
         * only y and z move, fully, and x stays. */
        assert!((v[0][0] - 0.4).abs() < 1e-6);
        assert!(v[0][1].abs() < 1e-6 && v[0][2].abs() < 1e-6);
    }

    #[test]
    fn factor_is_falloff_times_strength_times_unmasked() {
        let mut v = vec![[0.0, 0.5, 0.0], [0.0, 0.5, 0.0], [0.0, 2.0, 0.0]];
        dab(&mut v, &[0.0, 0.5, 0.0], N, [0.0; 3], DELTA, 1.0, 0.5, |d| 1.0 - d);
        assert!((v[0][1] - 0.5 * (1.0 - 0.25)).abs() < 1e-6); /* factor 0.5 * 0.5 * 1.0 = 0.25 of the 0.5 pull */
        assert!((v[1][1] - 0.5).abs() > 1e-6);
        assert_eq!(v[2][1], 2.0); /* Outside the radius. */
    }

    #[test]
    fn inverted_pushes_away() {
        let mut v = vec![[0.0, 0.5, 0.0]];
        dab(&mut v, &[], N, [0.0; 3], DELTA, 1.0, brush_strength(1.0, true), |_| 1.0);
        assert!(v[0][1] > 0.5);
    }
}
