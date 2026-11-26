//! glTF primitive mode -> points / edges / triangles (io_scene_gltf2/blender/imp/mesh.py `points_edges_tris`, :791-893) and
//! `uvs_gltf_to_blender` (:908). Edges are flat index pairs, triangles flat index triplets, as in the importer.

#![allow(dead_code)]

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PointsEdgesTris {
    pub points: Option<Vec<u32>>,
    pub edges: Option<Vec<u32>>,
    pub tris: Option<Vec<u32>>,
}

/// `points_edges_tris(mode, indices)`. Index counts for which numpy would raise (empty line loop/strip, fewer than 2
/// strip/fan indices) and unknown modes are errors.
pub fn points_edges_tris(mode: u32, indices: &[u32]) -> Result<PointsEdgesTris, String> {
    let n = indices.len();
    let mut r = PointsEdgesTris::default();
    match mode {
        0 => r.points = Some(indices.to_vec()),
        1 => r.edges = Some(indices.to_vec()),
        2 => {
            /* in 0123 -> out 01122330 */
            if n == 0 {
                return Err("line loop needs at least 1 index".into());
            }
            let mut e = Vec::with_capacity(2 * n);
            e.push(indices[0]);
            for &i in &indices[1..] {
                e.extend([i, i]);
            }
            e.push(indices[0]);
            r.edges = Some(e);
        }
        3 => {
            /* in 0123 -> out 011223 */
            if n < 2 {
                return Err("line strip needs at least 2 indices".into());
            }
            let mut e = Vec::with_capacity(2 * n - 2);
            e.push(indices[0]);
            for &i in &indices[1..n - 1] {
                e.extend([i, i]);
            }
            e.push(indices[n - 1]);
            r.edges = Some(e);
        }
        4 => r.tris = Some(indices.to_vec()),
        5 => {
            /* in 01234 -> 012, 132, 234 */
            if n < 3 {
                return Err("triangle strip needs at least 3 indices".into());
            }
            let mut t = Vec::with_capacity((n - 2) * 3);
            for k in 0..n - 2 {
                if k % 2 == 0 {
                    t.extend([indices[k], indices[k + 1], indices[k + 2]]);
                } else {
                    t.extend([indices[k], indices[k + 2], indices[k + 1]]);
                }
            }
            r.tris = Some(t);
        }
        6 => {
            /* in 01234 -> 012, 023, 034 */
            if n < 3 {
                return Err("triangle fan needs at least 3 indices".into());
            }
            let mut t = Vec::with_capacity((n - 2) * 3);
            for k in 1..n - 1 {
                t.extend([indices[0], indices[k], indices[k + 1]]);
            }
            r.tris = Some(t);
        }
        _ => return Err(format!("primitive mode unimplemented: {mode}")),
    }
    Ok(r)
}

/// `uvs_gltf_to_blender`: `(u, v) -> (u, 1 - v)`, in place.
pub fn uvs_gltf_to_blender(uvs: &mut [[f32; 2]]) {
    for uv in uvs {
        uv[1] = -uv[1] + 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mode: u32, idx: &[u32]) -> PointsEdgesTris {
        points_edges_tris(mode, idx).unwrap()
    }

    #[test]
    fn point_line_and_triangle_modes_pass_through() {
        assert_eq!(run(0, &[3, 1]).points, Some(vec![3, 1]));
        assert_eq!(run(1, &[0, 1, 2, 3]).edges, Some(vec![0, 1, 2, 3]));
        assert_eq!(run(4, &[0, 1, 2]).tris, Some(vec![0, 1, 2]));
        assert_eq!(run(4, &[0, 1, 2]).edges, None);
    }

    #[test]
    fn line_loop_and_strip() {
        assert_eq!(run(2, &[0, 1, 2, 3]).edges.unwrap(), [0, 1, 1, 2, 2, 3, 3, 0]);
        assert_eq!(run(2, &[5]).edges.unwrap(), [5, 5]);
        assert_eq!(run(3, &[0, 1, 2, 3]).edges.unwrap(), [0, 1, 1, 2, 2, 3]);
        assert_eq!(run(3, &[7, 9]).edges.unwrap(), [7, 9]);
    }

    #[test]
    fn triangle_strip_alternates_winding() {
        assert_eq!(run(5, &[0, 1, 2, 3, 4]).tris.unwrap(), [0, 1, 2, 1, 3, 2, 2, 3, 4]);
        assert_eq!(run(5, &[0, 1, 2]).tris.unwrap(), [0, 1, 2]);
    }

    #[test]
    fn triangle_fan_shares_the_first_index() {
        assert_eq!(run(6, &[0, 1, 2, 3, 4]).tris.unwrap(), [0, 1, 2, 0, 2, 3, 0, 3, 4]);
    }

    #[test]
    fn invalid_input_is_an_error() {
        assert!(points_edges_tris(7, &[0]).is_err());
        assert!(points_edges_tris(5, &[0, 1]).is_err());
        assert!(points_edges_tris(6, &[0, 1]).is_err());
        assert!(points_edges_tris(3, &[0]).is_err());
        assert!(points_edges_tris(2, &[]).is_err());
    }

    #[test]
    fn uvs_flip_v() {
        let mut uv = [[0.25, 0.0], [1.0, 1.0], [0.5, 0.75]];
        uvs_gltf_to_blender(&mut uv);
        assert_eq!(uv, [[0.25, 1.0], [1.0, 0.0], [0.5, 0.25]]);
    }
}
