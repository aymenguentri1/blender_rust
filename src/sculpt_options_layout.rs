//! Row model of `VIEW3D_PT_sculpt_options` and its subpanel `VIEW3D_PT_sculpt_options_gravity`
//! (scripts/startup/bl_ui/space_view3d_toolbar.py). Labels of props without a `text=` override come from
//! rna_sculpt_paint.cc (`show_low_resolution` "Fast Navigate", `use_sculpt_delay_updates` "Delay Viewport Updates",
//! `use_deform_only` "Use Deform Only", `gravity_object` "Orientation"). Pure data; the popover draws it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// `layout.column(heading=..)` heading label, shown above the first checkbox of the column.
    Heading(&'static str),
    Separator,
    /// `col.prop(owner, id, text=text)` checkbox.
    Check { id: &'static str, text: &'static str },
    /// Subpanel header (`bl_label`).
    SubPanel(&'static str),
    /// `col.prop(sculpt, "gravity", slider=True, text="Factor")`, `active` is `capabilities.has_gravity`.
    GravityFactor { active: bool },
    /// `col.prop(sculpt, "gravity_object")`.
    GravityObject { active: bool },
}

pub fn options() -> Vec<Row> {
    vec![
        Row::Heading("Transform Only"),
        Row::Check { id: "use_transform_data_pivot", text: "Pivot" },
        Row::Separator,
        Row::Heading("Display"),
        Row::Check { id: "show_low_resolution", text: "Fast Navigate" },
        Row::Check { id: "use_sculpt_delay_updates", text: "Delay Viewport Updates" },
        Row::Check { id: "use_deform_only", text: "Use Deform Only" },
    ]
}

/// `has_gravity` is `brush.sculpt_capabilities.has_gravity`.
pub fn gravity(has_gravity: bool) -> Vec<Row> {
    vec![Row::SubPanel("Gravity"), Row::GravityFactor { active: has_gravity }, Row::GravityObject { active: has_gravity }]
}

/// Full popover content: Options with the Gravity subpanel after it.
pub fn layout(has_gravity: bool) -> Vec<Row> {
    let mut r = options();
    r.extend(gravity(has_gravity));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_rows_follow_python() {
        let r = options();
        assert_eq!(r.len(), 7);
        assert_eq!(r[0], Row::Heading("Transform Only"));
        assert_eq!(r[1], Row::Check { id: "use_transform_data_pivot", text: "Pivot" });
        assert_eq!(r[2], Row::Separator);
        let ids: Vec<_> = r.iter().filter_map(|x| if let Row::Check { id, .. } = x { Some(*id) } else { None }).collect();
        assert_eq!(ids, ["use_transform_data_pivot", "show_low_resolution", "use_sculpt_delay_updates", "use_deform_only"]);
    }

    #[test]
    fn gravity_inactive_without_capability() {
        assert_eq!(gravity(false)[1], Row::GravityFactor { active: false });
        assert_eq!(gravity(false)[2], Row::GravityObject { active: false });
        assert_eq!(gravity(true)[1], Row::GravityFactor { active: true });
    }

    #[test]
    fn layout_has_gravity_subpanel_last() {
        let r = layout(true);
        assert_eq!(r[7], Row::SubPanel("Gravity"));
        assert_eq!(r.len(), 10);
    }
}
