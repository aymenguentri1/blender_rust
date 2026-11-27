//! Port of `ED_armature_ebone_find_name()`, `ED_armature_ebone_get_mirrored()`,
//! `ED_armature_ebone_is_child_recursive()` and `ED_armature_ebone_find_shared_parent()`
//! (source/blender/editors/armature/armature_utils.cc), and `editbone_unique_check()` and
//! `ED_armature_ebone_unique_name()`, the edit-mode part of `ED_armature_bone_rename()` and
//! `ED_armature_bones_flip_names()` (armature_naming.cc). The edit-bone list is a plain `Vec`.

use crate::scene::ops::flip_side_name;

/// `eBone_Flag` bits used so far (DNA_armature_types.h).
pub const BONE_SELECTED: u32 = 1 << 0;
pub const BONE_ROOTSEL: u32 = 1 << 1;
pub const BONE_TIPSEL: u32 = 1 << 2;
pub const BONE_CONNECTED: u32 = 1 << 4;
pub const BONE_DONE: u32 = 1 << 7;
pub const BONE_HIDDEN_A: u32 = 1 << 10;
pub const BONE_UNSELECTABLE: u32 = 1 << 21;

/// `MAXBONENAME` in DNA_armature_types.h.
pub const MAXBONENAME: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct EditBone {
    pub name: String,
    pub head: [f32; 3],
    pub tail: [f32; 3],
    /// `EditBone::parent` as an index into the edit-bone list.
    pub parent: Option<usize>,
    /// `EditBone::flag` (`BONE_*` bits below).
    pub flag: u32,
    /// `EditBone::temp.i`, scratch space for operators.
    pub temp: i32,
}

impl EditBone {
    pub fn new(name: &str, head: [f32; 3], tail: [f32; 3]) -> Self {
        Self { name: name.to_string(), head, tail, parent: None, flag: 0, temp: 0 }
    }
}

impl EditBone {
    pub fn with_parent(mut self, parent: usize) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// `ED_armature_ebone_is_child_recursive()`: whether `parent` is an ancestor of `child` (a bone is not its own child).
pub fn is_child_recursive(edbo: &[EditBone], parent: usize, child: usize) -> bool {
    let mut cur = edbo[child].parent;
    /* The step cap only guards against a corrupt parent cycle. */
    for _ in 0..=edbo.len() {
        match cur {
            Some(i) if i == parent => return true,
            Some(i) => cur = edbo[i].parent,
            None => return false,
        }
    }
    false
}

/// `ED_armature_ebone_find_shared_parent()`: the closest common ancestor of all `children` (indices), found along the
/// parent chain of the first one; `None` when there is none.
pub fn find_shared_parent(edbo: &[EditBone], children: &[usize]) -> Option<usize> {
    let first = *children.first()?;
    let mut count = vec![0usize; edbo.len()];
    for &c in children {
        let mut cur = edbo[c].parent;
        for _ in 0..=edbo.len() {
            let Some(i) = cur else { break };
            count[i] += 1;
            cur = edbo[i].parent;
        }
    }
    let mut cur = edbo[first].parent;
    for _ in 0..=edbo.len() {
        let i = cur?;
        if count[i] == children.len() {
            return Some(i);
        }
        cur = edbo[i].parent;
    }
    None
}

/// `animrig::bone_default_name`.
pub const BONE_DEFAULT_NAME: &str = "Bone";

/// `editbone_unique_check()`: whether another bone than `bone` (an index to ignore) already has `name`.
pub fn unique_check(edbo: &[EditBone], name: &str, bone: Option<usize>) -> bool {
    edbo.iter().enumerate().any(|(i, b)| b.name == name && Some(i) != bone)
}

/// `BLI_string_split_name_number()`: the name without a numeric `delim` suffix and that number (0 without one).
fn split_name_number(name: &str, delim: char) -> (&str, i32) {
    let Some(at) = name.rfind(delim) else { return (name, 0) };
    let num = &name[at + delim.len_utf8()..];
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return (name, 0);
    }
    match num.parse::<i32>() {
        Ok(n) => (&name[..at], n),
        Err(_) => (name, 0), /* `std::stoi()` out of range */
    }
}

