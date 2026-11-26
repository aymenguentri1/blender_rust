//! glTF node transforms: TRS and matrix conversion between glTF (Y up) and Blender (Z up), both directions.
//!
//! Import: `set_convert_functions()` (imp/blender_gltf.py:62-90: `convert_loc/quat/scale/matrix`) and
//! `get_node_trs()` (imp/vnode.py:256: a node `matrix` wins over TRS, defaults are t=0, r=(0,0,0,1), s=1).
//! Export: `__gather_trans_rot_scale()` (exp/nodes.py:489-550: swizzle, `round_if_near` of 2e-6, components equal to
//! their default are omitted) with `__convert_swizzle_*` (exp/nodes.py:601-625) and `round_if_near`
//! (com/gltf2_blender_math.py:163). A child's glTF TRS is relative to its parent, so world = parent world @ local.
//! Not ported: the vnode `rotation_before/after` axis adjustments, bones, `gltf_yup = False` and collection offsets.

/// Blender quaternion `[w, x, y, z]`, glTF quaternion `[x, y, z, w]`.
pub type Trs = ([f32; 3], [f32; 4], [f32; 3]);
/// Row-major 4x4 as `mathutils.Matrix` rows.
pub type Mat = [[f32; 4]; 4];

/// glTF node properties; `None` is an absent property.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GltfNodeTransform {
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
    /// Column-major, as in the glTF file.
    pub matrix: Option<[f32; 16]>,
}

/// `convert_loc`: x, y, z -> x, -z, y, times `u` (Blender units per metre = 1 / `scale_length`).
pub fn loc_to_blender(x: [f32; 3], u: f32) -> [f32; 3] {
    [u * x[0], u * -x[2], u * x[1]]
}

/// `convert_quat`: glTF `[x, y, z, w]` -> Blender `[w, x, -z, y]`.
pub fn quat_to_blender(q: [f32; 4]) -> [f32; 4] {
    [q[3], q[0], -q[2], q[1]]
}

/// `convert_scale`: x, y, z -> x, z, y.
pub fn scale_to_blender(s: [f32; 3]) -> [f32; 3] {
    [s[0], s[2], s[1]]
}

/// `convert_matrix` (column-major glTF array -> Blender rows).
pub fn matrix_to_blender(m: &[f32; 16], u: f32) -> Mat {
    [
        [m[0], -m[8], m[4], m[12] * u],
        [-m[2], m[10], -m[6], -m[14] * u],
        [m[1], -m[9], m[5], m[13] * u],
        [m[3] / u, -m[11] / u, m[7] / u, m[15]],
    ]
}

/// `__convert_swizzle_location` with `gltf_yup`: x, y, z -> x, z, -y.
pub fn loc_to_gltf(l: [f32; 3]) -> [f32; 3] {
    [l[0], l[2], -l[1]]
}

/// `__convert_swizzle_rotation`: Blender `[w, x, y, z]` -> `[w, x, z, -y]`, written to glTF as `[x, y, z, w]`.
pub fn quat_to_gltf(q: [f32; 4]) -> [f32; 4] {
    let s = [q[0], q[1], q[3], -q[2]];
    [s[1], s[2], s[3], s[0]]
}

/// `__convert_swizzle_scale`: x, y, z -> x, z, y.
pub fn scale_to_gltf(s: [f32; 3]) -> [f32; 3] {
    [s[0], s[2], s[1]]
}

/// `get_node_trs()`: the Blender location, quaternion `[w, x, y, z]` and scale of a node. A matrix is returned as a
/// matrix because `Matrix.decompose()` is not ported here.
pub fn node_trs_to_blender(n: &GltfNodeTransform, u: f32) -> Result<Trs, Mat> {
    if let Some(m) = &n.matrix {
        return Err(matrix_to_blender(m, u));
    }
    Ok((
        loc_to_blender(n.translation.unwrap_or([0.0; 3]), u),
        quat_to_blender(n.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0])),
        scale_to_blender(n.scale.unwrap_or([1.0; 3])),
    ))
}

/// `round_if_near()`.
fn round_if_near(v: f32, target: f32) -> f32 {
    if (v - target).abs() > 2.0e-6 { v } else { target }
}

/// Tail of `__gather_trans_rot_scale()`: a Blender local TRS (normalised quaternion) as glTF properties; values near
/// their default are rounded to it and components equal to the default are left out.
pub fn trs_to_gltf(trs: &Trs) -> GltfNodeTransform {
    let (loc, rot, scale) = trs;
    let t = loc_to_gltf(*loc).map(|v| round_if_near(v, 0.0));
    let r = quat_to_gltf(*rot);
    let r = [round_if_near(r[0], 0.0), round_if_near(r[1], 0.0), round_if_near(r[2], 0.0), round_if_near(r[3], 1.0)];
    let s = scale_to_gltf(*scale).map(|v| round_if_near(v, 1.0));
    GltfNodeTransform {
        translation: (t != [0.0; 3]).then_some(t),
        rotation: (r != [0.0, 0.0, 0.0, 1.0]).then_some(r),
        scale: (s != [1.0; 3]).then_some(s),
        matrix: None,
    }
}

/// `Matrix.LocRotScale(loc, quat [w, x, y, z], scale)` = T @ R @ S.
pub fn loc_rot_scale(loc: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Mat {
    let [w, x, y, z] = q;
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let mut m = [[0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j] * s[j];
        }
        m[i][3] = loc[i];
    }
    m
}

