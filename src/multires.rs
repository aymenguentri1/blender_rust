//! Multires data layer: the level counters of `MultiresModifierData` and the Subdivide operation.
//!
//! Modelled on Blender:
//!  - `multires_set_tot_level()` (blenkernel/intern/multires.cc:232): `totlvl` is set, `lvl` is raised to
//!    at least the new total (unless sculpting) and `sculptlvl` / `renderlvl` follow, all clamped to `totlvl`
//!  - `multires_get_level()` (multires.cc:212) with `get_render_subsurf_level()` (blenkernel/intern/scene.cc:3021)
//!  - level ranges: `rna_MultiresModifier_level_range()` (makesrna/intern/rna_modifier.cc:1321)
//!  - Subdivide uses Catmull-Clark (`subdiv_mesh.cc`), the `Mesh::subdivided` of this port; one level turns
//!    every n-gon into n quads, so a cube has 8 / 26 / 98 vertices and 6 / 24 / 96 faces at levels 0 / 1 / 2.
//!    Displacement grids (`MDisps`) are not ported.

use crate::mesh::Mesh;

/// The scene's Simplify limits (`R_SIMPLIFY`); `None` where Simplify is off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Simplify {
    /// `simplify_subsurf`.
    pub subsurf: i32,
    /// `simplify_subsurf_render`.
    pub subsurf_render: i32,
}

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

    /// RNA `levels` setter: clamped by `rna_MultiresModifier_level_range()` to `0..=max(0, totlvl)`.
    pub fn set_level(&mut self, v: i32) {
        self.lvl = v.clamp(0, self.totlvl.max(0));
    }

    /// RNA `sculpt_levels` setter, same range.
    pub fn set_sculpt_level(&mut self, v: i32) {
        self.sculptlvl = v.clamp(0, self.totlvl.max(0));
    }

    /// RNA `render_levels` setter, same range.
    pub fn set_render_level(&mut self, v: i32) {
        self.renderlvl = v.clamp(0, self.totlvl.max(0));
    }

    /// `multires_get_level()`: sculpt mode uses `sculptlvl` unless rendering and is never simplified.
    pub fn level(&self, render: bool, sculpt_mode: bool, simplify: Option<Simplify>) -> i32 {
        if render {
            simplify.map_or(self.renderlvl, |s| s.subsurf_render.min(self.renderlvl))
        } else if sculpt_mode {
            self.sculptlvl
        } else {
            simplify.map_or(self.lvl, |s| s.subsurf.min(self.lvl))
        }
    }

    /// The mesh displayed (or rendered) for `base` at the current level.
    pub fn evaluate(&self, base: &Mesh, render: bool, sculpt_mode: bool, simplify: Option<Simplify>) -> Mesh {
        base.subdivided(self.level(render, sculpt_mode, simplify).max(0) as u32)
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
        assert_eq!(mr.level(false, true, None), 1);
        assert_eq!(mr.level(false, false, None), 0);
    }

    #[test]
    fn level_setters_clamp_and_switch_the_evaluated_mesh() {
        let cube = Mesh::cube(2.0);
        let mut mr = Multires::default();
        mr.set_tot_level(2, false);
        mr.set_level(-3);
        assert_eq!(mr.evaluate(&cube, false, false, None).verts.len(), 8);
        mr.set_level(9);
        assert_eq!(mr.lvl, 2);
        assert_eq!(mr.evaluate(&cube, false, false, None).verts.len(), 98);
        mr.set_sculpt_level(1);
        mr.set_render_level(7);
        assert_eq!((mr.sculptlvl, mr.renderlvl), (1, 2));
        assert_eq!(mr.evaluate(&cube, false, true, None).verts.len(), 26);
    }

    #[test]
    fn simplify_limits_viewport_and_render_but_not_sculpt() {
        let mr = Multires { lvl: 3, sculptlvl: 3, renderlvl: 4, totlvl: 4 };
        let s = Some(Simplify { subsurf: 1, subsurf_render: 2 });
        assert_eq!(mr.level(false, false, s), 1);
        assert_eq!(mr.level(true, false, s), 2);
        assert_eq!(mr.level(true, true, s), 2);
        assert_eq!(mr.level(false, true, s), 3);
    }
}
