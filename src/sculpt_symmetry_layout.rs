//! Row model + state of `VIEW3D_PT_sculpt_symmetry` (scripts/startup/bl_ui/space_view3d_toolbar.py).
//! RNA: Mesh `use_mirror_x/y/z`, `use_radial_symmetry`, `radial_symmetry` (range 1..64, rna_mesh.cc); Sculpt `lock_*`,
//! `tile_*`, `use_symmetry_feather`, `tile_offset`, `symmetrize_direction` (rna_sculpt_paint.cc, items
//! `rna_enum_symmetrize_direction_items`). Pure data; the popover draws it.

pub const RADIAL_MIN: i32 = 1;
pub const RADIAL_MAX: i32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymmetrizeDirection {
    NegativeX,
    PositiveX,
    NegativeY,
    PositiveY,
    NegativeZ,
    PositiveZ,
}

impl SymmetrizeDirection {
    pub const ALL: [SymmetrizeDirection; 6] = [
        SymmetrizeDirection::NegativeX,
        SymmetrizeDirection::PositiveX,
        SymmetrizeDirection::NegativeY,
        SymmetrizeDirection::PositiveY,
        SymmetrizeDirection::NegativeZ,
        SymmetrizeDirection::PositiveZ,
    ];
    pub fn label(self) -> &'static str {
        ["-X to +X", "+X to -X", "-Y to +Y", "+Y to -Y", "-Z to +Z", "+Z to -Z"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymmetryState {
    pub mirror: [bool; 3],
    pub lock: [bool; 3],
    pub tile: [bool; 3],
    pub feather: bool,
    pub radial: bool,
    pub radial_count: i32,
    pub direction: SymmetrizeDirection,
}

impl Default for SymmetryState {
    fn default() -> Self {
        SymmetryState {
            mirror: [false; 3],
            lock: [false; 3],
            tile: [false; 3],
            feather: false,
            radial: false,
            radial_count: 1,
            direction: SymmetrizeDirection::NegativeX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// `layout.row(align=True, heading=..)` of three XYZ toggle buttons (`toggle=True`).
    Axes { heading: &'static str, ids: [&'static str; 3], on: [bool; 3] },
    /// `layout.prop(sculpt, "use_symmetry_feather", text="Feather")`.
    Feather { on: bool },
    /// `layout.panel("radial_symmetry_panel", default_closed=True)`: header checkbox "Radial Symmetry".
    RadialHeader { on: bool },
    /// Panel body `col.prop(mesh, "radial_symmetry", text="Count")`, `enabled = use_radial_symmetry`.
    RadialCount { value: i32, enabled: bool },
    /// `layout.prop(sculpt, "tile_offset", text="Tile Offset")`.
    TileOffset,
    Separator,
    Label(&'static str),
    /// `layout.prop(sculpt, "symmetrize_direction")`.
    Direction(SymmetrizeDirection),
    /// `layout.prop(operator_properties_last("sculpt.symmetrize"), "merge_tolerance")`.
    MergeTolerance,
    /// `layout.operator("sculpt.symmetrize")`.
    SymmetrizeButton,
}

/// `radial_open` is whether the (default closed) radial subpanel is expanded.
pub fn layout(s: &SymmetryState, radial_open: bool) -> Vec<Row> {
    let mut r = vec![
        Row::Axes { heading: "Mirror", ids: ["use_mirror_x", "use_mirror_y", "use_mirror_z"], on: s.mirror },
        Row::Axes { heading: "Lock", ids: ["lock_x", "lock_y", "lock_z"], on: s.lock },
        Row::Axes { heading: "Tiling", ids: ["tile_x", "tile_y", "tile_z"], on: s.tile },
        Row::Feather { on: s.feather },
        Row::RadialHeader { on: s.radial },
    ];
    if radial_open {
        r.push(Row::RadialCount { value: s.radial_count, enabled: s.radial });
    }
    r.extend([
        Row::TileOffset,
        Row::Separator,
        Row::Label("Symmetrize"),
        Row::Direction(s.direction),
        Row::MergeTolerance,
        Row::SymmetrizeButton,
    ]);
    r
}

/// Click on a toggle row; returns false for an unknown id.
pub fn toggle(s: &mut SymmetryState, id: &str) -> bool {
    let axis = |c: char| (c as u8 - b'x') as usize;
    match id {
        "use_mirror_x" | "use_mirror_y" | "use_mirror_z" => s.mirror[axis(id.chars().last().unwrap())] ^= true,
        "lock_x" | "lock_y" | "lock_z" => s.lock[axis(id.chars().last().unwrap())] ^= true,
        "tile_x" | "tile_y" | "tile_z" => s.tile[axis(id.chars().last().unwrap())] ^= true,
        "use_symmetry_feather" => s.feather ^= true,
        "use_radial_symmetry" => s.radial ^= true,
        _ => return false,
    }
    true
}

/// `radial_symmetry` is an int with range 1..64.
pub fn set_radial_count(s: &mut SymmetryState, v: i32) {
    s.radial_count = v.clamp(RADIAL_MIN, RADIAL_MAX);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_python_order() {
        let r = layout(&SymmetryState::default(), false);
        assert!(matches!(r[0], Row::Axes { heading: "Mirror", .. }));
        assert!(matches!(r[1], Row::Axes { heading: "Lock", .. }));
        assert!(matches!(r[2], Row::Axes { heading: "Tiling", .. }));
        assert_eq!(r[3], Row::Feather { on: false });
        assert_eq!(r[4], Row::RadialHeader { on: false });
        assert_eq!(r[5], Row::TileOffset);
        assert_eq!(r[7], Row::Label("Symmetrize"));
        assert_eq!(r.last(), Some(&Row::SymmetrizeButton));
        assert_eq!(r.len(), 11);
    }

    #[test]
    fn radial_count_only_when_open_and_disabled_unless_on() {
        let mut s = SymmetryState::default();
        let r = layout(&s, true);
        assert_eq!(r[5], Row::RadialCount { value: 1, enabled: false });
        assert!(toggle(&mut s, "use_radial_symmetry"));
        assert_eq!(layout(&s, true)[5], Row::RadialCount { value: 1, enabled: true });
        assert_eq!(layout(&s, true).len(), 12);
    }

    #[test]
    fn toggles_hit_the_right_axis() {
        let mut s = SymmetryState::default();
        assert!(toggle(&mut s, "use_mirror_y"));
        assert!(toggle(&mut s, "lock_z"));
        assert!(toggle(&mut s, "tile_x"));
        assert!(toggle(&mut s, "use_symmetry_feather"));
        assert_eq!((s.mirror, s.lock, s.tile, s.feather), ([false, true, false], [false, false, true], [true, false, false], true));
        assert!(toggle(&mut s, "use_mirror_y"));
        assert_eq!(s.mirror, [false; 3]);
        assert!(!toggle(&mut s, "bogus"));
    }

    #[test]
    fn radial_count_is_clamped_and_directions_labelled() {
        let mut s = SymmetryState::default();
        set_radial_count(&mut s, 100);
        assert_eq!(s.radial_count, 64);
        set_radial_count(&mut s, -3);
        assert_eq!(s.radial_count, 1);
        assert_eq!(SymmetrizeDirection::PositiveZ.label(), "+Z to -Z");
        assert_eq!(SymmetrizeDirection::NegativeY.label(), "-Y to +Y");
    }
}
