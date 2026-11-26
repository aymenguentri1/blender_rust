//! Mirror modifier settings and panel state (`MirrorModifierData`, DNA_modifier_types.h:448-473; RNA
//! `rna_def_modifier_mirror`, rna_modifier.cc:3212; panel `MOD_mirror.cc` `panel_draw`/`data_panel_draw`).
//! Audit: `Modifier::Mirror` in modifiers.rs only has axes, merge and threshold; the rest of the Blender
//! properties live here so the panel and evaluator can be wired to them in small steps.

/// `RNA_def_property_range(prop, 0, FLT_MAX)` of `merge_threshold` and `bisect_threshold`.
pub const THRESHOLD_MAX: f32 = f32::MAX;
/// `mirror_offset_u/v` range.
pub const MIRROR_OFFSET_RANGE: (f32, f32) = (-1.0, 1.0);
/// `offset_u/v` range (the UI range is -1..1, typed values may go further).
pub const COPY_OFFSET_RANGE: (f32, f32) = (-10000.0, 10000.0);

#[derive(Clone, Debug, PartialEq)]
pub struct MirrorSettings {
    /// `use_axis` (`MOD_MIR_AXIS_X..Z`).
    pub use_axis: [bool; 3],
    /// `use_bisect_axis` (`MOD_MIR_BISECT_AXIS_X..Z`).
    pub use_bisect_axis: [bool; 3],
    /// `use_bisect_flip_axis` (`MOD_MIR_BISECT_FLIP_AXIS_X..Z`).
    pub use_bisect_flip_axis: [bool; 3],
    /// `use_clip`, shown as "Clipping" (`MOD_MIR_CLIPPING`).
    pub use_clip: bool,
    /// `use_mirror_merge` is the negation of `MOD_MIR_NO_MERGE`.
    pub use_mirror_merge: bool,
    /// `merge_threshold` (`tolerance`).
    pub merge_threshold: f32,
    pub bisect_threshold: f32,
    /// `use_mirror_vertex_groups` (`MOD_MIR_VGROUP`).
    pub use_mirror_vertex_groups: bool,
    pub use_mirror_u: bool,
    pub use_mirror_v: bool,
    pub use_mirror_udim: bool,
    /// `mirror_offset_u/v` (`uv_offset`).
    pub mirror_offset: [f32; 2],
    /// `offset_u/v` (`uv_offset_copy`).
    pub offset: [f32; 2],
}

impl Default for MirrorSettings {
    /// `DNA_modifier_types.h:473`: `flag = MOD_MIR_AXIS_X | MOD_MIR_VGROUP`, tolerance and bisect threshold 0.001.
    fn default() -> Self {
        MirrorSettings {
            use_axis: [true, false, false],
            use_bisect_axis: [false; 3],
            use_bisect_flip_axis: [false; 3],
            use_clip: false,
            use_mirror_merge: true,
            merge_threshold: 0.001,
            bisect_threshold: 0.001,
            use_mirror_vertex_groups: true,
            use_mirror_u: false,
            use_mirror_v: false,
            use_mirror_udim: false,
            mirror_offset: [0.0; 2],
            offset: [0.0; 2],
        }
    }
}

impl MirrorSettings {
    pub fn set_merge_threshold(&mut self, v: f32) {
        self.merge_threshold = v.clamp(0.0, THRESHOLD_MAX);
    }

    pub fn set_bisect_threshold(&mut self, v: f32) {
        self.bisect_threshold = v.clamp(0.0, THRESHOLD_MAX);
    }

    pub fn set_mirror_offset(&mut self, i: usize, v: f32) {
        self.mirror_offset[i.min(1)] = v.clamp(MIRROR_OFFSET_RANGE.0, MIRROR_OFFSET_RANGE.1);
    }

    pub fn set_offset(&mut self, i: usize, v: f32) {
        self.offset[i.min(1)] = v.clamp(COPY_OFFSET_RANGE.0, COPY_OFFSET_RANGE.1);
    }

    /// `has_bisect` in `panel_draw`: any bisect axis on. Greys the Flip row and "Bisect Distance".
    pub fn has_bisect(&self) -> bool {
        self.use_bisect_axis.iter().any(|&b| b)
    }

    /// `sub->active_set(use_mirror_merge)` on the merge threshold.
    pub fn merge_threshold_active(&self) -> bool {
        self.use_mirror_merge
    }

    /// `sub->active_set(use_mirror_u/v)` on the Mirror U / V offset sliders.
    pub fn mirror_offset_active(&self, i: usize) -> bool {
        if i == 0 { self.use_mirror_u } else { self.use_mirror_v }
    }
}

/// Labels of the main panel rows in `panel_draw` order, then the "Data" subpanel rows in `data_panel_draw` order.
pub const MAIN_ROWS: [&str; 6] = ["Axis", "Bisect", "Flip", "Mirror Object", "Clipping", "Merge"];
pub const BISECT_DISTANCE_ROW: &str = "Bisect Distance";
pub const DATA_ROWS: [&str; 6] = ["Mirror U", "V", "Offset U", "V", "Vertex Groups", "Flip UDIM"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_dna() {
        let m = MirrorSettings::default();
        assert_eq!(m.use_axis, [true, false, false]);
        assert!(m.use_mirror_merge && m.use_mirror_vertex_groups && !m.use_clip);
        assert_eq!((m.merge_threshold, m.bisect_threshold), (0.001, 0.001));
        assert!(!m.has_bisect());
    }

    #[test]
    fn thresholds_and_offsets_clamp_to_rna_ranges() {
        let mut m = MirrorSettings::default();
        m.set_merge_threshold(-1.0);
        m.set_bisect_threshold(-0.5);
        assert_eq!((m.merge_threshold, m.bisect_threshold), (0.0, 0.0));
        m.set_mirror_offset(0, 3.0);
        m.set_mirror_offset(1, -3.0);
        assert_eq!(m.mirror_offset, [1.0, -1.0]);
        m.set_offset(0, 20000.0);
        m.set_offset(1, 0.5);
        assert_eq!(m.offset, [10000.0, 0.5]);
    }

    #[test]
    fn active_states_follow_panel_draw() {
        let mut m = MirrorSettings::default();
        assert!(m.merge_threshold_active());
        m.use_mirror_merge = false;
        assert!(!m.merge_threshold_active());
        m.use_bisect_axis[2] = true;
        assert!(m.has_bisect());
        assert!(!m.mirror_offset_active(0) && !m.mirror_offset_active(1));
        m.use_mirror_v = true;
        assert!(!m.mirror_offset_active(0) && m.mirror_offset_active(1));
    }

    #[test]
    fn rows_match_blender_panel_order() {
        assert_eq!(MAIN_ROWS, ["Axis", "Bisect", "Flip", "Mirror Object", "Clipping", "Merge"]);
        assert_eq!(DATA_ROWS[0], "Mirror U");
        assert_eq!(BISECT_DISTANCE_ROW, "Bisect Distance");
    }
}
