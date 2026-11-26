//! Port of `multires_reshape_apply_base_refit_base_mesh()`
//! (`source/blender/blenkernel/intern/multires_reshape_apply_base.cc`).
//! The `MultiresReshapeContext` is reduced to the base mesh (`Mesh`).

use crate::mesh::{add, dot, normalized, scale, sub, Mesh, V3};

/// Moves every base vertex along the averaged normal of its faces (built with the vertex replaced by the
/// centre of its neighbours) by its signed distance from the plane through that centre. Vertices that no face
/// uses are left alone.
pub fn apply_base_refit_base_mesh(base: &mut Mesh) {
    let origco: Vec<V3> = base.verts.clone();
    let mut vert_faces: Vec<Vec<usize>> = vec![Vec::new(); base.verts.len()];
    for (f, face) in base.faces.iter().enumerate() {
        for &v in face {
            vert_faces[v as usize].push(f);
        }
    }
    for i in 0..base.verts.len() {
        if vert_faces[i].is_empty() {
            continue;
        }
        /* Centre: double counts like Blender, a vertex repeated in several faces is added once per face. */
        let mut center = [0.0f32; 3];
        let mut tot = 0;
        for &f in &vert_faces[i] {
            for &v in &base.faces[f] {
                if v as usize != i {
                    center = add(center, origco[v as usize]);
                    tot += 1;
                }
            }
        }
        center = scale(center, 1.0 / tot as f32);

        let mut avg_no = [0.0f32; 3];
        for &f in &vert_faces[i] {
            let face = &base.faces[f];
            let fake = Mesh {
                verts: face.iter().map(|&v| if v as usize == i { center } else { origco[v as usize] }).collect(),
                faces: vec![(0..face.len() as u32).collect()],
            };
            avg_no = add(avg_no, fake.face_normal(0));
        }
        let avg_no = normalized(avg_no);
        let dist = dot(sub(base.verts[i], center), avg_no);
        base.verts[i] = add(base.verts[i], scale(avg_no, dist));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_grid_is_unchanged() {
        let mut m = Mesh::grid(3, 3, 2.0);
        let before = m.verts.clone();
        apply_base_refit_base_mesh(&mut m);
        for (a, b) in m.verts.iter().zip(&before) {
            assert!(dot(sub(*a, *b), sub(*a, *b)) < 1e-10);
        }
    }

    #[test]
    fn raised_vertex_moves_by_its_distance_along_the_normal() {
        /* Blender adds `normal * signed_distance` to the vertex, so an offset of 1 becomes 2. */
        let mut m = Mesh::grid(2, 2, 2.0);
        let c = (0..m.verts.len()).find(|&i| m.verts[i][0].abs() < 1e-6 && m.verts[i][1].abs() < 1e-6).unwrap();
        let z0 = m.verts[c][2];
        m.verts[c][2] = z0 + 1.0;
        apply_base_refit_base_mesh(&mut m);
        assert!((m.verts[c][2] - (z0 + 2.0)).abs() < 1e-4, "{:?}", m.verts[c]);
    }

    #[test]
    fn vertex_without_faces_is_untouched() {
        let mut m = Mesh { verts: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [5.0, 5.0, 5.0]], faces: vec![vec![0, 1, 2]] };
        apply_base_refit_base_mesh(&mut m);
        assert_eq!(m.verts[3], [5.0, 5.0, 5.0]);
    }
}
