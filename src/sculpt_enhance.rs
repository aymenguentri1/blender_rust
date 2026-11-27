//! Enhance Details (`brushes/enhance_details.cc`). Blender has no separate brush for it: `sculpt.cc` runs it when
//! the Smooth brush (Laplacian, the default deform type) starts a stroke with its direction flipped (Ctrl).

use crate::mesh::{Mesh, V3};
use crate::sculpt::{affected, mirror_signs, mirrored, Topo};

/// `calc_smooth_translations()`: how far each vertex is from the average of its neighbours (loose vertices: zero).
/// Taken once, at the first dab of a stroke (`SculptSession::cache::detail_directions`).
pub fn detail_directions(mesh: &Mesh, adj: &Topo) -> Vec<V3> {
    mesh.verts
        .iter()
        .zip(&adj.adj)
        .map(|(v, n)| {
            if n.is_empty() {
                return [0.0; 3];
            }
            let mut avg = [0.0f32; 3];
            for &i in n {
                for k in 0..3 {
                    avg[k] += mesh.verts[i as usize][k] / n.len() as f32;
                }
            }
            [avg[0] - v[0], avg[1] - v[1], avg[2] - v[2]]
        })
        .collect()
}

/// `do_enhance_details_brush()`: moves each vertex by its detail direction times the falloff and the flipped brush
/// strength (`brush_strength()`: `flip * alpha^2`, clamped to `-1..1`), so away from the smoothed surface.
#[allow(clippy::too_many_arguments)]
pub fn enhance_details(mesh: &mut Mesh, mask: &[f32], directions: &[V3], center: V3, radius: f32, strength: f32, symmetry: [bool; 3]) {
    let s = -(strength * strength).clamp(0.0, 1.0);
    for sign in mirror_signs(symmetry) {
        for (i, f) in affected(mesh, mask, mirrored(center, sign), radius) {
            if let Some(d) = directions.get(i) {
                for (co, delta) in mesh.verts[i].iter_mut().zip(d) {
                    *co += delta * f * s;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::adjacency;

    /// A 5 x 5 plane of spacing 0.5 around the origin, and the index of its centre.
    fn plane() -> (Mesh, usize) {
        let mut m = Mesh::default();
        for y in 0..5 {
            for x in 0..5 {
                m.verts.push([x as f32 * 0.5 - 1.0, y as f32 * 0.5 - 1.0, 0.0]);
            }
        }
        for y in 0..4u32 {
            for x in 0..4u32 {
                let a = y * 5 + x;
                m.faces.push(vec![a, a + 1, a + 6, a + 5]);
            }
        }
        (m, 12)
    }

    #[test]
    fn bump_is_amplified_by_falloff_and_squared_strength() {
        let (mut m, c) = plane();
        m.verts[c][2] = 0.1;
        let dirs = detail_directions(&m, &adjacency(&m));
        assert!((dirs[c][2] + 0.1).abs() < 1e-6);
        let before = m.clone();
        enhance_details(&mut m, &[], &dirs, [0.0, 0.0, 0.1], 0.8, 0.5, [false; 3]);
        assert!((m.verts[c][2] - 0.125).abs() < 1e-5, "{}", m.verts[c][2]);
        /* The neighbour dips (it lies below its average) by less than the centre rises. */
        let n = c + 1;
        assert!(m.verts[n][2] < 0.0 && -m.verts[n][2] < 0.025);
        /* Outside the radius nothing moves. */
        assert_eq!(m.verts[0], before.verts[0]);
    }

    #[test]
    fn inner_flat_plane_is_unchanged_and_mask_protects() {
        let (m, c) = plane();
        let dirs = detail_directions(&m, &adjacency(&m));
        let mut after = m.clone();
        enhance_details(&mut after, &[], &dirs, [0.0; 3], 0.8, 1.0, [false; 3]);
        assert_eq!(after, m);
        let mut bump = m.clone();
        bump.verts[c][2] = 0.1;
        let dirs = detail_directions(&bump, &adjacency(&bump));
        let mut after = bump.clone();
        enhance_details(&mut after, &vec![1.0; 25], &dirs, [0.0, 0.0, 0.1], 0.8, 1.0, [false; 3]);
        assert_eq!(after, bump);
    }
}
