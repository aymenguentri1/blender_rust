//! Solidify modifier settings and panel layout as in Blender (`SolidifyModifierData`, DNA_modifier_types.h:1336-1410;
//! RNA `rna_def_modifier_solidify`, rna_modifier.cc:5685; `panel_draw` and subpanels, MOD_solidify.cc:70-228).
//!
//! `modifiers::Modifier::Solidify` stores only thickness, offset and even. Differences found by this audit:
//! 1. Rust clamps thickness to >= 0.001; RNA `thickness` ranges over -FLT_MAX..FLT_MAX (negative flips the shell).
//! 2. Missing: mode (Simple/Complex), the Complex-mode thickness and boundary modes, merge threshold, flat faces,
//!    rim fill / rim only, flip and high-quality normals, material offsets, edge creases, bevel convex, thickness
//!    clamp (+ angle clamp), vertex group + factor (+ invert), shell and rim vertex groups.
//! 3. Defaults agree: thickness 0.01 (`offset`), offset -1 (`offset_fac`). Blender's flag default is rim fill only.
//!
//! RNA names differ from DNA: RNA `thickness` is DNA `offset`, RNA `offset` is DNA `offset_fac`.

#![allow(dead_code)] // not wired into the app yet

/// `SolidifyModifierMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// "Simple" (`MOD_SOLIDIFY_MODE_EXTRUDE`).
    Extrude,
    /// "Complex" (`MOD_SOLIDIFY_MODE_NONMANIFOLD`).
    NonManifold,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self { Mode::Extrude => "Simple", Mode::NonManifold => "Complex" }
    }
}

/// `SolidifyModifierNonManifoldOffsetMode` ("Thickness Mode").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThicknessMode {
    Fixed,
    Even,
    Constraints,
}

impl ThicknessMode {
    pub const ALL: [ThicknessMode; 3] = [ThicknessMode::Fixed, ThicknessMode::Even, ThicknessMode::Constraints];
    pub fn label(self) -> &'static str {
        match self { ThicknessMode::Fixed => "Fixed", ThicknessMode::Even => "Even", ThicknessMode::Constraints => "Constraints" }
    }
}

/// `SolidifyModifierNonManifoldBoundaryMode` ("Boundary").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryMode {
    None,
    Round,
    Flat,
}

impl BoundaryMode {
    pub const ALL: [BoundaryMode; 3] = [BoundaryMode::None, BoundaryMode::Round, BoundaryMode::Flat];
    pub fn label(self) -> &'static str {
        match self { BoundaryMode::None => "None", BoundaryMode::Round => "Round", BoundaryMode::Flat => "Flat" }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolidifySettings {
    pub mode: Mode,
    pub thickness_mode: ThicknessMode,
    pub boundary_mode: BoundaryMode,
    /// RNA `thickness` (DNA `offset`).
    pub thickness: f32,
    /// RNA `offset` (DNA `offset_fac`).
    pub offset: f32,
    /// `merge_tolerance` (RNA `nonmanifold_merge_threshold`).
    pub merge_threshold: f32,
    pub use_even_offset: bool,
    pub use_rim: bool,
    pub use_rim_only: bool,
    pub use_flip_normals: bool,
    pub use_quality_normals: bool,
    pub use_flat_faces: bool,
    /// RNA `vertex_group` (`defgrp_name`); empty = none.
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    /// `offset_fac_vg` (RNA `thickness_vertex_group`).
    pub thickness_vertex_group: f32,
    /// `offset_clamp` (RNA `thickness_clamp`).
    pub thickness_clamp: f32,
    pub use_thickness_angle_clamp: bool,
    pub material_offset: i16,
    pub material_offset_rim: i16,
    pub edge_crease_inner: f32,
    pub edge_crease_outer: f32,
    pub edge_crease_rim: f32,
    pub bevel_convex: f32,
}

impl Default for SolidifySettings {
    /// DNA defaults (DNA_modifier_types.h:1372-1410): extrude mode, Constraints / None for Complex mode, flag = rim.
    fn default() -> Self {
        SolidifySettings {
            mode: Mode::Extrude,
            thickness_mode: ThicknessMode::Constraints,
            boundary_mode: BoundaryMode::None,
            thickness: 0.01,
            offset: -1.0,
            merge_threshold: 0.0001,
            use_even_offset: false,
            use_rim: true,
            use_rim_only: false,
            use_flip_normals: false,
            use_quality_normals: false,
            use_flat_faces: false,
            vertex_group: String::new(),
            invert_vertex_group: false,
            thickness_vertex_group: 0.0,
            thickness_clamp: 0.0,
            use_thickness_angle_clamp: false,
            material_offset: 0,
            material_offset_rim: 0,
            edge_crease_inner: 0.0,
            edge_crease_outer: 0.0,
            edge_crease_rim: 0.0,
            bevel_convex: 0.0,
        }
    }
}

impl SolidifySettings {
    /// RNA range of `thickness` and `offset`: -FLT_MAX..FLT_MAX (no clamp besides finiteness).
    pub fn set_thickness(&mut self, v: f32) { self.thickness = v.clamp(-f32::MAX, f32::MAX); }
    pub fn set_offset(&mut self, v: f32) { self.offset = v.clamp(-f32::MAX, f32::MAX); }
    /// `nonmanifold_merge_threshold`: 0..1.
    pub fn set_merge_threshold(&mut self, v: f32) { self.merge_threshold = v.clamp(0.0, 1.0); }
    /// `thickness_vertex_group`: 0..1.
    pub fn set_thickness_vertex_group(&mut self, v: f32) { self.thickness_vertex_group = v.clamp(0.0, 1.0); }
    /// `thickness_clamp`: 0..100.
    pub fn set_thickness_clamp(&mut self, v: f32) { self.thickness_clamp = v.clamp(0.0, 100.0); }
    /// `edge_crease_inner/outer/rim`: 0..1.
    pub fn set_crease(v: f32) -> f32 { v.clamp(0.0, 1.0) }
    /// `bevel_convex`: -1..1.
    pub fn set_bevel_convex(&mut self, v: f32) { self.bevel_convex = v.clamp(-1.0, 1.0); }

