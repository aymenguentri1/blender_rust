//! Port of `ED_armature_ebone_find_name()`, `ED_armature_ebone_get_mirrored()`,
//! `ED_armature_ebone_is_child_recursive()` and `ED_armature_ebone_find_shared_parent()`
//! (source/blender/editors/armature/armature_utils.cc), and `editbone_unique_check()` and
//! `ED_armature_ebone_unique_name()` (armature_naming.cc). The edit-bone list is a plain `Vec`.

use crate::scene::ops::flip_side_name;

/// `MAXBONENAME` in DNA_armature_types.h.
pub const MAXBONENAME: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct EditBone {
    pub name: String,
    pub head: [f32; 3],
    pub tail: [f32; 3],
    /// `EditBone::parent` as an index into the edit-bone list.
    pub parent: Option<usize>,
}

impl EditBone {
    pub fn new(name: &str, head: [f32; 3], tail: [f32; 3]) -> Self {
        Self { name: name.to_string(), head, tail, parent: None }
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
}
