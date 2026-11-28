//! VIEW3D_MT_select_object and VIEW3D_MT_select_object_more_less: the Select menu of the 3D viewport header in
//! Object Mode, as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_select_object`,
//! `VIEW3D_MT_select_object_more_less`). Labels without `text=` are the operator names (`ot->name`:
//! view3d_select.cc, object_select.cc). `operator_menu_enum` rows are operator rows here; their enum submenu is
//! not available.

use crate::sculpt_menu::Row;

const fn op(idname: &'static str, label: &'static str, props: &'static [(&'static str, &'static str)]) -> Row {
    Row::Operator { idname, label, props, checkbox: None }
}

/// `VIEW3D_MT_select_object`.
pub const SELECT_OBJECT: &[Row] = &[
    op("object.select_all", "All", &[("action", "SELECT")]),
    op("object.select_all", "None", &[("action", "DESELECT")]),
    op("object.select_all", "Invert", &[("action", "INVERT")]),
    Row::Separator,
    op("view3d.select_box", "Box Select", &[]),
    op("view3d.select_circle", "Circle Select", &[]),
    op("view3d.select_lasso", "Lasso Select", &[]),
    Row::Separator,
    op("object.select_camera", "Select Active Camera", &[]),
    op("object.select_mirror", "Select Mirror", &[]),
    op("object.select_random", "Select Random", &[]),
    Row::Separator,
    Row::Menu { idname: "VIEW3D_MT_select_object_more_less", label: "More/Less" },
    Row::Separator,
    op("object.select_by_type", "Select All by Type", &[]),
    op("object.select_grouped", "Select Grouped", &[]),
    op("object.select_linked", "Select Linked", &[]),
    op("object.select_pattern", "Select Pattern...", &[]),
];

/// `VIEW3D_MT_select_object_more_less`.
pub const MORE_LESS: &[Row] = &[
    op("object.select_more", "More", &[]),
    op("object.select_less", "Less", &[]),
    Row::Separator,
    op("object.select_hierarchy", "Parent", &[("extend", "False"), ("direction", "PARENT")]),
    op("object.select_hierarchy", "Child", &[("extend", "False"), ("direction", "CHILD")]),
    Row::Separator,
    op("object.select_hierarchy", "Extend Parent", &[("extend", "True"), ("direction", "PARENT")]),
    op("object.select_hierarchy", "Extend Child", &[("extend", "True"), ("direction", "CHILD")]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(rows: &[Row]) -> Vec<&'static str> {
        rows.iter()
            .map(|r| match r {
                Row::Separator => "-",
                Row::Menu { label, .. } | Row::Operator { label, .. } => label,
            })
            .collect()
    }

    #[test]
    fn select_menu_in_blender_order() {
        assert_eq!(
            labels(SELECT_OBJECT),
            [
                "All", "None", "Invert", "-", "Box Select", "Circle Select", "Lasso Select", "-", "Select Active Camera",
                "Select Mirror", "Select Random", "-", "More/Less", "-", "Select All by Type", "Select Grouped",
                "Select Linked", "Select Pattern...",
            ]
        );
    }

    #[test]
    fn more_less_in_blender_order() {
        assert_eq!(labels(MORE_LESS), ["More", "Less", "-", "Parent", "Child", "-", "Extend Parent", "Extend Child"]);
    }
}
