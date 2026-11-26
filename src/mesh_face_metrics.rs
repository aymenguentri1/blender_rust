//! BMesh face metrics: `BM_face_calc_area` and `BM_face_calc_perimeter` (`bmesh_polygon.cc`).
//!
//! Area is half the length of the Newell normal (`add_newell_cross_v3_v3v3`, `math_vector_inline.cc`), so a non-planar
//! polygon gets the area of its projection onto the Newell plane. Perimeter is the sum of the loop edge lengths.

use crate::mesh::Mesh;

/// `BM_face_calc_area`.
pub fn face_calc_area(mesh: &Mesh, f: usize) -> f32 {
    let n = mesh.face_area_normal(f);
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() * 0.5
}

/// `BM_face_calc_perimeter`.
pub fn face_calc_perimeter(mesh: &Mesh, f: usize) -> f32 {
    let face = &mesh.faces[f];
    (0..face.len())
        .map(|i| {
            let (a, b) = (mesh.verts[face[i] as usize], mesh.verts[face[(i + 1) % face.len()] as usize]);
            let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(v: &[[f32; 3]]) -> Mesh {
        Mesh { verts: v.to_vec(), faces: vec![(0..v.len() as u32).collect()] }
    }

    #[test]
    fn unit_square() {
        let m = mesh(&[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]);
        assert!((face_calc_area(&m, 0) - 1.0).abs() < 1e-6);
        assert!((face_calc_perimeter(&m, 0) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn triangle() {
        let m = mesh(&[[0., 0., 0.], [3., 0., 0.], [0., 4., 0.]]);
        assert!((face_calc_area(&m, 0) - 6.0).abs() < 1e-5);
        assert!((face_calc_perimeter(&m, 0) - 12.0).abs() < 1e-5);
    }

    #[test]
    fn non_planar_quad_uses_projected_area() {
        // one corner lifted: Newell normal is (-1, -1, 2) by hand, area = sqrt(6) / 2
        let m = mesh(&[[0., 0., 0.], [1., 0., 0.], [1., 1., 1.], [0., 1., 0.]]);
        assert!((face_calc_area(&m, 0) - 6f32.sqrt() / 2.0).abs() < 1e-6);
    }

    #[test]
    fn winding_does_not_change_area() {
        let m = mesh(&[[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]]);
        assert!((face_calc_area(&m, 0) - 1.0).abs() < 1e-6);
    }
}
