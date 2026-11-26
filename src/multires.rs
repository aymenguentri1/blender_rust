//! Multires data layer: the level counters of `MultiresModifierData` and the Subdivide operation.
//!
//! Modelled on Blender:
//!  - `multires_set_tot_level()` (blenkernel/intern/multires.cc:232): `totlvl` is set, `lvl` is raised to
//!    at least the new total (unless sculpting) and `sculptlvl` / `renderlvl` follow, all clamped to `totlvl`
//!  - `multires_get_level()` (multires.cc:212) without the scene's simplify setting
//!  - Subdivide uses Catmull-Clark (`subdiv_mesh.cc`), the `Mesh::subdivided` of this port; one level turns
//!    every n-gon into n quads, so a cube has 8 / 26 / 98 vertices and 6 / 24 / 96 faces at levels 0 / 1 / 2.
//! Displacement grids (`MDisps`) are not ported.

use crate::mesh::Mesh;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Multires {
    /// Viewport level (`lvl`).
    pub lvl: i32,
    /// Sculpt level (`sculptlvl`).
    pub sculptlvl: i32,
    /// Render level (`renderlvl`).
    pub renderlvl: i32,
    /// Number of subdivision levels stored (`totlvl`).
    pub totlvl: i32,
}

impl Multires {
    /// `multires_set_tot_level()`.
    pub fn set_tot_level(&mut self, lvl: i32, sculpt_mode: bool) {
        self.totlvl = lvl;
        if !sculpt_mode {
            self.lvl = self.lvl.max(lvl).clamp(0, self.totlvl);
        }
        self.sculptlvl = self.sculptlvl.max(lvl).clamp(0, self.totlvl);
        self.renderlvl = self.renderlvl.max(lvl).clamp(0, self.totlvl);
    }

    /// `multires_get_level()` with `ignore_simplify`.
    pub fn level(&self, render: bool, sculpt_mode: bool) -> i32 {
        if render {
            self.renderlvl
        } else if sculpt_mode {
            self.sculptlvl
        } else {
            self.lvl
        }
    }

    /// The Subdivide operator: adds one level to `base` data, returning the mesh at the new total level.
    pub fn subdivide(&mut self, base: &Mesh, sculpt_mode: bool) -> Mesh {
        self.set_tot_level(self.totlvl + 1, sculpt_mode);
        base.subdivided(self.totlvl as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_two_levels_matches_blender_counts() {
        let cube = Mesh::cube(2.0);
        let mut mr = Multires::default();
        let m1 = mr.subdivide(&cube, false);
        assert_eq!((m1.verts.len(), m1.faces.len()), (26, 24));
        let m2 = mr.subdivide(&cube, false);
        assert_eq!((m2.verts.len(), m2.faces.len()), (98, 96));
        assert_eq!(mr, Multires { lvl: 2, sculptlvl: 2, renderlvl: 2, totlvl: 2 });
    }

    #[test]
    fn sculpt_mode_keeps_viewport_level() {
        let mut mr = Multires::default();
        mr.set_tot_level(1, true);
        assert_eq!((mr.lvl, mr.sculptlvl, mr.renderlvl), (0, 1, 1));
        assert_eq!(mr.level(false, true), 1);
        assert_eq!(mr.level(false, false), 0);
    }
}
