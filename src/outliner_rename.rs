//! Outliner rename (`OUTLINER_OT_item_rename`, editors/space_outliner/outliner_edit.cc:326-467).
//!
//! `do_item_rename()` only opens the text field (`TSE_TEXTBUT`) when the element has an editable name: the scene
//! collection (`COLLECTION_IS_MASTER`, "Not an editable name") does not. Double click renames the element under
//! the cursor, F2 (`use_active`) the active one (outliner_edit.cc:405-417, keymap blender_default.py:1365-1367).

use crate::scene::{RowKind, Scene};

/// An open rename field: the object being renamed and the text edited so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub id: u32,
    pub text: String,
}

/// `do_item_rename()` for the tree row `kind`: the object it names, if that name is editable here. Collections are
/// not objects in `Scene`, so their rows (and the master Scene Collection) open no field.
pub fn start(scene: &Scene, kind: &RowKind) -> Option<Rename> {
    let RowKind::Object(id) = *kind else { return None };
    let o = scene.index_of(id).map(|i| &scene.objects[i])?;
    Some(Rename { id, text: o.name.clone() })
}

/// `outliner_item_rename_find_active()`: F2 renames the active object; without one nothing happens
/// ("No active item to rename").
pub fn start_active(scene: &Scene) -> Option<Rename> {
    let id = scene.active?;
    start(scene, &RowKind::Object(id))
}

/// Confirms the field: sets the name like `rna_Object_name_set`. Returns whether the name changed.
pub fn commit(scene: &mut Scene, rename: &Rename) -> bool {
    scene.rename_object(rename.id, &rename.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_object_rows_open_a_field() {
        let s = Scene::startup();
        let rows = s.outliner_rows();
        assert_eq!(start(&s, &rows[0].kind), None); /* Scene Collection: master, not editable */
        assert_eq!(start(&s, &rows[1].kind), None);
        let r = start(&s, &rows[3].kind).unwrap();
        assert_eq!(r.text, "Cube");
    }

    #[test]
    fn f2_uses_the_active_object() {
        let mut s = Scene::startup();
        let r = start_active(&s).unwrap();
        assert_eq!(Some(r.id), s.active);
        s.active = None;
        assert_eq!(start_active(&s), None);
    }

    #[test]
    fn commit_renames_uniquely() {
        let mut s = Scene::startup();
        let mut r = start_active(&s).unwrap();
        r.text = "Light".into();
        assert!(commit(&mut s, &r));
        assert_eq!(s.objects[s.index_of(r.id).unwrap()].name, "Light.001");
        r.text = "  ".into();
        assert!(!commit(&mut s, &r));
    }
}
