//! `brushes/clay_strips.cc`: a square tip that raises what lies below a plane above the surface.

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

/// `Brush::tip_roundness` of Clay Strips (`versioning_280.cc`).
pub const TIP_ROUNDNESS: f32 = 0.18;
/// `Brush::tip_scale_x` (`DNA_brush_types.h`).
pub const TIP_SCALE_X: f32 = 1.0;
/// Assumed `Brush::plane_offset`: the Essentials brush asset is not in the reference repository.
pub const PLANE_OFFSET: f32 = 0.25;

/// `calc_brush_cube_distances()` for one point of the brush plane: 0 inside the square core, 1 at the edge.
pub fn cube_distance(xy: [f32; 2], roundness: f32) -> f32 {
    let local = [xy[0].abs(), xy[1].abs()];
    if local[0].max(local[1]) > 1.0 {
        return 1.0;
    }
    let hardness = 1.0 - roundness;
    let excess = [(local[0] - hardness).max(0.0), (local[1] - hardness).max(0.0)];
    if excess[0].max(excess[1]) == 0.0 {
        return 0.0;
    }
    let rcp = if roundness != 0.0 { 1.0 / roundness } else { 0.0 };
    ((excess[0] * excess[0] + excess[1] * excess[1]).sqrt() * rcp).min(1.0)
}

/// `apply_z_axis_factors()`: parabolic between the plane (z = 0) and one radius below it.
pub fn z_factor(z: f32) -> f32 {
    (z * (1.0 - z)).max(0.0)
}

/// The brush local space of `clay_strips::calc_local_matrix()`.
pub struct Local {
    center: V3,
    axes: [V3; 3],
    radius: f32,
}

impl Local {
    /// `None` without a stroke direction (or with one along the normal): the matrix would be degenerate.
    pub fn new(normal: V3, delta: V3, plane_center: V3, radius: f32, flip: bool) -> Option<Local> {
        let x = cross(normal, delta);
        if dot(x, x) <= 1e-12 || radius <= 0.0 {
            return None;
        }
        let y = cross(normal, x);
        let z = if flip { normal } else { scale(normal, -1.0) };
        Some(Local { center: plane_center, axes: [normalized(x), normalized(y), normalized(z)], radius })
    }

    pub fn coords(&self, p: V3) -> V3 {
        let d = sub(p, self.center);
        let c = |k: usize| dot(d, self.axes[k]) / self.radius;
        [c(0), c(1) / TIP_SCALE_X, c(2)]
    }
}

/// One dab on `verts`. `normals` holds the vertices within `radius` of `center` with their normals (the area of
/// `calc_brush_plane()`), `delta` is the stroke direction, `alpha` is `Brush::alpha`.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], area: &[(usize, V3)], center: V3, delta: V3, radius: f32, alpha: f32, invert: bool, falloff: impl Fn(f32) -> f32) {
    if area.is_empty() {
        return;
    }
    let (mut pos, mut normal) = ([0.0f32; 3], [0.0f32; 3]);
    for &(i, n) in area {
        pos = add(pos, verts[i]);
        normal = add(normal, n);
    }
    let normal = normalized(normal);
    let pos = scale(pos, 1.0 / area.len() as f32);
    /* The plane passes through the area centre, projected under the cursor, and is displaced by the offset. */
    let on_plane = sub(center, scale(normal, dot(sub(center, pos), normal)));
    let flip = invert;
    let sign = if flip { -1.0 } else { 1.0 };
    let plane_center = add(on_plane, scale(normal, radius * PLANE_OFFSET * sign));
    let Some(local) = Local::new(normal, delta, plane_center, radius, flip) else { return };
    /* `brush_strength()`: alpha squared, times 0.3 to compensate the curve. */
    let offset = scale(normal, alpha * alpha * 0.3 * sign * radius);
    for (i, v) in verts.iter_mut().enumerate() {
        let c = local.coords(*v);
        let mut factor = (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0) * z_factor(c[2]);
        let distance = cube_distance([c[0], c[1]], TIP_ROUNDNESS);
        if factor <= 0.0 || distance >= 1.0 {
            continue;
        }
        factor *= falloff(distance);
        *v = add(*v, scale(offset, factor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_tip() {
        assert_eq!(cube_distance([0.8, 0.8], TIP_ROUNDNESS), 0.0, "the corner of the core is full strength");
        assert_eq!(cube_distance([1.1, 0.0], TIP_ROUNDNESS), 1.0);
        assert!((cube_distance([0.91, 0.0], TIP_ROUNDNESS) - 0.5).abs() < 1e-5);
        assert_eq!(z_factor(0.5), 0.25);
        assert_eq!((z_factor(-0.1), z_factor(1.1)), (0.0, 0.0));
    }

    fn strip(delta: V3, invert: bool) -> Vec<V3> {
        let mut v: Vec<V3> = vec![[0.0; 3], [0.8, 0.8, 0.0], [1.2, 0.0, 0.0]];
        let area = [(0, [0.0, 0.0, 1.0])];
        dab(&mut v, &[], &area, [0.0; 3], delta, 1.0, 1.0, invert, |d| 1.0 - d);
        v
    }

    #[test]
    fn dab_raises_a_square_and_needs_a_direction() {
        let v = strip([1.0, 0.0, 0.0], false);
        let expected = 0.3 * z_factor(PLANE_OFFSET);
        assert!((v[0][2] - expected).abs() < 1e-6 && (v[1][2] - expected).abs() < 1e-6);
        assert_eq!(v[2][2], 0.0, "outside the square");
        assert!((strip([1.0, 0.0, 0.0], true)[0][2] + expected).abs() < 1e-6, "inverted digs");
        assert_eq!(strip([0.0; 3], false)[0][2], 0.0);
    }
}
