//! Port of `armature_select_linked_impl()` (source/blender/editors/armature/armature_select.cc) with
//! `ED_armature_ebone_selectflag_set()`, `ED_armature_ebone_select_set()` and `ED_armature_edit_sync_selection()`
//! (armature_utils.cc). Bone-collection visibility is not modelled yet: `EBONE_SELECTABLE` only checks
//! `BONE_HIDDEN_A` and `BONE_UNSELECTABLE`.

use crate::edit_bone::*;

const TOUCH: i32 = 1 << 0;
const LINK: i32 = 1 << 1;
const SEL_MASK: u32 = BONE_SELECTED | BONE_ROOTSEL | BONE_TIPSEL;

/// `EBONE_SELECTABLE(arm, ebone)`.
pub fn ebone_selectable(e: &EditBone) -> bool {
    e.flag & BONE_HIDDEN_A == 0 && e.flag & BONE_UNSELECTABLE == 0
}

/// `ED_armature_ebone_selectflag_set()`.
pub fn selectflag_set(edbo: &mut [EditBone], i: usize, flag: u32) {
    let flag = flag & SEL_MASK;
    let connected_parent = match edbo[i].parent {
        Some(p) if edbo[i].flag & BONE_CONNECTED != 0 => Some(p),
        _ => None,
    };
    edbo[i].flag &= !SEL_MASK;
    if let Some(p) = connected_parent {
        edbo[p].flag &= !BONE_TIPSEL;
        edbo[i].flag |= flag;
        if flag & BONE_ROOTSEL != 0 {
            edbo[p].flag |= BONE_TIPSEL;
        }
    } else {
        edbo[i].flag |= flag;
    }
}

/// `ED_armature_ebone_select_set()`.
pub fn ebone_select_set(edbo: &mut [EditBone], i: usize, select: bool) {
    selectflag_set(edbo, i, if select { SEL_MASK } else { 0 });
}

/// `ED_armature_edit_sync_selection()`.
pub fn edit_sync_selection(edbo: &mut [EditBone]) {
    for i in 0..edbo.len() {
        if edbo[i].flag & BONE_UNSELECTABLE != 0 {
            continue;
        }
        if let Some(p) = edbo[i].parent {
            if edbo[i].flag & BONE_CONNECTED != 0 {
                if edbo[p].flag & BONE_TIPSEL != 0 {
                    edbo[i].flag |= BONE_ROOTSEL;
                } else {
                    edbo[i].flag &= !BONE_ROOTSEL;
                }
            }
        }
        if edbo[i].flag & BONE_TIPSEL != 0 && edbo[i].flag & BONE_ROOTSEL != 0 {
            edbo[i].flag |= BONE_SELECTED;
        } else {
            edbo[i].flag &= !BONE_SELECTED;
        }
    }
}

/// `CHECK_PARENT(ebone)`: connected to a selectable parent.
fn check_parent(edbo: &[EditBone], i: usize) -> Option<usize> {
    let p = edbo[i].parent?;
    (edbo[i].flag & BONE_CONNECTED != 0 && ebone_selectable(&edbo[p])).then_some(p)
}

