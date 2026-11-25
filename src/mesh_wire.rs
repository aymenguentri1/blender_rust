//! The Wireframe modifier (simplified).
//!
//! Modelled on `MOD_wireframe`: faces are replaced by a frame of the given `thickness`. Each face keeps a
//! border of that width (mitered at the corners, clamped so the hole never closes), the hole is cut through,
//! and the frame is extruded `thickness` against the face normal so it is a solid bar, not a flat ring.
//! Blender merges the bars of adjacent faces into tubes around the edges; here every face gets its own
//! closed frame, so shared edges carry two touching bars (identical look, twice the faces).
//! Faces with fewer than 3 corners are skipped.

use crate::mesh::{add, cross, dot, normalized, scale, sub, Mesh};

impl Mesh {
    pub fn wireframed(&self, thickness: f32) -> Mesh {
        let mut out = Mesh::default();
        for (fi, face) in self.faces.iter().enumerate() {
            let n = face.len();
            if n < 3 {
                continue;
            }
            let nrm = self.face_normal(fi);
            let center = self.face_center(fi);
            let down = scale(nrm, -thickness);
            let base = out.verts.len() as u32;
            /* Layout: outer front [0, n), inner front [n, 2n), outer back [2n, 3n), inner back [3n, 4n). */
            let mut inner = Vec::with_capacity(n);
            for i in 0..n {
                let v = self.verts[face[i] as usize];
                let prev = self.verts[face[(i + n - 1) % n] as usize];
                let next = self.verts[face[(i + 1) % n] as usize];
                let e1 = normalized(sub(prev, v));
                let e2 = normalized(sub(next, v));
                let bis = normalized(add(e1, e2));
                let sin_half = ((1.0 - dot(e1, e2)) * 0.5).max(0.0).sqrt().max(1e-3);
                let to_center = sub(center, v);
                let max_len = dot(to_center, to_center).sqrt() * 0.9;
                let len = (thickness / sin_half).min(max_len);
                /* The bisector points into the face for a convex corner; flip it for a reflex one. */
                let dir = if dot(bis, to_center) >= 0.0 { bis } else { scale(bis, -1.0) };
                inner.push(add(v, scale(dir, len)));
            }
            for i in 0..n {
                out.verts.push(self.verts[face[i] as usize]);
            }
            for p in &inner {
                out.verts.push(*p);
            }
            for i in 0..n {
                out.verts.push(add(self.verts[face[i] as usize], down));
            }
            for p in &inner {
                out.verts.push(add(*p, down));
            }
            let (of, inf, ob, ib) = (base, base + n as u32, base + 2 * n as u32, base + 3 * n as u32);
            for i in 0..n as u32 {
                let j = (i + 1) % n as u32;
                out.faces.push(vec![of + i, of + j, inf + j, inf + i]); /* front ring */
                out.faces.push(vec![inf + i, inf + j, ib + j, ib + i]); /* wall of the hole */
                out.faces.push(vec![ob + i, ib + i, ib + j, ob + j]); /* back ring */
                out.faces.push(vec![of + i, ob + i, ob + j, of + j]); /* outer wall */
            }
        }
        let _ = cross; /* kept for symmetry with the other geometry modules */
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_becomes_six_frames() {
        let cube = Mesh::cube(2.0);
        let w = cube.wireframed(0.1);
        assert_eq!(w.faces.len(), 6 * 16);
        assert_eq!(w.verts.len(), 6 * 16);
        assert!(w.faces.iter().all(|f| f.len() == 4));
        assert!(w.faces.iter().flatten().all(|&i| (i as usize) < w.verts.len()));
    }

    #[test]
    fn the_hole_stays_open_and_the_frame_has_the_requested_width() {
        let plane = Mesh::plane(2.0);
        let w = plane.wireframed(0.2);
        /* Inner front corners sit 0.2 m in from each side of the square: |x|, |y| = 0.8. */
        for v in &w.verts[4..8] {
            assert!((v[0].abs() - 0.8).abs() < 1e-4 && (v[1].abs() - 0.8).abs() < 1e-4, "{v:?}");
        }
        /* Back vertices sit 0.2 m below the front ones (the plane's normal is +Z). */
        assert!((w.verts[8][2] + 0.2).abs() < 1e-5);
        /* A huge thickness is clamped, never inverting the hole. */
        let big = plane.wireframed(50.0);
        assert!(big.verts[4][0].abs() < 1.0);
    }
}