/// `BLI_strncpy_utf8_rlen()`: `s` cut to at most `max` bytes on a character boundary.
fn cut_utf8(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `ED_armature_ebone_unique_name()` (`BLI_uniquename_cb()` with `.` and the `Bone` default): `name` as is when no
/// other bone than `bone` has it, else with the `.001`, `.002`, ... suffix counted up from its own number.
pub fn unique_name(edbo: &[EditBone], name: &str, bone: Option<usize>) -> String {
    let name = if name.is_empty() { BONE_DEFAULT_NAME } else { name };
    if !unique_check(edbo, name, bone) {
        return name.to_string();
    }
    let (left, mut number) = split_name_number(name, '.');
    loop {
        number += 1;
        let numstr = format!(".{number:03}");
        let candidate = if left.is_empty() || numstr.len() + 1 >= MAXBONENAME {
            numstr
        } else {
            format!("{}{}", cut_utf8(left, MAXBONENAME - numstr.len() - 1), numstr)
        };
        if !unique_check(edbo, &candidate, bone) {
            return candidate;
        }
    }
}

/// `ED_armature_bone_rename()` in edit mode: the first bone named `old` gets `new`, made unique against every bone
/// (the renamed one included, so a taken name gets a suffix). Nothing happens for equal names or an unknown `old`.
pub fn rename(edbo: &mut [EditBone], old: &str, new: &str) {
    if old == new {
        return;
    }
    let Some(i) = edbo.iter().position(|b| b.name == old) else { return };
    let unique = unique_name(edbo, new, None);
    edbo[i].name = unique;
}

/// `BLI_string_flip_side_name()` for a bone: the name itself when it has no side token or is shorter than 3
/// characters; `strip_number` drops a trailing `.001` style number first. Uses `flip_side_name()`'s tokens.
pub fn flip_name(name: &str, strip_number: bool) -> String {
    if name.len() < 3 {
        return name.to_string();
    }
    let base = match name.rfind('.') {
        Some(i) if strip_number && name.ends_with(|c: char| c.is_ascii_digit()) && name[i + 1..].starts_with(|c: char| c.is_ascii_digit()) => &name[..i],
        _ => name,
    };
    flip_side_name(base).unwrap_or_else(|| base.to_string())
}

/// `ED_armature_bones_flip_names()`: flips the names of the bones `selected` (indices, in order; Blender's list holds
/// the live name buffers, so a bone is always renamed from its current name). The first pass renames blindly; a bone
/// that was renamed, whatever came out, is renamed again to the flipped name in a second pass, once the others have
/// moved out of the way.
pub fn flip_names(edbo: &mut [EditBone], selected: &[usize], strip_number: bool) {
    let mut conflicts: Vec<(usize, String)> = Vec::new();
    for &i in selected {
        let name = edbo[i].name.clone();
        let flipped = flip_name(&name, strip_number);
        rename(edbo, &name, &flipped);
        if name != flipped {
            conflicts.push((i, flipped));
        }
    }
    for (i, flipped) in conflicts {
        let name = edbo[i].name.clone();
        rename(edbo, &name, &flipped);
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

    /* root(0) <- a(1) <- b(2); root <- c(3); lone(4) */
    fn tree() -> Vec<EditBone> {
        let b = |n: &str| EditBone::new(n, [0.0; 3], [0.0, 0.0, 1.0]);
        vec![b("root"), b("a").with_parent(0), b("b").with_parent(1), b("c").with_parent(0), b("lone")]
    }

    #[test]
    fn child_recursive() {
        let t = tree();
        assert!(is_child_recursive(&t, 1, 2)); // direct
        assert!(is_child_recursive(&t, 0, 2)); // indirect
        assert!(!is_child_recursive(&t, 2, 0)); // reversed
        assert!(!is_child_recursive(&t, 1, 3)); // unrelated branch
        assert!(!is_child_recursive(&t, 4, 2)); // unrelated bone
        assert!(!is_child_recursive(&t, 1, 1)); // not its own child
    }

    #[test]
    fn shared_parent() {
        let t = tree();
        assert_eq!(find_shared_parent(&t, &[2, 3]), Some(0)); // cousins
        assert_eq!(find_shared_parent(&t, &[1, 3]), Some(0)); // siblings
        assert_eq!(find_shared_parent(&t, &[2]), Some(1)); // a single bone: its parent
        assert_eq!(find_shared_parent(&t, &[2, 1]), Some(0)); // a bone and its own child: above the ancestor
        assert_eq!(find_shared_parent(&t, &[2, 4]), None); // unrelated
        assert_eq!(find_shared_parent(&t, &[0]), None); // a root
        assert_eq!(find_shared_parent(&t, &[]), None);
    }

    fn named(names: &[&str]) -> Vec<EditBone> {
        names.iter().map(|n| EditBone::new(n, [0.0; 3], [0.0, 0.0, 1.0])).collect()
    }

    #[test]
    fn unique_name_unchanged_when_free() {
        let l = named(&["Bone", "Arm"]);
        assert_eq!(unique_name(&l, "Leg", None), "Leg");
        assert_eq!(unique_name(&l, "", None), "Bone.001"); // the default name is taken
        assert_eq!(unique_name(&named(&[]), "", None), "Bone");
    }

    #[test]
    fn unique_name_adds_and_counts_the_suffix() {
        let l = named(&["Bone", "Bone.001", "Arm.005"]);
        assert_eq!(unique_name(&l, "Bone", None), "Bone.002");
        assert_eq!(unique_name(&l, "Arm.005", None), "Arm.006"); // counts up from its own number
        assert_eq!(unique_name(&l, "Bone.001", None), "Bone.002");
        assert_eq!(unique_name(&named(&["a."]), "a.", None), "a..001"); // a trailing delimiter is no number
        assert_eq!(unique_name(&named(&["a.x"]), "a.x", None), "a.x.001");
    }

    #[test]
    fn unique_name_ignores_the_bone_itself() {
        let l = named(&["Bone", "Arm"]);
        assert_eq!(unique_name(&l, "Bone", Some(0)), "Bone"); // renaming a bone to its own name
        assert_eq!(unique_name(&l, "Arm", Some(0)), "Arm.001"); // another bone has it
        assert!(!unique_check(&l, "Bone", Some(0)) && unique_check(&l, "Bone", Some(1)) && unique_check(&l, "Bone", None));
    }

    #[test]
    fn unique_name_cuts_long_names_to_the_buffer() {
        let long = "x".repeat(63);
        let l = named(&[&long]);
        let u = unique_name(&l, &long, None);
        assert_eq!(u.len(), 63);
        assert!(u.ends_with(".001"));
    }

    #[test]
    fn flip_name_sides_and_untouched() {
        assert_eq!(flip_name("Arm.L", false), "Arm.R");
        assert_eq!(flip_name("Arm.R", false), "Arm.L");
        assert_eq!(flip_name("Spine", false), "Spine");
        assert_eq!(flip_name(".L", false), ".L"); // shorter than 3 characters
        assert_eq!(flip_name("Arm.L.001", false), "Arm.R.001");
        assert_eq!(flip_name("Arm.L.001", true), "Arm.R");
        assert_eq!(flip_name("Spine.001", true), "Spine"); // the number is stripped even without a side
    }

    #[test]
    fn flip_names_swaps_a_selected_pair() {
        let mut l = named(&["Arm.L", "Arm.R", "Spine"]);
        flip_names(&mut l, &[0, 1], false);
        assert_eq!(l.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["Arm.R", "Arm.L", "Spine"]);
    }

    #[test]
    fn flip_names_keeps_names_unique_and_leaves_no_side_alone() {
        /* Only .L is selected and .R exists: the flipped name is taken, so it gets a suffix. The second pass asks for
         * .R again without ignoring the bone (`ED_armature_bone_rename()` passes no bone), so its own .001 counts as
         * taken and it ends on .002. */
        let mut l = named(&["Arm.L", "Arm.R", "Spine"]);
        flip_names(&mut l, &[0, 2], false);
        assert_eq!(l[0].name, "Arm.R.002");
        assert_eq!(l[1].name, "Arm.R");
        assert_eq!(l[2].name, "Spine");
        let mut all: Vec<_> = l.iter().map(|b| b.name.clone()).collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn rename_ignores_equal_and_unknown() {
        let mut l = named(&["A", "B"]);
        rename(&mut l, "A", "A");
        rename(&mut l, "Z", "Q");
        rename(&mut l, "A", "B");
        assert_eq!((l[0].name.as_str(), l[1].name.as_str()), ("B.001", "B"));
    }
}
