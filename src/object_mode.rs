//! `object.mode_set` for Tab (blender_default.py:2228, Object Mode keymap, mode=EDIT toggle=True;
//! `ED_object_editmode_enter_ex` / `ED_object_editmode_exit_ex` in object_edit.cc). Edit Mode needs an active
//! mesh object; this port has no other editable object type.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ObjectMode {
    #[default]
    Object,
    Edit,
    Sculpt,
}

/// The mode after pressing Tab: Edit Mode on an active mesh, back to Object Mode when already in it. Entering
/// from Sculpt Mode swaps modes like `mode_set_ex` (object_modes.cc:187). Without a mesh nothing changes.
pub fn toggle_edit(current: ObjectMode, active_is_mesh: bool) -> ObjectMode {
    match current {
        ObjectMode::Edit => ObjectMode::Object,
        _ if active_is_mesh => ObjectMode::Edit,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_toggles_between_object_and_edit_on_a_mesh() {
        assert_eq!(toggle_edit(ObjectMode::Object, true), ObjectMode::Edit);
        assert_eq!(toggle_edit(ObjectMode::Edit, true), ObjectMode::Object);
    }

    #[test]
    fn no_mesh_no_edit_mode_but_leaving_always_works() {
        assert_eq!(toggle_edit(ObjectMode::Object, false), ObjectMode::Object);
        assert_eq!(toggle_edit(ObjectMode::Edit, false), ObjectMode::Object);
    }

    #[test]
    fn sculpt_swaps_to_edit() {
        assert_eq!(toggle_edit(ObjectMode::Sculpt, true), ObjectMode::Edit);
        assert_eq!(toggle_edit(ObjectMode::Sculpt, false), ObjectMode::Sculpt);
    }
}
