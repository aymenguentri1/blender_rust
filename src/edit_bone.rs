//! Port of `ED_armature_ebone_find_name()` and `ED_armature_ebone_get_mirrored()`
//! `ED_armature_ebone_is_child_recursive()` and `ED_armature_ebone_find_shared_parent()`
//! (source/blender/editors/armature/armature_utils.cc). The edit-bone list is a plain `Vec`.

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
}
