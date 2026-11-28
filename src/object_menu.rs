//! VIEW3D_MT_object: the Object menu of the 3D viewport header in Object Mode, as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_object`, `draw`). Submenu labels are the
//! `bl_label` of their class, operator labels the `text=` of the call or else `ot->name` (object_add.cc,
//! object_edit.cc, object_ops.cc). `operator_menu_enum` (Set Origin) is an operator row here.

use crate::sculpt_menu::Row;

const fn op(idname: &'static str, label: &'static str, props: &'static [(&'static str, &'static str)]) -> Row {
    Row::Operator { idname, label, props, checkbox: None }
}

const fn menu(idname: &'static str, label: &'static str) -> Row {
    Row::Menu { idname, label }
}

/// `ob.type in {'MESH', 'CURVE', 'FONT'}` of `context.object`, the condition of Shade Auto Smooth.
pub fn auto_smooth_type(ob_type: Option<&str>) -> bool {
    matches!(ob_type, Some("MESH" | "CURVE" | "FONT"))
}

/// Rows of the menu; `ob_type` is the RNA type of the active object, if any.
pub fn rows(ob_type: Option<&str>) -> Vec<Row> {
    use Row::Separator as Sep;
    let mut r = vec![
        menu("VIEW3D_MT_transform_object", "Transform"),
        op("object.origin_set", "Set Origin", &[]),
        menu("VIEW3D_MT_mirror", "Mirror"),
        menu("VIEW3D_MT_object_clear", "Clear"),
        menu("VIEW3D_MT_object_apply", "Apply"),
        menu("VIEW3D_MT_snap", "Snap"),
        Sep,
        op("object.duplicate_move", "Duplicate Objects", &[]),
        op("object.duplicate_move_linked", "Duplicate Linked", &[]),
        op("object.join", "Join", &[]),
        Sep,
        op("view3d.copybuffer", "Copy Objects", &[]),
        op("view3d.pastebuffer", "Paste Objects", &[]),
        Sep,
        menu("VIEW3D_MT_object_asset", "Asset"),
        menu("VIEW3D_MT_object_collection", "Collection"),
        Sep,
        menu("VIEW3D_MT_object_liboverride", "Library Override"),
        menu("VIEW3D_MT_object_relations", "Relations"),
        menu("VIEW3D_MT_object_parent", "Parent"),
        menu("VIEW3D_MT_object_modifiers", "Modifiers"),
        menu("VIEW3D_MT_object_constraints", "Constraints"),
        menu("VIEW3D_MT_object_track", "Track"),
        menu("VIEW3D_MT_make_links", "Link/Transfer Data"),
        Sep,
        op("object.shade_smooth", "Shade Smooth", &[]),
    ];
    if auto_smooth_type(ob_type) {
        r.push(op("object.shade_auto_smooth", "Shade Auto Smooth", &[]));
    }
    r.extend([
        op("object.shade_flat", "Shade Flat", &[]),
        Sep,
        menu("VIEW3D_MT_object_animation", "Animation"),
        menu("VIEW3D_MT_object_rigid_body", "Rigid Body"),
        Sep,
        menu("VIEW3D_MT_object_quick_effects", "Quick Effects"),
        Sep,
        menu("VIEW3D_MT_object_convert", "Convert"),
        Sep,
        menu("VIEW3D_MT_object_showhide", "Show/Hide"),
        menu("VIEW3D_MT_object_cleanup", "Clean Up"),
        Sep,
        op("object.delete", "Delete Global", &[("use_global", "True")]),
        op("object.delete", "Delete", &[("use_global", "False")]),
    ]);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(ob: Option<&str>) -> Vec<&'static str> {
        rows(ob)
            .iter()
            .map(|r| match r {
                Row::Separator => "-",
                Row::Menu { label, .. } | Row::Operator { label, .. } => label,
            })
            .collect()
    }

    #[test]
    fn labels_in_blender_order() {
        assert_eq!(
            labels(Some("MESH")),
            [
                "Transform", "Set Origin", "Mirror", "Clear", "Apply", "Snap", "-", "Duplicate Objects",
                "Duplicate Linked", "Join", "-", "Copy Objects", "Paste Objects", "-", "Asset", "Collection", "-",
                "Library Override", "Relations", "Parent", "Modifiers", "Constraints", "Track", "Link/Transfer Data", "-",
                "Shade Smooth", "Shade Auto Smooth", "Shade Flat", "-", "Animation", "Rigid Body", "-", "Quick Effects",
                "-", "Convert", "-", "Show/Hide", "Clean Up", "-", "Delete Global", "Delete",
            ]
        );
    }

    #[test]
    fn shade_auto_smooth_only_for_mesh_curve_and_font() {
        for t in ["MESH", "CURVE", "FONT"] {
            assert!(labels(Some(t)).contains(&"Shade Auto Smooth"), "{t}");
        }
        for t in [Some("EMPTY"), Some("CAMERA"), None] {
            assert!(!labels(t).contains(&"Shade Auto Smooth"), "{t:?}");
        }
        assert_eq!(labels(Some("MESH")).len(), labels(None).len() + 1);
    }

    #[test]
    fn delete_rows_set_use_global() {
        let r = rows(None);
        let d: Vec<_> = r
            .iter()
            .filter_map(|x| match x {
                Row::Operator { idname: "object.delete", props, .. } => Some(*props),
                _ => None,
            })
            .collect();
        assert_eq!(d, [&[("use_global", "True")][..], &[("use_global", "False")][..]]);
    }
}
