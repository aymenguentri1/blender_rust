//! `MeshAutomaskingSettings` (DNA_scene_types.h:1232 defaults; RNA ranges in rna_sculpt_paint.cc
//! `rna_def_mesh_automasking`; `AUTOMASKING_BOUNDARY_EDGES_MAX_PROPAGATION_STEPS` in DNA_brush_types.h) and the header
//! popover icon rule `VIEW3D_HT_header._mesh_paint_automasking_icon` (space_view3d.py).
//! Deviation: the cavity curve mapping is not stored; `use_automasking_custom_cavity_curve` is only the flag.

use std::f32::consts::PI;

/// `AUTOMASKING_BOUNDARY_EDGES_MAX_PROPAGATION_STEPS`.
pub const BOUNDARY_MAX_STEPS: i32 = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct AutomaskSettings {
    pub topology: bool,
    pub face_sets: bool,
    pub boundary_edges: bool,
    pub boundary_face_sets: bool,
    pub cavity: bool,
    pub cavity_inverted: bool,
    pub custom_cavity_curve: bool,
    pub view_normal: bool,
    pub view_occlusion: bool,
    pub start_normal: bool,
    boundary_edges_propagation_steps: i32,
    cavity_blur_steps: i32,
    cavity_factor: f32,
    start_normal_limit: f32,
    start_normal_falloff: f32,
    view_normal_limit: f32,
    view_normal_falloff: f32,
}

impl Default for AutomaskSettings {
    fn default() -> Self {
        AutomaskSettings {
            topology: false,
            face_sets: false,
            boundary_edges: false,
            boundary_face_sets: false,
            cavity: false,
            cavity_inverted: false,
            custom_cavity_curve: false,
            view_normal: false,
            view_occlusion: false,
            start_normal: false,
            boundary_edges_propagation_steps: 1,
            cavity_blur_steps: 2,
            cavity_factor: 0.5,
            start_normal_limit: 0.349_065_85,
            start_normal_falloff: 0.25,
            view_normal_limit: 1.570_796,
            view_normal_falloff: 0.25,
        }
    }
}

impl AutomaskSettings {
    pub fn boundary_edges_propagation_steps(&self) -> i32 {
        self.boundary_edges_propagation_steps
    }
    /// RNA range 1..=20.
    pub fn set_boundary_edges_propagation_steps(&mut self, v: i32) {
        self.boundary_edges_propagation_steps = v.clamp(1, BOUNDARY_MAX_STEPS);
    }
    pub fn cavity_blur_steps(&self) -> i32 {
        self.cavity_blur_steps
    }
    /// RNA range 0..=25.
    pub fn set_cavity_blur_steps(&mut self, v: i32) {
        self.cavity_blur_steps = v.clamp(0, 25);
    }
    pub fn cavity_factor(&self) -> f32 {
        self.cavity_factor
    }
    /// RNA range 0..=5.
    pub fn set_cavity_factor(&mut self, v: f32) {
        self.cavity_factor = v.clamp(0.0, 5.0);
    }
    pub fn start_normal_limit(&self) -> f32 {
        self.start_normal_limit
    }
    /// RNA range 0.0001..=pi.
    pub fn set_start_normal_limit(&mut self, v: f32) {
        self.start_normal_limit = v.clamp(0.0001, PI);
    }
    pub fn start_normal_falloff(&self) -> f32 {
        self.start_normal_falloff
    }
    /// RNA range 0.0001..=1.
    pub fn set_start_normal_falloff(&mut self, v: f32) {
        self.start_normal_falloff = v.clamp(0.0001, 1.0);
    }
    pub fn view_normal_limit(&self) -> f32 {
        self.view_normal_limit
    }
    pub fn set_view_normal_limit(&mut self, v: f32) {
        self.view_normal_limit = v.clamp(0.0001, PI);
    }
    pub fn view_normal_falloff(&self) -> f32 {
        self.view_normal_falloff
    }
    pub fn set_view_normal_falloff(&mut self, v: f32) {
        self.view_normal_falloff = v.clamp(0.0001, 1.0);
    }

    /// `_mesh_paint_automasking_icon()`: whether any of the eight listed modes is on.
    pub fn any_enabled(&self) -> bool {
        self.topology
            || self.face_sets
            || self.boundary_edges
            || self.boundary_face_sets
            || self.cavity
            || self.cavity_inverted
            || self.start_normal
            || self.view_normal
    }

    /// The icon name of the header popover button.
    pub fn header_icon(&self) -> &'static str {
        if self.any_enabled() { "CLIPUV_DEHLT" } else { "CLIPUV_HLT" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_dna() {
        let s = AutomaskSettings::default();
        assert!(!s.any_enabled());
        assert_eq!(s.boundary_edges_propagation_steps(), 1);
        assert_eq!(s.cavity_blur_steps(), 2);
        assert_eq!(s.cavity_factor(), 0.5);
        assert!((s.start_normal_limit() - 20.0_f32.to_radians()).abs() < 1e-6);
        assert_eq!(s.start_normal_falloff(), 0.25);
        assert!((s.view_normal_limit() - PI / 2.0).abs() < 1e-5);
        assert_eq!(s.view_normal_falloff(), 0.25);
    }

    #[test]
    fn icon_is_dehlt_iff_one_of_the_eight_flags_is_set() {
        assert_eq!(AutomaskSettings::default().header_icon(), "CLIPUV_HLT");
        let flags: [fn(&mut AutomaskSettings); 8] = [
            |s| s.topology = true,
            |s| s.face_sets = true,
            |s| s.boundary_edges = true,
            |s| s.boundary_face_sets = true,
            |s| s.cavity = true,
            |s| s.cavity_inverted = true,
            |s| s.start_normal = true,
            |s| s.view_normal = true,
        ];
        for set in flags {
            let mut s = AutomaskSettings::default();
            set(&mut s);
            assert_eq!(s.header_icon(), "CLIPUV_DEHLT");
        }
        // Occlusion and the custom curve alone do not count in the Python rule.
        let mut s = AutomaskSettings::default();
        s.view_occlusion = true;
        s.custom_cavity_curve = true;
        assert_eq!(s.header_icon(), "CLIPUV_HLT");
    }

    #[test]
    fn setters_clamp_to_rna_ranges() {
        let mut s = AutomaskSettings::default();
        s.set_boundary_edges_propagation_steps(0);
        assert_eq!(s.boundary_edges_propagation_steps(), 1);
        s.set_boundary_edges_propagation_steps(99);
        assert_eq!(s.boundary_edges_propagation_steps(), BOUNDARY_MAX_STEPS);
        s.set_cavity_blur_steps(-3);
        assert_eq!(s.cavity_blur_steps(), 0);
        s.set_cavity_blur_steps(40);
        assert_eq!(s.cavity_blur_steps(), 25);
        s.set_cavity_factor(9.0);
        assert_eq!(s.cavity_factor(), 5.0);
        s.set_view_normal_limit(0.0);
        assert_eq!(s.view_normal_limit(), 0.0001);
        s.set_start_normal_limit(10.0);
        assert!((s.start_normal_limit() - PI).abs() < 1e-6);
        s.set_start_normal_falloff(2.0);
        assert_eq!(s.start_normal_falloff(), 1.0);
        s.set_view_normal_falloff(-1.0);
        assert_eq!(s.view_normal_falloff(), 0.0001);
    }
}
