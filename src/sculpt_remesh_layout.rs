//! Row model of `VIEW3D_PT_sculpt_voxel_remesh` (scripts/startup/bl_ui/space_view3d_toolbar.py).
//! Labels and ranges from the Mesh RNA (`remesh_voxel_size`, `remesh_voxel_adaptivity`, `use_remesh_fix_poles`,
//! `use_remesh_preserve_volume`, `use_remesh_preserve_attributes` in rna_mesh.cc). Pure data; the popover draws it.

/// `(min, max)` of the UI range of a float property of the panel.
pub fn ui_range(id: &str) -> Option<(f32, f32)> {
    match id {
        "remesh_voxel_size" => Some((0.0001, f32::MAX)),
        "remesh_voxel_adaptivity" => Some((0.0, 1.0)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// `row.prop(mesh, id)`, with the `sculpt.sample_detail_size` eyedropper (mode VOXEL) when `eyedropper`.
    Prop { id: &'static str, text: &'static str, eyedropper: bool },
    /// `col.prop(mesh, id)` of a checkbox, `heading` is the column heading (`layout.column(heading="Preserve")`)
    /// shown on the first row of that column.
    Check { id: &'static str, text: &'static str, heading: Option<&'static str> },
    /// `layout.operator("object.voxel_remesh", text="Remesh")`.
    RemeshButton,
}

pub fn layout() -> Vec<Row> {
    vec![
        Row::Prop { id: "remesh_voxel_size", text: "Voxel Size", eyedropper: true },
        Row::Prop { id: "remesh_voxel_adaptivity", text: "Adaptivity", eyedropper: false },
        Row::Check { id: "use_remesh_fix_poles", text: "Fix Poles", heading: None },
        Row::Check { id: "use_remesh_preserve_volume", text: "Volume", heading: Some("Preserve") },
        Row::Check { id: "use_remesh_preserve_attributes", text: "Attributes", heading: None },
        Row::RemeshButton,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_the_python_order() {
        let ids: Vec<_> = layout()
            .iter()
            .map(|r| match r {
                Row::Prop { id, .. } | Row::Check { id, .. } => *id,
                Row::RemeshButton => "remesh",
            })
            .collect();
        assert_eq!(
            ids,
            ["remesh_voxel_size", "remesh_voxel_adaptivity", "use_remesh_fix_poles", "use_remesh_preserve_volume", "use_remesh_preserve_attributes", "remesh"]
        );
    }

    #[test]
    fn only_voxel_size_has_eyedropper_and_preserve_has_heading() {
        let r = layout();
        assert_eq!(r.iter().filter(|x| matches!(x, Row::Prop { eyedropper: true, .. })).count(), 1);
        assert!(matches!(r[3], Row::Check { heading: Some("Preserve"), text: "Volume", .. }));
        assert!(matches!(r[4], Row::Check { heading: None, text: "Attributes", .. }));
    }

    #[test]
    fn ui_ranges() {
        assert_eq!(ui_range("remesh_voxel_adaptivity"), Some((0.0, 1.0)));
        assert_eq!(ui_range("remesh_voxel_size").map(|r| r.0), Some(0.0001));
        assert_eq!(ui_range("use_remesh_fix_poles"), None);
    }
}
