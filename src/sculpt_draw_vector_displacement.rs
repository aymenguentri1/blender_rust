//! Vector Displacement brush (editors/sculpt_paint/mesh/brushes/draw_vector_displacement.cc): the brush texture
//! colour of each vertex is the displacement, scaled by the falloff factor and transformed by the displacement
//! matrix. Ports `calc_vertex_displacement_matrix()`, `transform_directions()` and the `scale_translations()`
//! step of `calc_faces()` (:45-70, :154-158). Deviation: the texture colours and falloff factors come from the
//! caller; `brush_local_mat_inv` is given already inverted.

pub type Mat3 = [[f32; 3]; 3];

pub const IDENTITY3: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mul(a: Mat3, b: Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn from_scale(s: [f32; 3]) -> Mat3 {
    [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]]
}

/// `math::transform_direction(mat, v)`.
pub fn transform_direction(m: Mat3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// `calc_vertex_displacement_matrix()`: `mirror * rotation * brush_local_mat_inv * scale` with
/// `scale = safe_divide(1, size^2) * bstrength` (xy flipped for a negative strength). `rotation` is
/// `symm_rot_mat` for a radial symmetry pass, else identity; `mirror_axes` are the X/Y/Z mirror flags.
pub fn displacement_matrix(
    mtex_size: [f32; 3],
    bstrength: f32,
    brush_local_mat_inv: Mat3,
    rotation: Mat3,
    mirror_axes: [bool; 3],
) -> Mat3 {
    let mut scale: [f32; 3] = mtex_size.map(|s| {
        let d = s * s;
        (if d == 0.0 { 0.0 } else { 1.0 / d }) * bstrength
    });
    if bstrength < 0.0 {
        scale[0] = -scale[0];
        scale[1] = -scale[1];
    }
    let mirror = from_scale(mirrors(mirror_axes));
    mul(mul(mul(mirror, rotation), brush_local_mat_inv), from_scale(scale))
}

fn mirrors(axes: [bool; 3]) -> [f32; 3] {
    axes.map(|m| if m { -1.0 } else { 1.0 })
}

/// Translations of the vertices: texture colour `* factor`, then `transform_directions()`.
pub fn translations(colors: &[[f32; 3]], factors: &[f32], displacement_mat: Mat3) -> Vec<[f32; 3]> {
    colors
        .iter()
        .zip(factors)
        .map(|(c, &f)| transform_direction(displacement_mat, c.map(|x| x * f)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(strength: f32) -> Mat3 {
        displacement_matrix([1.0; 3], strength, IDENTITY3, IDENTITY3, [false; 3])
    }

    #[test]
    fn zero_sample_gives_zero_offset() {
        let t = translations(&[[0.0; 3]], &[1.0], mat(1.0));
        assert_eq!(t, vec![[0.0; 3]]);
    }

    #[test]
    fn offset_scales_linearly_with_strength_and_falloff() {
        let c = [[1.0, 2.0, 3.0]];
        assert_eq!(translations(&c, &[1.0], mat(1.0)), vec![[1.0, 2.0, 3.0]]);
        assert_eq!(translations(&c, &[0.5], mat(2.0)), vec![[1.0, 2.0, 3.0]]);
        assert_eq!(translations(&c, &[0.0], mat(1.0)), vec![[0.0; 3]]);
    }

    #[test]
    fn size_negative_strength_and_mirror() {
        let m = displacement_matrix([2.0, 2.0, 0.0], 1.0, IDENTITY3, IDENTITY3, [false; 3]);
        assert_eq!(m[0][0], 0.25);
        assert_eq!(m[2][2], 0.0);
        let n = mat(-1.0);
        assert_eq!((n[0][0], n[1][1], n[2][2]), (1.0, 1.0, -1.0));
        let x = displacement_matrix([1.0; 3], 1.0, IDENTITY3, IDENTITY3, [true, false, false]);
        assert_eq!(transform_direction(x, [1.0, 1.0, 1.0]), [-1.0, 1.0, 1.0]);
    }
}
