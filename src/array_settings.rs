//! Array modifier settings and panel layout as in Blender (`ArrayModifierData`, DNA_modifier_types.h:396-441;
//! `panel_draw` and the subpanels, MOD_array.cc:901-1068).
//!
//! `modifiers::Modifier::Array` stores only count, one relative and one constant offset, a merge distance (0 = off)
//! and a fit length (0 = fixed count). The differences it has to Blender, found by this audit:
//! 1. Blender keeps three independent offset toggles (`offset_type` flags: relative, constant, object); the default is
//!    relative only, while the stored constant offset is {1, 0, 0}. `Modifier::array()` stores constant `[0; 3]`.
//! 2. Merging is a flag (`use_merge_vertices`, default off) with its own distance, default 0.01, plus
//!    `use_merge_vertices_cap` (First and Last Copies). `Modifier::array()` stores merge 0.0 as "off".
//! 3. Fit type is an enum (Fixed Count, Fit Length, Fit Curve); the default length is 0 and a curve object is
//!    needed for Fit Curve. Rust treats `fit > 0` as Fit Length.
//! 4. Missing: object offset, start and end caps, UV offsets (`uv_offset`, default {0, 0}).
//!
//! This module is the faithful model (defaults, panel rows and their active state) for that migration.

/// `ArrayModifierFitType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitType {
    FixedCount,
    FitLength,
    FitCurve,
}

impl FitType {
    pub const ALL: [FitType; 3] = [FitType::FixedCount, FitType::FitLength, FitType::FitCurve];
}

/// `ArrayModifierData::offset_type` (`MOD_ARR_OFF_*`): which offsets are summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffsetFlags {
    pub constant: bool,
    pub relative: bool,
    pub object: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArraySettings {
    pub fit_type: FitType,
    pub count: i32,
    pub length: f32,
    /// `offset`: constant offset, in units.
    pub offset: [f32; 3],
    /// `scale`: relative offset, in object widths.
    pub scale: [f32; 3],
    pub offset_type: OffsetFlags,
    /// `MOD_ARR_MERGE`.
    pub merge: bool,
    pub merge_dist: f32,
    /// `MOD_ARR_MERGEFINAL`.
    pub merge_final: bool,
    pub uv_offset: [f32; 2],
}

impl Default for ArraySettings {
    /// The in-class initialisers of `ArrayModifierData`.
    fn default() -> Self {
        ArraySettings {
            fit_type: FitType::FixedCount,
            count: 2,
            length: 0.0,
            offset: [1.0, 0.0, 0.0],
            scale: [1.0, 0.0, 0.0],
            offset_type: OffsetFlags { constant: false, relative: true, object: false },
            merge: false,
            merge_dist: 0.01,
            merge_final: false,
            uv_offset: [0.0, 0.0],
        }
    }
}

/// One drawn row of the modifier panel: the label (RNA name where no text override exists) and whether its column is
/// active (`col.active_set()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: &'static str,
    pub active: bool,
}

/// A panel or subpanel: its header checkbox (if any) and rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub title: &'static str,
    pub header: Option<&'static str>,
    pub rows: Vec<Row>,
}

impl ArraySettings {
    /// The sections in registration order (`panel_register`, MOD_array.cc:1048): the main panel with the fit rows,
    /// then Relative Offset, Constant Offset, Object Offset, Merge, UVs and Caps. Subpanels with a header checkbox
    /// have no title of their own ("" in `modifier_subpanel_register`).
    pub fn sections(&self) -> Vec<Section> {
        let fit_row = match self.fit_type {
            FitType::FixedCount => "Count",
            FitType::FitLength => "Length",
            FitType::FitCurve => "Curve",
        };
        let o = self.offset_type;
        let row = |label, active| Row { label, active };
        vec![
            Section { title: "", header: None, rows: vec![row("Fit Type", true), row(fit_row, true)] },
            Section { title: "", header: Some("use_relative_offset"), rows: vec![row("Factor", o.relative)] },
            Section { title: "", header: Some("use_constant_offset"), rows: vec![row("Distance", o.constant)] },
            Section { title: "", header: Some("use_object_offset"), rows: vec![row("Object", o.object)] },
            Section {
                title: "",
                header: Some("use_merge_vertices"),
                rows: vec![row("Distance", self.merge), row("First and Last Copies", self.merge)],
            },
            Section { title: "UVs", header: None, rows: vec![row("Offset U", true), row("V", true)] },
            Section { title: "Caps", header: None, rows: vec![row("Cap Start", true), row("End", true)] },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_dna_struct() {
        let a = ArraySettings::default();
        assert_eq!((a.fit_type, a.count, a.length), (FitType::FixedCount, 2, 0.0));
        assert_eq!((a.offset, a.scale), ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        assert_eq!(a.offset_type, OffsetFlags { constant: false, relative: true, object: false });
        assert!(!a.merge && !a.merge_final);
        assert_eq!(a.merge_dist, 0.01);
        assert_eq!(a.uv_offset, [0.0, 0.0]);
    }

    #[test]
    fn default_panel_matches_the_c_layout() {
        let s = ArraySettings::default().sections();
        let titles: Vec<_> = s.iter().map(|x| (x.title, x.header)).collect();
        assert_eq!(
            titles,
            [
                ("", None),
                ("", Some("use_relative_offset")),
                ("", Some("use_constant_offset")),
                ("", Some("use_object_offset")),
                ("", Some("use_merge_vertices")),
                ("UVs", None),
                ("Caps", None),
            ]
        );
        assert_eq!(s[0].rows, [Row { label: "Fit Type", active: true }, Row { label: "Count", active: true }]);
        /* Only the relative column is active by default; merge is off. */
        let active: Vec<_> = s[1..5].iter().map(|x| x.rows.iter().all(|r| r.active)).collect();
        assert_eq!(active, [true, false, false, false]);
        assert_eq!(s[4].rows.iter().map(|r| r.label).collect::<Vec<_>>(), ["Distance", "First and Last Copies"]);
        assert_eq!(s[5].rows.iter().map(|r| r.label).collect::<Vec<_>>(), ["Offset U", "V"]);
        assert_eq!(s[6].rows.iter().map(|r| r.label).collect::<Vec<_>>(), ["Cap Start", "End"]);
    }

    #[test]
    fn fit_type_picks_the_second_row() {
        let mut a = ArraySettings::default();
        a.fit_type = FitType::FitLength;
        assert_eq!(a.sections()[0].rows[1].label, "Length");
        a.fit_type = FitType::FitCurve;
        assert_eq!(a.sections()[0].rows[1].label, "Curve");
    }

    #[test]
    fn toggles_drive_the_active_state() {
        let mut a = ArraySettings::default();
        a.offset_type = OffsetFlags { constant: true, relative: false, object: true };
        a.merge = true;
        let s = a.sections();
        assert!(!s[1].rows[0].active && s[2].rows[0].active && s[3].rows[0].active);
        assert!(s[4].rows.iter().all(|r| r.active));
    }
}
