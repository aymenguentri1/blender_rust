//! VIEW3D_MT_mask: the Mask menu of the 3D viewport header (sculpt mode), as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_mask`, `draw`). The trailing
//! `template_node_operator_asset_menu_items` (node group assets) is not ported.

use crate::sculpt_menu::Row;

const fn op(
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

pub const ROWS: &[Row] = &[
    op(
        "paint.mask_flood_fill",
        "Fill Mask",
        &[("mode", "VALUE"), ("value", "1")],
    ),
    op(
        "paint.mask_flood_fill",
        "Clear Mask",
        &[("mode", "VALUE"), ("value", "0")],
    ),
    op(
        "paint.mask_flood_fill",
        "Invert Mask",
        &[("mode", "INVERT")],
    ),
    Row::Separator,
    op(
        "paint.mask_box_gesture",
        "Box Mask",
        &[("mode", "VALUE"), ("value", "0")],
    ),
    op("paint.mask_lasso_gesture", "Lasso Mask", &[]),
    op("paint.mask_line_gesture", "Line Mask", &[]),
    op("paint.mask_polyline_gesture", "Polyline Mask", &[]),
    Row::Separator,
    op(
        "sculpt.mask_filter",
        "Smooth Mask",
        &[("filter_type", "SMOOTH")],
    ),
    op(
        "sculpt.mask_filter",
        "Sharpen Mask",
        &[("filter_type", "SHARPEN")],
    ),
    op(
        "sculpt.mask_filter",
        "Grow Mask",
        &[("filter_type", "GROW")],
    ),
    op(
        "sculpt.mask_filter",
        "Shrink Mask",
        &[("filter_type", "SHRINK")],
    ),
    op(
        "sculpt.mask_filter",
        "Increase Contrast",
        &[
            ("filter_type", "CONTRAST_INCREASE"),
            ("auto_iteration_count", "False"),
        ],
    ),
    op(
        "sculpt.mask_filter",
        "Decrease Contrast",
        &[
            ("filter_type", "CONTRAST_DECREASE"),
            ("auto_iteration_count", "False"),
        ],
    ),
    Row::Separator,
    op(
        "sculpt.expand",
        "Expand Mask by Topology",
        &[
            ("target", "MASK"),
            ("falloff_type", "GEODESIC"),
            ("invert", "False"),
            ("use_auto_mask", "False"),
            ("use_mask_preserve", "True"),
        ],
    ),
    op(
        "sculpt.expand",
        "Expand Mask by Normals",
        &[
            ("target", "MASK"),
            ("falloff_type", "NORMALS"),
            ("invert", "False"),
            ("use_mask_preserve", "True"),
        ],
    ),
    Row::Separator,
    op("sculpt.paint_mask_extract", "Mask Extract", &[]),
    Row::Separator,
    op(
        "sculpt.paint_mask_slice",
        "Mask Slice",
        &[("fill_holes", "False"), ("new_object", "False")],
    ),
    op(
        "sculpt.paint_mask_slice",
        "Mask Slice and Fill Holes",
        &[("new_object", "False")],
    ),
    op("sculpt.paint_mask_slice", "Mask Slice to New Object", &[]),
    Row::Separator,
    op(
        "sculpt.mask_from_cavity",
        "Mask from Cavity",
        &[("settings_source", "OPERATOR")],
    ),
    op(
        "sculpt.mask_from_boundary",
        "Mask from Mesh Boundary",
        &[("settings_source", "OPERATOR"), ("boundary_mode", "MESH")],
    ),
    op(
        "sculpt.mask_from_boundary",
        "Mask from Face Sets Boundary",
        &[
            ("settings_source", "OPERATOR"),
            ("boundary_mode", "FACE_SETS"),
        ],
    ),
    op("sculpt.mask_by_color", "Mask by Color", &[]),
    Row::Separator,
    Row::Menu {
        idname: "VIEW3D_MT_random_mask",
        label: "Random Mask",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn label(r: &Row) -> &'static str {
        match r {
            Row::Separator => "-",
            Row::Menu { label, .. } | Row::Operator { label, .. } => label,
        }
    }

    #[test]
    fn labels_in_blender_order() {
        let l: Vec<_> = ROWS.iter().map(label).collect();
        assert_eq!(l.len(), 31);
        assert_eq!(
            &l[..8],
            [
                "Fill Mask",
                "Clear Mask",
                "Invert Mask",
                "-",
                "Box Mask",
                "Lasso Mask",
                "Line Mask",
                "Polyline Mask"
            ]
        );
        assert_eq!(
            &l[9..15],
            [
                "Smooth Mask",
                "Sharpen Mask",
                "Grow Mask",
                "Shrink Mask",
                "Increase Contrast",
                "Decrease Contrast"
            ]
        );
        assert_eq!(l[l.len() - 1], "Random Mask");
        assert_eq!(l.iter().filter(|x| **x == "-").count(), 7);
    }

    #[test]
    fn operator_props() {
        let find = |name: &str| {
            ROWS.iter()
                .find_map(|r| match r {
                    Row::Operator { label, props, .. } if *label == name => Some(*props),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(find("Fill Mask"), &[("mode", "VALUE"), ("value", "1")]);
        assert_eq!(find("Clear Mask"), &[("mode", "VALUE"), ("value", "0")]);
        assert_eq!(find("Invert Mask"), &[("mode", "INVERT")]);
        assert!(find("Increase Contrast").contains(&("auto_iteration_count", "False")));
        assert!(find("Expand Mask by Topology").contains(&("falloff_type", "GEODESIC")));
        assert!(find("Mask from Face Sets Boundary").contains(&("boundary_mode", "FACE_SETS")));
    }
}