/// `armature_select_linked_impl()`: bones flagged `BONE_DONE` seed the walk. `all_forks` selects every connected bone
/// up and down forks, otherwise all parents and children but not the children of the root bone. Returns `changed`.
pub fn select_linked_impl(edbo: &mut [EditBone], select: bool, all_forks: bool) -> bool {
    let mut changed = false;
    for e in edbo.iter_mut() {
        e.temp = 0;
    }

    /* Select parents. */
    for iter in 0..edbo.len() {
        if edbo[iter].temp & TOUCH != 0 || edbo[iter].flag & BONE_DONE == 0 {
            continue;
        }
        edbo[iter].temp |= TOUCH | LINK;
        let mut cur = Some(iter);
        while let Some(i) = cur {
            ebone_select_set(edbo, i, select);
            changed = true;
            edbo[i].temp |= if all_forks { TOUCH | LINK } else { TOUCH };
            if let Some(p) = edbo[i].parent {
                if edbo[p].temp & LINK != 0 {
                    break;
                }
            }
            cur = check_parent(edbo, i);
        }
    }

    /* Select children. */
    for iter in 0..edbo.len() {
        if check_parent(edbo, iter).is_none() || edbo[iter].temp & TOUCH != 0 {
            continue;
        }
        let mut touched_parent = None;
        let mut cur = Some(iter);
        while let Some(i) = cur {
            if edbo[i].temp & TOUCH != 0 {
                touched_parent = Some(i);
                break;
            }
            edbo[i].temp |= TOUCH;
            cur = check_parent(edbo, i);
        }
        if let Some(tp) = touched_parent {
            if edbo[tp].temp & LINK != 0 {
                let mut i = iter;
                while i != tp {
                    if edbo[i].temp & LINK == 0 {
                        edbo[i].temp |= LINK;
                        ebone_select_set(edbo, i, select);
                        changed = true;
                    }
                    i = edbo[i].parent.expect("walked up through a connected parent");
                }
            }
        }
    }

    if changed {
        edit_sync_selection(edbo);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// root(0) <- a(1) <- b(2), a <- c(3) (fork), all connected; d(4) unconnected child of root.
    fn rig() -> Vec<EditBone> {
        let mut v = vec![
            EditBone::new("root", [0.; 3], [0., 0., 1.]),
            EditBone::new("a", [0., 0., 1.], [0., 0., 2.]).with_parent(0),
            EditBone::new("b", [0., 0., 2.], [0., 0., 3.]).with_parent(1),
            EditBone::new("c", [0., 0., 2.], [1., 0., 2.]).with_parent(1),
            EditBone::new("d", [5., 0., 0.], [5., 0., 1.]).with_parent(0),
        ];
        for i in [1, 2, 3] {
            v[i].flag |= BONE_CONNECTED;
        }
        v
    }

    fn selected(v: &[EditBone]) -> Vec<usize> {
        (0..v.len()).filter(|&i| v[i].flag & BONE_SELECTED != 0).collect()
    }

    #[test]
    fn all_forks_selects_the_whole_connected_tree() {
        let mut v = rig();
        v[2].flag |= BONE_DONE;
        assert!(select_linked_impl(&mut v, true, true));
        assert_eq!(selected(&v), vec![0, 1, 2, 3]);
        assert_eq!(v[4].flag & BONE_SELECTED, 0);
    }

    #[test]
    fn fork_sibling_is_skipped_when_not_all_forks() {
        let mut v = rig();
        v[2].flag |= BONE_DONE;
        assert!(select_linked_impl(&mut v, true, false));
        assert_eq!(selected(&v), vec![0, 1, 2]);
    }

    #[test]
    fn children_of_a_linked_bone_are_selected() {
        let mut v = rig();
        v[1].flag |= BONE_DONE;
        assert!(select_linked_impl(&mut v, true, true));
        assert_eq!(selected(&v), vec![0, 1, 2, 3]);
    }

    #[test]
    fn deselect_clears_flags_and_no_seed_changes_nothing() {
        let mut v = rig();
        for i in 0..5 {
            ebone_select_set(&mut v, i, true);
        }
        assert!(!select_linked_impl(&mut v, false, true));
        v[1].flag |= BONE_DONE;
        assert!(select_linked_impl(&mut v, false, true));
        assert_eq!(v[1].flag & SEL_MASK, 0);
        assert_eq!(v[3].flag & BONE_SELECTED, 0);
        assert_ne!(v[4].flag & BONE_SELECTED, 0);
    }

    #[test]
    fn selectflag_set_moves_tip_to_connected_parent() {
        let mut v = rig();
        ebone_select_set(&mut v, 1, true);
        assert_ne!(v[0].flag & BONE_TIPSEL, 0);
        ebone_select_set(&mut v, 1, false);
        assert_eq!(v[0].flag & BONE_TIPSEL, 0);
    }
}
