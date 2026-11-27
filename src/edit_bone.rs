//! Port of `ED_armature_ebone_find_name()` and `ED_armature_ebone_get_mirrored()`
//! (source/blender/editors/armature/armature_utils.cc). The edit-bone list is a plain `Vec`.

use crate::scene::ops::flip_side_name;

/// `MAXBONENAME` in DNA_armature_types.h.
pub const MAXBONENAME: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct EditBone {
    pub name: String,
    pub head: [f32; 3],
    pub tail: [f32; 3],
}

impl EditBone {
    pub fn new(name: &str, head: [f32; 3], tail: [f32; 3]) -> Self {
        Self { name: name.to_string(), head, tail }
    }
}

/// `ED_armature_ebone_find_name()`: first bone whose name equals `name` (BLI_findstring).
pub fn find_name<'a>(edbo: &'a [EditBone], name: &str) -> Option<&'a EditBone> {
    edbo.iter().find(|b| b.name == name)
}

/// `ED_armature_ebone_get_mirrored()`: the bone named like `ebo` with its side flipped, `None` when the name has no
/// side token or no such bone exists.
pub fn get_mirrored<'a>(edbo: &'a [EditBone], ebo: Option<&EditBone>) -> Option<&'a EditBone> {
    let ebo = ebo?;
    let flipped = flip_side_name(&ebo.name)?;
    if flipped == ebo.name {
        return None;
    }
    find_name(edbo, &flipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Vec<EditBone> {
        ["Arm.L", "Arm.R", "Spine", "Leg_l"].iter().map(|n| EditBone::new(n, [0.0; 3], [0.0, 0.0, 1.0])).collect()
    }

    #[test]
    fn find_by_name() {
        let l = list();
        assert_eq!(find_name(&l, "Spine").map(|b| b.name.as_str()), Some("Spine"));
        assert!(find_name(&l, "Nope").is_none());
    }

    #[test]
    fn mirrored_lookup() {
        let l = list();
        assert_eq!(get_mirrored(&l, Some(&l[0])).map(|b| b.name.as_str()), Some("Arm.R"));
        assert_eq!(get_mirrored(&l, Some(&l[1])).map(|b| b.name.as_str()), Some("Arm.L"));
        assert!(get_mirrored(&l, Some(&l[2])).is_none()); // no side token
        assert!(get_mirrored(&l, Some(&l[3])).is_none()); // Leg_r missing
        assert!(get_mirrored(&l, None).is_none());
    }
}