    pub fn has_vertex_group(&self) -> bool { !self.vertex_group.is_empty() }
    /// `sub.active_set(use_rim)` on "Only Rim".
    pub fn rim_only_active(&self) -> bool { self.use_rim }
    /// `row->active_set(has_vertex_group)` on the "Factor" row (and on Flat Faces in Complex mode).
    pub fn vertex_group_factor_active(&self) -> bool { self.has_vertex_group() }
    /// `row.active_set(thickness_clamp > 0)` on "Angle Clamp".
    pub fn angle_clamp_active(&self) -> bool { self.thickness_clamp > 0.0 }

    /// Main panel rows in `panel_draw` order for the current mode.
    pub fn main_rows(&self) -> Vec<&'static str> {
        let mut rows = vec!["Mode"];
        if self.mode == Mode::NonManifold { rows.extend(["Thickness Mode", "Boundary"]); }
        rows.extend(["Thickness", "Offset"]);
        rows.push(if self.mode == Mode::NonManifold { "Merge Threshold" } else { "Even Thickness" });
        rows.extend(["Fill", "Only Rim", "Vertex Group", "Factor"]);
        if self.mode == Mode::NonManifold { rows.push("Flat Faces"); }
        rows
    }

    /// "Normals" subpanel rows: High Quality exists only in Simple mode (`normals_panel_draw`).
    pub fn normals_rows(&self) -> Vec<&'static str> {
        if self.mode == Mode::Extrude { vec!["Flip", "High Quality"] } else { vec!["Flip"] }
    }

    /// "Edge Data" subpanel shows the creases only in Simple mode; Bevel Convex always (`edge_data_panel_draw`).
    pub fn edge_data_rows(&self) -> Vec<&'static str> {
        if self.mode == Mode::Extrude { vec!["Crease Inner", "Outer", "Rim", "Bevel Convex"] } else { vec!["Bevel Convex"] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_dna() {
        let s = SolidifySettings::default();
        assert_eq!((s.thickness, s.offset, s.merge_threshold), (0.01, -1.0, 0.0001));
        assert!(s.use_rim && !s.use_rim_only && !s.use_even_offset && !s.use_flip_normals);
        assert_eq!((s.mode, s.thickness_mode, s.boundary_mode), (Mode::Extrude, ThicknessMode::Constraints, BoundaryMode::None));
    }

    #[test]
    fn thickness_may_be_negative_and_ranges_clamp() {
        let mut s = SolidifySettings::default();
        s.set_thickness(-2.5);
        assert_eq!(s.thickness, -2.5);
        s.set_merge_threshold(3.0);
        s.set_thickness_clamp(500.0);
        s.set_bevel_convex(-4.0);
        s.set_thickness_vertex_group(-1.0);
        assert_eq!((s.merge_threshold, s.thickness_clamp, s.bevel_convex, s.thickness_vertex_group), (1.0, 100.0, -1.0, 0.0));
        assert_eq!(SolidifySettings::set_crease(2.0), 1.0);
    }

    #[test]
    fn active_states_follow_panel_draw() {
        let mut s = SolidifySettings::default();
        assert!(s.rim_only_active() && !s.vertex_group_factor_active() && !s.angle_clamp_active());
        s.use_rim = false;
        s.vertex_group = "Group".into();
        s.thickness_clamp = 0.5;
        assert!(!s.rim_only_active() && s.vertex_group_factor_active() && s.angle_clamp_active());
    }

    #[test]
    fn rows_depend_on_mode() {
        let mut s = SolidifySettings::default();
        assert_eq!(s.main_rows(), ["Mode", "Thickness", "Offset", "Even Thickness", "Fill", "Only Rim", "Vertex Group", "Factor"]);
        assert_eq!(s.normals_rows(), ["Flip", "High Quality"]);
        assert_eq!(s.edge_data_rows().len(), 4);
        s.mode = Mode::NonManifold;
        assert_eq!(s.main_rows(), ["Mode", "Thickness Mode", "Boundary", "Thickness", "Offset", "Merge Threshold", "Fill", "Only Rim", "Vertex Group", "Factor", "Flat Faces"]);
        assert_eq!(s.normals_rows(), ["Flip"]);
        assert_eq!(s.edge_data_rows(), ["Bevel Convex"]);
    }

    #[test]
    fn enum_labels_match_rna() {
        assert_eq!((Mode::Extrude.label(), Mode::NonManifold.label()), ("Simple", "Complex"));
        assert_eq!(ThicknessMode::ALL.map(ThicknessMode::label), ["Fixed", "Even", "Constraints"]);
        assert_eq!(BoundaryMode::ALL.map(BoundaryMode::label), ["None", "Round", "Flat"]);
    }
}