/// `a @ b`.
pub fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// The Blender local matrix of a node (matrix path or TRS path).
pub fn local_matrix(n: &GltfNodeTransform, u: f32) -> Mat {
    match node_trs_to_blender(n, u) {
        Ok((l, q, s)) => loc_rot_scale(l, q, s),
        Err(m) => m,
    }
}

/// Blender world matrix of every node of a glTF hierarchy: `parents[i]` is the parent index (parents come first).
pub fn world_matrices(nodes: &[GltfNodeTransform], parents: &[Option<usize>], u: f32) -> Vec<Mat> {
    let mut world: Vec<Mat> = Vec::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        let local = local_matrix(n, u);
        world.push(match parents[i] {
            Some(p) => mul(&world[p], &local),
            None => local,
        });
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat, b: &Mat) -> bool {
        (0..4).all(|i| (0..4).all(|j| (a[i][j] - b[i][j]).abs() < 1e-5))
    }

    /// Column-major glTF matrix of T @ R @ S (R from the glTF quaternion `[x, y, z, w]`), built in glTF space.
    fn gltf_matrix(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> [f32; 16] {
        let m = loc_rot_scale(t, [q[3], q[0], q[1], q[2]], s);
        let mut out = [0.0; 16];
        for c in 0..4 {
            for r in 0..4 {
                out[c * 4 + r] = m[r][c];
            }
        }
        out
    }

    fn unit(q: [f32; 4]) -> [f32; 4] {
        let l = q.iter().map(|v| v * v).sum::<f32>().sqrt();
        q.map(|v| v / l)
    }

    #[test]
    fn components_swizzle_y_up_to_z_up() {
        assert_eq!(loc_to_blender([1.0, 2.0, 3.0], 1.0), [1.0, -3.0, 2.0]);
        assert_eq!(loc_to_blender([1.0, 2.0, 3.0], 0.5), [0.5, -1.5, 1.0]);
        assert_eq!(scale_to_blender([1.0, 2.0, 3.0]), [1.0, 3.0, 2.0]);
        assert_eq!(quat_to_blender([0.1, 0.2, 0.3, 0.9]), [0.9, 0.1, -0.3, 0.2]);
    }

    #[test]
    fn missing_properties_use_gltf_defaults() {
        let (l, q, s) = node_trs_to_blender(&GltfNodeTransform::default(), 1.0).unwrap();
        assert_eq!((l, q, s), ([0.0; 3], [1.0, 0.0, 0.0, 0.0], [1.0; 3]));
    }

    #[test]
    fn matrix_wins_over_trs_and_agrees_with_trs() {
        let (t, q, s) = ([1.0, 2.0, 3.0], unit([0.2, -0.4, 0.1, 0.8]), [2.0, 3.0, 0.5]);
        let by_trs = GltfNodeTransform { translation: Some(t), rotation: Some(q), scale: Some(s), matrix: None };
        let by_matrix = GltfNodeTransform { translation: Some([9.0; 3]), matrix: Some(gltf_matrix(t, q, s)), ..Default::default() };
        assert!(node_trs_to_blender(&by_matrix, 1.0).is_err());
        assert!(close(&local_matrix(&by_trs, 1.0), &local_matrix(&by_matrix, 1.0)));
    }

    #[test]
    fn export_inverts_import() {
        let n = GltfNodeTransform {
            translation: Some([1.0, -2.0, 3.5]),
            rotation: Some(unit([0.3, 0.1, -0.5, 0.7])),
            scale: Some([2.0, 1.0, 0.25]),
            matrix: None,
        };
        let trs = node_trs_to_blender(&n, 1.0).unwrap();
        let back = trs_to_gltf(&trs);
        for (a, b) in back.translation.unwrap().iter().zip(n.translation.unwrap()) {
            assert!((a - b).abs() < 1e-6);
        }
        for (a, b) in back.rotation.unwrap().iter().zip(n.rotation.unwrap()) {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(back.scale, n.scale);
    }

    #[test]
    fn export_omits_defaults_and_rounds_near_values() {
        let id = trs_to_gltf(&([1.0e-6, 0.0, 0.0], [1.0, 1.0e-6, 0.0, 0.0], [1.0 + 1.0e-6, 1.0, 1.0]));
        assert_eq!(id, GltfNodeTransform::default());
        let only_scale = trs_to_gltf(&([0.0; 3], [1.0, 0.0, 0.0, 0.0], [1.0, 2.0, 3.0]));
        assert_eq!((only_scale.translation, only_scale.rotation, only_scale.scale), (None, None, Some([1.0, 3.0, 2.0])));
    }

    #[test]
    fn child_world_is_parent_world_times_local() {
        let parent = GltfNodeTransform { translation: Some([0.0, 0.0, -2.0]), scale: Some([2.0, 2.0, 2.0]), ..Default::default() };
        let child = GltfNodeTransform { translation: Some([1.0, 0.0, 0.0]), ..Default::default() };
        let w = world_matrices(&[parent, child], &[None, Some(0)], 1.0);
        /* Parent: Blender location (0, 2, 0), scale 2; the child's local x offset of 1 becomes 2 in world. */
        assert!(close(&w[0], &loc_rot_scale([0.0, 2.0, 0.0], [1.0, 0.0, 0.0, 0.0], [2.0; 3])));
        assert!(close(&w[1], &loc_rot_scale([2.0, 2.0, 0.0], [1.0, 0.0, 0.0, 0.0], [2.0; 3])));
    }
}
