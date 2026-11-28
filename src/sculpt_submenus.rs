//! Sculpt submenus of the 3D viewport header, as pure layout rows: VIEW3D_MT_sculpt_transform,
//! VIEW3D_MT_sculpt_showhide, VIEW3D_MT_sculpt_trim and VIEW3D_MT_sculpt_set_pivot.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`draw` of each class). Operators drawn without
//! `text=` use their registered name (`transform.translate` is "Move", `transform.rotate` is "Rotate").

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

pub const TRANSFORM: &[Row] = &[
    op("transform.translate", "Move", &[]),
    op("transform.rotate", "Rotate", &[]),
    op("transform.resize", "Scale", &[]),
    Row::Separator,
    op("sculpt.mesh_filter", "To Sphere", &[("type", "SPHERE")]),
];

pub const SHOWHIDE: &[Row] = &[
    op("paint.hide_show", "Box Hide", &[("action", "HIDE")]),
    op(
        "paint.hide_show_lasso_gesture",
        "Lasso Hide",
        &[("action", "HIDE")],
    ),
    op(
        "paint.hide_show_line_gesture",
        "Line Hide",
        &[("action", "HIDE")],
    ),
    op(
        "paint.hide_show_polyline_gesture",
        "Polyline Hide",
        &[("action", "HIDE")],
    ),
    Row::Separator,
    op("paint.hide_show", "Box Show", &[("action", "SHOW")]),
    op(
        "paint.hide_show_lasso_gesture",
        "Lasso Show",
        &[("action", "SHOW")],
    ),
    op(
        "paint.hide_show_line_gesture",
        "Line Show",
        &[("action", "SHOW")],
    ),
    op(
        "paint.hide_show_polyline_gesture",
        "Polyline Show",
        &[("action", "SHOW")],
    ),
];

pub const TRIM: &[Row] = &[
    op(
        "sculpt.trim_box_gesture",
        "Box Trim",
        &[("trim_mode", "DIFFERENCE")],
    ),
    op(
        "sculpt.trim_lasso_gesture",
        "Lasso Trim",
        &[("trim_mode", "DIFFERENCE")],
    ),
    op(
        "sculpt.trim_line_gesture",
        "Line Trim",
        &[("trim_mode", "DIFFERENCE")],
    ),
    op(
        "sculpt.trim_polyline_gesture",
        "Polyline Trim",
        &[("trim_mode", "DIFFERENCE")],
    ),
    Row::Separator,
    op(
        "sculpt.trim_box_gesture",
        "Box Add",
        &[("trim_mode", "JOIN")],
    ),
    op(
        "sculpt.trim_lasso_gesture",
        "Lasso Add",
        &[("trim_mode", "JOIN")],
    ),
    op(
        "sculpt.trim_polyline_gesture",
        "Polyline Add",
        &[("trim_mode", "JOIN")],
    ),
];

pub const SET_PIVOT: &[Row] = &[
    op(
        "sculpt.set_pivot_position",
        "Pivot to Origin",
        &[("mode", "ORIGIN")],
    ),
    op(
        "sculpt.set_pivot_position",
        "Pivot to Unmasked",
        &[("mode", "UNMASKED")],
    ),
    op(
        "sculpt.set_pivot_position",
        "Pivot to Mask Border",
        &[("mode", "BORDER")],
    ),
    op(
        "sculpt.set_pivot_position",
        "Pivot to Active Vertex",
        &[("mode", "ACTIVE")],
    ),
    op(
        "sculpt.set_pivot_position",
        "Pivot to Surface Under Cursor",
        &[("mode", "SURFACE")],
    ),
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

    fn labels(rows: &[Row]) -> Vec<&'static str> {
        rows.iter().map(label).collect()
    }

    #[test]
    fn transform_rows() {
        assert_eq!(
            labels(TRANSFORM),
            ["Move", "Rotate", "Scale", "-", "To Sphere"]
        );
    }

    #[test]
    fn showhide_rows() {
        assert_eq!(
            labels(SHOWHIDE),
            [
                "Box Hide",
                "Lasso Hide",
                "Line Hide",
                "Polyline Hide",
                "-",
                "Box Show",
                "Lasso Show",
                "Line Show",
                "Polyline Show"
            ]
        );
        assert!(
            matches!(&SHOWHIDE[7], Row::Operator { idname: "paint.hide_show_line_gesture", props, .. } if props == &[("action", "SHOW")])
        );
    }

    #[test]
    fn trim_has_no_line_add() {
        assert_eq!(
            labels(TRIM),
            [
                "Box Trim",
                "Lasso Trim",
                "Line Trim",
                "Polyline Trim",
                "-",
                "Box Add",
                "Lasso Add",
                "Polyline Add"
            ]
        );
        assert!(
            matches!(&TRIM[5], Row::Operator { props, .. } if props == &[("trim_mode", "JOIN")])
        );
    }

    #[test]
    fn pivot_modes() {
        let m: Vec<_> = SET_PIVOT
            .iter()
            .map(|r| {
                if let Row::Operator { props, .. } = r {
                    props[0].1
                } else {
                    ""
                }
            })
            .collect();
        assert_eq!(m, ["ORIGIN", "UNMASKED", "BORDER", "ACTIVE", "SURFACE"]);
    }
}
