//! VIEW3D_MT_sculpt: the Sculpt menu of the 3D viewport header, as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_sculpt`, `draw`).

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Separator,
    Menu {
        idname: &'static str,
        label: &'static str,
    },
    Operator {
        idname: &'static str,
        label: &'static str,
        props: &'static [(&'static str, &'static str)],
        checkbox: Option<bool>,
    },
}

fn op(
    idname: &'static str,
    label: &'static str,
    props: &'static [(&'static str, &'static str)],
) -> Row {
    Row::Operator {
        idname,
        label,
        props,
        checkbox: None,
    }
}

/// The `sculpt_filters_types` list of Blender: (filter type, UI name).
pub const FILTERS: [(&str, &str); 9] = [
    ("SMOOTH", "Smooth"),
    ("SURFACE_SMOOTH", "Surface Smooth"),
    ("INFLATE", "Inflate"),
    ("RELAX", "Relax Topology"),
    ("RELAX_FACE_SETS", "Relax Face Sets"),
    ("SHARPEN", "Sharpen"),
    ("ENHANCE_DETAILS", "Enhance Details"),
    ("ERASE_DISPLACEMENT", "Erase Multires Displacement"),
    ("RANDOM", "Randomize"),
];

const FILTER_PROPS: [[(&str, &str); 1]; 9] = [
    [("type", "SMOOTH")],
    [("type", "SURFACE_SMOOTH")],
    [("type", "INFLATE")],
    [("type", "RELAX")],
    [("type", "RELAX_FACE_SETS")],
    [("type", "SHARPEN")],
    [("type", "ENHANCE_DETAILS")],
    [("type", "ERASE_DISPLACEMENT")],
    [("type", "RANDOM")],
];

/// Rows of the menu; `dyntopo` is `context.sculpt_object.use_dynamic_topology_sculpting`.
pub fn rows(dyntopo: bool) -> Vec<Row> {
    use Row::Separator as Sep;
    let menu = |idname, label| Row::Menu { idname, label };
    let mut r = vec![
        menu("VIEW3D_MT_sculpt_transform", "Transform"),
        Sep,
        op(
            "sculpt.face_set_change_visibility",
            "Toggle Visibility",
            &[("mode", "TOGGLE")],
        ),
        op(
            "sculpt.face_set_change_visibility",
            "Hide Active Face Set",
            &[("mode", "HIDE_ACTIVE")],
        ),
        op("paint.hide_show_all", "Show All", &[("action", "SHOW")]),
        op("paint.visibility_invert", "Invert Visible", &[]),
        op(
            "paint.hide_show_masked",
            "Hide Masked",
            &[("action", "HIDE")],
        ),
        op(
            "paint.visibility_filter",
            "Grow Visibility",
            &[("action", "GROW")],
        ),
        op(
            "paint.visibility_filter",
            "Shrink Visibility",
            &[("action", "SHRINK")],
        ),
        menu("VIEW3D_MT_sculpt_showhide", "Show/Hide"),
        Sep,
        op(
            "sculpt.face_set_edit",
            "Fair Positions",
            &[("mode", "FAIR_POSITIONS")],
        ),
        op(
            "sculpt.face_set_edit",
            "Fair Tangency",
            &[("mode", "FAIR_TANGENCY")],
        ),
        op("sculpt.project_line_gesture", "Line Project", &[]),
        menu("VIEW3D_MT_sculpt_trim", "Trim/Add"),
        Sep,
    ];
    for (i, (_, name)) in FILTERS.iter().enumerate() {
        r.push(op("sculpt.mesh_filter", name, &FILTER_PROPS[i]));
    }
    r.push(Sep);
    r.push(op(
        "sculpt.color_filter",
        "Set Vertex Color",
        &[
            ("type", "FILL"),
            ("strength", "1.0"),
            ("use_immediate", "True"),
        ],
    ));
    r.push(op("paint.sample_color", "Sample Color", &[]));
    r.push(Sep);
    r.push(menu("VIEW3D_MT_sculpt_set_pivot", "Set Pivot"));
    r.push(Sep);
    r.push(op("sculpt.optimize", "Rebuild BVH", &[]));
    r.push(Row::Operator {
        idname: "sculpt.dynamic_topology_toggle",
        label: "Dynamic Topology",
        props: &[],
        checkbox: Some(dyntopo),
    });
    r.push(Sep);
    r.push(op("object.transfer_mode", "Transfer Sculpt Mode", &[]));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(r: &Row) -> &str {
        match r {
            Row::Separator => "-",
            Row::Menu { label, .. } | Row::Operator { label, .. } => label,
        }
    }

    #[test]
    fn order_and_labels() {
        let r = rows(false);
        let l: Vec<&str> = r.iter().map(label).collect();
        assert_eq!(
            &l[..10],
            [
                "Transform",
                "-",
                "Toggle Visibility",
                "Hide Active Face Set",
                "Show All",
                "Invert Visible",
                "Hide Masked",
                "Grow Visibility",
                "Shrink Visibility",
                "Show/Hide"
            ]
        );
        assert_eq!(
            &l[10..16],
            [
                "-",
                "Fair Positions",
                "Fair Tangency",
                "Line Project",
                "Trim/Add",
                "-"
            ]
        );
        assert_eq!(l.last(), Some(&"Transfer Sculpt Mode"));
    }

    #[test]
    fn nine_mesh_filters_with_type() {
        let r = rows(false);
        let f: Vec<_> = r
            .iter()
            .filter_map(|x| match x {
                Row::Operator {
                    idname: "sculpt.mesh_filter",
                    props,
                    ..
                } => Some(props[0].1),
                _ => None,
            })
            .collect();
        let want: Vec<_> = FILTERS.iter().map(|f| f.0).collect();
        assert_eq!(f, want);
    }

    #[test]
    fn dyntopo_checkbox_follows_state() {
        let get = |on| {
            rows(on).into_iter().find_map(|x| match x {
                Row::Operator {
                    idname: "sculpt.dynamic_topology_toggle",
                    checkbox,
                    ..
                } => checkbox,
                _ => None,
            })
        };
        assert_eq!(get(true), Some(true));
        assert_eq!(get(false), Some(false));
    }
}
