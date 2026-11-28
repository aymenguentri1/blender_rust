//! Row model of `VIEW3D_PT_tools_object_options` + `VIEW3D_PT_tools_object_options_transform`
//! (scripts/startup/bl_ui/space_view3d_toolbar.py): the object-mode "Options" popover of the tool header.
//! The parent panel draws nothing; the Transform child (HIDE_HEADER) is one column with heading "Affect Only".
//! Pure data + toggle state; the popover draws it.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TransformOptions {
    /// `tool_settings.use_transform_data_origin`.
    pub data_origin: bool,
    /// `tool_settings.use_transform_pivot_point_align`.
    pub pivot_point_align: bool,
    /// `tool_settings.use_transform_skip_children`.
    pub skip_children: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// `layout.column(heading="Affect Only")` heading, on the first row only.
    pub heading: Option<&'static str>,
    pub id: &'static str,
    pub text: &'static str,
    pub on: bool,
}

pub fn layout(o: &TransformOptions) -> Vec<Row> {
    vec![
        Row { heading: Some("Affect Only"), id: "use_transform_data_origin", text: "Origins", on: o.data_origin },
        Row { heading: None, id: "use_transform_pivot_point_align", text: "Locations", on: o.pivot_point_align },
        Row { heading: None, id: "use_transform_skip_children", text: "Parents", on: o.skip_children },
    ]
}

/// Click on a checkbox row: flips the property; returns false for an unknown id.
pub fn toggle(o: &mut TransformOptions, id: &str) -> bool {
    match id {
        "use_transform_data_origin" => o.data_origin = !o.data_origin,
        "use_transform_pivot_point_align" => o.pivot_point_align = !o.pivot_point_align,
        "use_transform_skip_children" => o.skip_children = !o.skip_children,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_python() {
        let r = layout(&TransformOptions::default());
        let t: Vec<_> = r.iter().map(|x| x.text).collect();
        assert_eq!(t, ["Origins", "Locations", "Parents"]);
        assert_eq!(r[0].heading, Some("Affect Only"));
        assert!(r[1..].iter().all(|x| x.heading.is_none()));
        assert!(r.iter().all(|x| !x.on));
    }

    #[test]
    fn toggle_updates_state_and_rows() {
        let mut o = TransformOptions::default();
        assert!(toggle(&mut o, "use_transform_skip_children"));
        assert!(layout(&o)[2].on && !layout(&o)[0].on);
        assert!(toggle(&mut o, "use_transform_skip_children"));
        assert!(!layout(&o)[2].on);
        assert!(!toggle(&mut o, "nope"));
    }
}
