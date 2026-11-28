//! What the rows of the sculpt header menus do: maps an operator row (`idname` and its properties, as drawn by
//! `sculpt_menu::Row`) to the command this port runs, or to nothing when the operator is not available (the row is
//! then drawn disabled).
//!
//! The Mask menu rows follow `paint.mask_flood_fill` (`mode` VALUE with `value`, or INVERT; paint_mask.cc) and
//! `sculpt.mask_filter` (`filter_type`). Other operators are not available yet.

use crate::scene::Scene;
use crate::sculpt::MaskFilter;
use crate::sculpt_hide::{self, VisAction};
use crate::sculpt_menu::Row;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    MaskFill,
    MaskClear,
    MaskInvert,
    MaskFilter(MaskFilter),
    HideShowAll(VisAction),
    HideShowMasked(VisAction),
    VisibilityInvert,
}

fn prop(props: &[(&str, &str)], name: &str) -> Option<String> {
    props
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

pub fn command_of(idname: &str, props: &[(&str, &str)]) -> Option<Command> {
    match idname {
        "paint.mask_flood_fill" => match prop(props, "mode")?.as_str() {
            "INVERT" => Some(Command::MaskInvert),
            "VALUE" => match prop(props, "value")?.parse::<f32>().ok()? {
                v if v == 1.0 => Some(Command::MaskFill),
                v if v == 0.0 => Some(Command::MaskClear),
                _ => None,
            },
            _ => None,
        },
        "sculpt.mask_filter" => match prop(props, "filter_type")?.as_str() {
            "SMOOTH" => Some(Command::MaskFilter(MaskFilter::Smooth)),
            "SHARPEN" => Some(Command::MaskFilter(MaskFilter::Sharpen)),
            "GROW" => Some(Command::MaskFilter(MaskFilter::Grow)),
            "SHRINK" => Some(Command::MaskFilter(MaskFilter::Shrink)),
            "CONTRAST_INCREASE" => Some(Command::MaskFilter(MaskFilter::ContrastIncrease)),
            "CONTRAST_DECREASE" => Some(Command::MaskFilter(MaskFilter::ContrastDecrease)),
            _ => None,
        },
        "paint.hide_show_all" => vis_action(props).map(Command::HideShowAll),
        "paint.hide_show_masked" => vis_action(props).map(Command::HideShowMasked),
        "paint.visibility_invert" => Some(Command::VisibilityInvert),
        _ => None,
    }
}

/// The `action` enum of the hide operators (`VisAction`, paint_hide.cc).
fn vis_action(props: &[(&str, &str)]) -> Option<VisAction> {
    match prop(props, "action")?.as_str() {
        "HIDE" => Some(VisAction::Hide),
        "SHOW" => Some(VisAction::Show),
        _ => None,
    }
}

/// The command of a menu row, if it is an operator row this port can run.
pub fn command_of_row(row: &Row) -> Option<Command> {
    match row {
        Row::Operator { idname, props, .. } => command_of(idname, props),
        _ => None,
    }
}

/// Runs `cmd` on the active object; true when the scene changed.
pub fn apply(scene: &mut Scene, cmd: Command) -> bool {
    match cmd {
        Command::MaskFill => scene.fill_mask_active(),
        Command::MaskClear => scene.clear_mask_active(),
        Command::MaskInvert => scene.invert_mask_active(),
        Command::MaskFilter(f) => scene.mask_filter_active(f),
        Command::HideShowAll(a) => scene.visibility_edit_active(|m, v, _| sculpt_hide::hide_show_all(m, v, a)),
        Command::HideShowMasked(a) => scene.visibility_edit_active(|m, v, mask| sculpt_hide::hide_show_masked(m, v, a, Some(mask))),
        Command::VisibilityInvert => scene.visibility_edit_active(|m, v, _| sculpt_hide::invert_visibility(m, v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mask_menu;
    use crate::mesh::Primitive;

    fn scene() -> Scene {
        let mut scene = Scene::startup();
        let id = scene.add_primitive(Primitive::UvSphere, [0.0, 0.0, 0.0]);
        scene.active = Some(id);
        scene
    }

    fn cmd(label: &str) -> Option<Command> {
        mask_menu::ROWS
            .iter()
            .find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label))
            .and_then(command_of_row)
    }

    #[test]
    fn contrast_rows_map_to_commands() {
        assert_eq!(cmd("Increase Contrast"), Some(Command::MaskFilter(MaskFilter::ContrastIncrease)));
        assert_eq!(cmd("Decrease Contrast"), Some(Command::MaskFilter(MaskFilter::ContrastDecrease)));
    }

    #[test]
    fn mask_rows_map_to_commands() {
        assert_eq!(cmd("Fill Mask"), Some(Command::MaskFill));
        assert_eq!(cmd("Clear Mask"), Some(Command::MaskClear));
        assert_eq!(cmd("Invert Mask"), Some(Command::MaskInvert));
        assert_eq!(
            cmd("Smooth Mask"),
            Some(Command::MaskFilter(MaskFilter::Smooth))
        );
        assert_eq!(
            cmd("Sharpen Mask"),
            Some(Command::MaskFilter(MaskFilter::Sharpen))
        );
        assert_eq!(
            cmd("Grow Mask"),
            Some(Command::MaskFilter(MaskFilter::Grow))
        );
        assert_eq!(
            cmd("Shrink Mask"),
            Some(Command::MaskFilter(MaskFilter::Shrink))
        );
    }

    #[test]
    fn hide_rows_map_and_change_visibility() {
        use crate::sculpt_menu;
        let find = |label: &str| {
            sculpt_menu::rows(true).into_iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).and_then(|r| command_of_row(&r))
        };
        assert_eq!(find("Show All"), Some(Command::HideShowAll(VisAction::Show)));
        assert_eq!(find("Invert Visible"), Some(Command::VisibilityInvert));
        assert_eq!(find("Hide Masked"), Some(Command::HideShowMasked(VisAction::Hide)));
        let mut s = scene();
        assert!(!apply(&mut s, Command::HideShowAll(VisAction::Show)), "nothing hidden");
        assert!(!apply(&mut s, Command::HideShowMasked(VisAction::Hide)), "nothing masked");
        assert!(apply(&mut s, Command::HideShowAll(VisAction::Hide)));
        assert!(s.active_object().unwrap().vis.hide_vert.as_ref().unwrap().iter().all(|&h| h));
        assert!(apply(&mut s, Command::HideShowAll(VisAction::Show)));
        assert_eq!(s.active_object().unwrap().vis, Default::default());
        assert!(apply(&mut s, Command::VisibilityInvert));
        assert!(s.undo() && s.active_object().unwrap().vis == Default::default());
    }

    #[test]
    fn unavailable_rows_have_no_command() {
        for l in [
            "Box Mask",
            "Mask Extract",
            "Mask by Color",
        ] {
            assert_eq!(cmd(l), None, "{l}");
        }
    }

    #[test]
    fn fill_clear_invert_change_the_mask() {
        let mut s = scene();
        let n = s.active_object().unwrap().mesh().unwrap().verts.len();
        assert!(apply(&mut s, Command::MaskFill));
        assert_eq!(s.active_object().unwrap().mask, vec![1.0; n]);
        assert!(!apply(&mut s, Command::MaskFill), "already full");
        assert!(apply(&mut s, Command::MaskInvert));
        assert!(!s.has_mask());
        assert!(apply(&mut s, Command::MaskFill) && apply(&mut s, Command::MaskClear));
        assert!(!s.has_mask());
        assert!(s.undo());
        assert!(s.has_mask(), "undo brings the filled mask back");
    }

    #[test]
    fn filters_need_a_mask() {
        let mut s = scene();
        assert!(!apply(&mut s, Command::MaskFilter(MaskFilter::Grow)));
        apply(&mut s, Command::MaskFill);
        assert!(apply(&mut s, Command::MaskFilter(MaskFilter::Smooth)));
    }
}
