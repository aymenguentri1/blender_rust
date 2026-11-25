//! The Corrective Smooth modifier (`MOD_correctivesmooth.cc`, "delta mush"): smooths the vertices, then puts back the
//! detail that the smoothing removed from the rest mesh, expressed in a per-corner tangent space so that it follows
//! the deformation.
//!
//! The rest coordinates are the vertices of the mesh at the start of the stack (Blender's Original Coords
//! source), so the modifier does nothing when an earlier modifier changed the vertex count. Simple smoothing only;
//! Bind, Only Smooth, Pin Boundary, the vertex group and Length Weight are not modelled.

use crate::mesh::{add, cross, dot, normalized, sub, Mesh, V3};

/// `smooth_iter__simple()`: each round moves a vertex by `lambda` times the mean of its edge vectors (an isolated
/// vertex stays put).
fn smooth_simple(mesh: &Mesh, cos: &mut [V3], lambda: f32, repeat: u32) {
    let edges = mesh.edges();
    let mut count = vec![0.0f32; cos.len()];
    for e in &edges {
        count[e.a as usize] += 1.0;
        count[e.b as usize] += 1.0;
    }
    let scale: Vec<f32> = count.iter().map(|&c| lambda * if c > 0.0 { 1.0 / c } else { 1.0 }).collect();
    for _ in 0..repeat {
        let mut delta = vec![[0.0f32; 3]; cos.len()];
        for e in &edges {
            let dir = sub(cos[e.b as usize], cos[e.a as usize]);
            for k in 0..3 {
                delta[e.a as usize][k] += dir[k];
                delta[e.b as usize][k] -= dir[k];
            }
        }
        for i in 0..cos.len() {
            for k in 0..3 {
                cos[i][k] += delta[i][k] * scale[i];
            }
        }
    }
}

/// `calc_tangent_loop()`: the orthonormal frame of a corner from the directions to its previous and next
/// neighbours, `None` when they coincide.
fn tangent_loop(prev: V3, next: V3) -> Option<[V3; 3]> {
    if (0..3).all(|k| (prev[k] - next[k]).abs() <= f32::EPSILON * 10.0) {
        return None;
    }
    let z = normalized(cross(prev, next));
    let x = normalized(add(prev, next));
    let y = cross(z, x);
    Some([x, y, z])
}

/// `calc_tangent_spaces()`: per corner (faces in order) the frame, the angle weight and the weight sum per vertex.
fn tangent_spaces(mesh: &Mesh, cos: &[V3]) -> (Vec<[V3; 3]>, Vec<f32>, Vec<f32>) {
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut spaces = Vec::new();
    let mut weights = Vec::new();
    let mut per_vertex = vec![0.0f32; cos.len()];
    for face in &mesh.faces {
        let n = face.len();
        for k in 0..n {
            let (prev, cur, next) = (face[(k + n - 1) % n] as usize, face[k] as usize, face[(k + 1) % n] as usize);
            let dir_prev = normalized(sub(cos[prev], cos[cur]));
            let dir_next = normalized(sub(cos[cur], cos[next]));
            match tangent_loop(dir_prev, dir_next) {
                Some(ts) => {
                    let w = dot(dir_next, dir_prev).clamp(-1.0, 1.0).acos().abs();
                    spaces.push(ts);
                    weights.push(w);
                    per_vertex[cur] += w;
                }
                None => {
                    spaces.push(identity);
                    weights.push(0.0);
                }
            }
        }
    }
    (spaces, weights, per_vertex)
}

impl Mesh {
    /// The Corrective Smooth modifier with `rest` as the rest coordinates, see the module documentation.
    pub fn corrective_smoothed(&self, rest: &[V3], lambda: f32, repeat: u32, scale: f32) -> Mesh {
        if rest.len() != self.verts.len() || self.faces.is_empty() {
            return self.clone();
        }
        /* `calc_deltas()`: what the smoothing takes off the rest mesh, per corner in its tangent space. */
        let mut smooth_rest = rest.to_vec();
        smooth_simple(self, &mut smooth_rest, lambda, repeat);
        let (rest_spaces, _, _) = tangent_spaces(self, &smooth_rest);
        let mut deltas = Vec::new();
        let mut l = 0;
        for face in &self.faces {
            for &v in face {
                let d = sub(rest[v as usize], smooth_rest[v as usize]);
                let ts = rest_spaces[l];
                deltas.push([dot(ts[0], d), dot(ts[1], d), dot(ts[2], d)]);
                l += 1;
            }
        }
        /* Smooth the current vertices and add the deltas back in the new tangent spaces. */
        let mut cos = self.verts.clone();
        smooth_simple(self, &mut cos, lambda, repeat);
        let (spaces, weights, per_vertex) = tangent_spaces(self, &cos);
        let mut out = cos.clone();
        let mut l = 0;
        for face in &self.faces {
            for &v in face {
                let weight = weights[l] / per_vertex[v as usize];
                if weight > 0.0 {
                    let (ts, d) = (spaces[l], deltas[l]);
                    for k in 0..3 {
                        out[v as usize][k] += (ts[0][k] * d[0] + ts[1][k] * d[1] + ts[2][k] * d[2]) * weight * scale;
                    }
                }
                l += 1;
            }
        }
        Mesh { verts: out, faces: self.faces.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_diff(a: &Mesh, b: &Mesh) -> f32 {
        a.verts.iter().zip(&b.verts).map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max)
    }

    #[test]
    fn an_undeformed_mesh_comes_back_unchanged() {
        /* The deltas are exactly what the smoothing removes, so smoothing and correcting the rest mesh is the identity. */
        let sphere = Mesh::uv_sphere(16, 8, 1.0);
        let r = sphere.corrective_smoothed(&sphere.verts, 0.5, 5, 1.0);
        assert_eq!(r.faces, sphere.faces);
        assert!(max_diff(&r, &sphere) < 1e-3, "{}", max_diff(&r, &sphere));
    }

    #[test]
    fn it_keeps_the_detail_that_plain_smoothing_loses() {
        let rest = Mesh::uv_sphere(16, 8, 1.0);
        /* Stretch the sphere along Z (what an earlier deformer would do). */
        let stretched = rest.mapped(|v| [v[0], v[1], v[2] * 2.0], false);
        let plain = stretched.smoothed(0.5, 5);
        let fixed = stretched.corrective_smoothed(&rest.verts, 0.5, 5, 1.0);
        assert_eq!(fixed.faces, stretched.faces);
        assert!(max_diff(&fixed, &plain) > 1e-3);
        assert!(fixed.verts.iter().flatten().all(|c| c.is_finite()));
        /* Scale 0 leaves only the smoothing. */
        let flat = stretched.corrective_smoothed(&rest.verts, 0.5, 5, 0.0);
        let mut by_hand = stretched.verts.clone();
        smooth_simple(&stretched, &mut by_hand, 0.5, 5);
        assert!(flat.verts.iter().zip(&by_hand).all(|(a, b)| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6)));
    }

    #[test]
    fn a_vertex_count_mismatch_does_nothing() {
        let cube = Mesh::cube(2.0);
        assert_eq!(cube.corrective_smoothed(&[[0.0; 3]; 3], 0.5, 5, 1.0), cube);
    }
}
