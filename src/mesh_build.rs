//! The Build modifier (`MOD_build.cc`): the mesh is revealed face by face over a frame range, or taken apart
//! again with Reverse.
//!
//! The fraction `(frame - start) / length` (clamped to 0..1, flipped by Reverse) of the faces is kept, in
//! mesh order or, with Randomize, in the order `BLI_array_randomize()` gives for the seed. The vertices are the
//! ones those faces use, numbered by first use. A mesh without faces keeps the same fraction of its vertices.
//! Not modelled: Blender builds loose edges one by one while less than one face is due (a mesh here has no loose
//! edges, so the result is empty until the first face appears).

use crate::mesh::Mesh;

/// `RandomNumberGenerator` of `BLI_rand.hh`: a 48 bit linear congruential generator.
struct Rng {
    x: u64,
}

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng { x: ((seed as u64) << 16) | 0x330E }
    }

    /// `get_uint32()`: the 31 high bits of the state after one step.
    fn get_uint32(&mut self) -> u32 {
        self.x = 0x5_DEEC_E66D_u64.wrapping_mul(self.x).wrapping_add(0xB) & 0x0000_FFFF_FFFF_FFFF;
        (self.x >> 17) as u32
    }
}

/// `BLI_array_randomize()` / `BLI_rng_shuffle_array()`.
pub fn array_randomize(data: &mut [u32], seed: u32) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    let mut rng = Rng::new(seed);
    for i in (0..n).rev() {
        let j = (rng.get_uint32() % n as u32) as usize;
        if i != j {
            data.swap(i, j);
        }
    }
}

/// The fraction of the mesh that is built at `frame`.
pub fn build_fraction(frame: f32, start: f32, length: f32, reverse: bool) -> f32 {
    let frac = ((frame - start) / length.max(1.0)).clamp(0.0, 1.0);
    if reverse {
        1.0 - frac
    } else {
        frac
    }
}

impl Mesh {
    /// The Build modifier at `frame`, see the module documentation.
    pub fn built(&self, frame: f32, start: f32, length: f32, reverse: bool, randomize: bool, seed: u32) -> Mesh {
        let frac = build_fraction(frame, start, length, reverse);
        if self.faces.is_empty() {
            let mut map: Vec<u32> = (0..self.verts.len() as u32).collect();
            if randomize {
                array_randomize(&mut map, seed);
            }
            let keep = (self.verts.len() as f32 * frac) as usize;
            return Mesh { verts: map[..keep].iter().map(|&i| self.verts[i as usize]).collect(), faces: Vec::new() };
        }
        let keep = (self.faces.len() as f32 * frac) as usize;
        if keep == 0 {
            return Mesh::default();
        }
        let mut map: Vec<u32> = (0..self.faces.len() as u32).collect();
        if randomize {
            array_randomize(&mut map, seed);
        }
        let mut new_index = vec![u32::MAX; self.verts.len()];
        let mut out = Mesh::default();
        for &f in &map[..keep] {
            let face: Vec<u32> = self.faces[f as usize]
                .iter()
                .map(|&v| {
                    if new_index[v as usize] == u32::MAX {
                        new_index[v as usize] = out.verts.len() as u32;
                        out.verts.push(self.verts[v as usize]);
                    }
                    new_index[v as usize]
                })
                .collect();
            out.faces.push(face);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fraction_follows_the_frame_range() {
        assert_eq!(build_fraction(1.0, 1.0, 100.0, false), 0.0);
        assert_eq!(build_fraction(51.0, 1.0, 100.0, false), 0.5);
        assert_eq!(build_fraction(500.0, 1.0, 100.0, false), 1.0);
        assert_eq!(build_fraction(-4.0, 1.0, 100.0, false), 0.0);
        assert_eq!(build_fraction(51.0, 1.0, 100.0, true), 0.5);
        assert_eq!(build_fraction(1.0, 1.0, 100.0, true), 1.0);
    }

    #[test]
    fn faces_appear_in_order_with_their_vertices() {
        let cube = Mesh::cube(2.0);
        assert!(cube.built(1.0, 1.0, 100.0, false, false, 0).faces.is_empty());
        /* Half of the 6 faces, 3 of them. */
        let half = cube.built(51.0, 1.0, 100.0, false, false, 0);
        assert_eq!(half.faces.len(), 3);
        assert!(half.faces.iter().flatten().all(|&v| (v as usize) < half.verts.len()));
        /* One face of six needs only its own 4 corners. */
        assert_eq!(cube.built(26.0, 1.0, 100.0, false, false, 0).verts.len(), 4);
        assert_eq!(cube.built(101.0, 1.0, 100.0, false, false, 0), cube);
        /* Reverse takes the cube apart again. */
        assert_eq!(cube.built(101.0, 1.0, 100.0, true, false, 0).faces.len(), 0);
        assert_eq!(cube.built(1.0, 1.0, 100.0, true, false, 0), cube);
    }

    #[test]
    fn random_order_is_a_seeded_permutation() {
        let mut a: Vec<u32> = (0..20).collect();
        array_randomize(&mut a, 3);
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<u32>>());
        assert_ne!(a, (0..20).collect::<Vec<u32>>());
        let mut b: Vec<u32> = (0..20).collect();
        array_randomize(&mut b, 3);
        assert_eq!(a, b);
        let mut c: Vec<u32> = (0..20).collect();
        array_randomize(&mut c, 4);
        assert_ne!(a, c);
        let grid = Mesh::grid(4, 4, 2.0);
        let r = grid.built(51.0, 1.0, 100.0, false, true, 1);
        assert_eq!(r.faces.len(), grid.faces.len() / 2);
        assert_ne!(r, grid.built(51.0, 1.0, 100.0, false, false, 1));
    }

    #[test]
    fn a_mesh_without_faces_builds_its_vertices() {
        let loose = Mesh { verts: (0..10).map(|i| [i as f32, 0.0, 0.0]).collect(), faces: Vec::new() };
        assert_eq!(loose.built(51.0, 1.0, 100.0, false, false, 0).verts.len(), 5);
    }
}
