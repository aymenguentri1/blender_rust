//! `outliner.item_activate` on an object row (outliner_select.cc:1989 `outliner_item_activate_invoke`, keymap
//! blender_default.py:1368-1374): click, Ctrl + click (`extend`), Shift + click (`extend_range`), Ctrl + Shift + click
//! (both). Object rows only; hidden objects are never selected (`base_select` skips them).

use crate::scene::{RowKind, Scene, SelectAction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Plain,
    Extend,
    Range,
    ExtendRange,
}

impl Click {
    pub fn from_keys(ctrl: bool, shift: bool) -> Click {
        match (ctrl, shift) {
            (false, false) => Click::Plain,
            (true, false) => Click::Extend,
            (false, true) => Click::Range,
            (true, true) => Click::ExtendRange,
        }
    }
}

/// Select and activate `id`, deselecting everything else (`OL_ITEM_SELECT | OL_ITEM_ACTIVATE` without `OL_ITEM_EXTEND`).
fn select_only(scene: &mut Scene, id: u32) {
    scene.select_all(SelectAction::Deselect);
    if let Some(i) = scene.index_of(id) {
        scene.objects[i].selected = true;
    }
    scene.active = Some(id);
}

/// The object ids of the visible rows, top to bottom.
fn object_rows(scene: &Scene) -> Vec<u32> {
    scene.outliner_rows().iter().filter_map(|r| if let RowKind::Object(id) = r.kind { Some(id) } else { None }).collect()
}

/// `outliner_item_do_activate_from_cursor()` for the object `id` under the cursor.
pub fn item_activate(scene: &mut Scene, id: u32, click: Click) {
    let Some(i) = scene.index_of(id) else { return };
    if scene.objects[i].hidden {
        return;
    }
    match click {
        /* Not extending: select and activate; everything else is deselected. */
        Click::Plain => select_only(scene, id),
        /* `tree_element_object_activate()` with `OL_SETSEL_EXTEND` swaps the selection and still activates. */
        Click::Extend => {
            scene.objects[i].selected = !scene.objects[i].selected;
            scene.active = Some(id);
        }
        Click::Range | Click::ExtendRange => {
            let extend = click == Click::ExtendRange;
            let active_selected = scene.active.and_then(|a| scene.index_of(a)).is_some_and(|k| scene.objects[k].selected);
            let Some(active) = scene.active.filter(|_| active_selected) else {
                /* `do_outliner_range_select()`: no active, or an unselected one: select and activate the cursor row. */
                select_only(scene, id);
                return;
            };
            if !extend {
                scene.select_all(SelectAction::Deselect);
            }
            let rows = object_rows(scene);
            let (Some(a), Some(c)) = (rows.iter().position(|&r| r == active), rows.iter().position(|&r| r == id)) else {
                select_only(scene, id);
                return;
            };
            for &r in &rows[a.min(c)..=a.max(c)] {
                if let Some(k) = scene.index_of(r).filter(|&k| !scene.objects[k].hidden) {
                    scene.objects[k].selected = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &Scene) -> Vec<u32> {
        object_rows(s)
    }
    fn selected(s: &Scene) -> Vec<u32> {
        ids(s).into_iter().filter(|&id| s.objects[s.index_of(id).unwrap()].selected).collect()
    }

    #[test]
    fn keys_map_to_the_keymap_items() {
        assert_eq!(Click::from_keys(false, false), Click::Plain);
        assert_eq!(Click::from_keys(true, false), Click::Extend);
        assert_eq!(Click::from_keys(false, true), Click::Range);
        assert_eq!(Click::from_keys(true, true), Click::ExtendRange);
    }

    #[test]
    fn plain_click_selects_only_that_object() {
        let mut s = Scene::startup();
        let r = ids(&s);
        item_activate(&mut s, r[0], Click::Plain);
        item_activate(&mut s, r[1], Click::Plain);
        assert_eq!(selected(&s), vec![r[1]]);
        assert_eq!(s.active, Some(r[1]));
    }

    #[test]
    fn ctrl_click_swaps_and_always_activates() {
        let mut s = Scene::startup();
        let r = ids(&s);
        item_activate(&mut s, r[0], Click::Plain);
        item_activate(&mut s, r[1], Click::Extend);
        assert_eq!(selected(&s), vec![r[0], r[1]]);
        assert_eq!(s.active, Some(r[1]));
        item_activate(&mut s, r[1], Click::Extend);
        assert_eq!(selected(&s), vec![r[0]]);
        assert_eq!(s.active, Some(r[1]), "the deselected object stays active");
    }

    #[test]
    fn shift_click_selects_the_range_from_the_active_row() {
        let mut s = Scene::startup();
        let r = ids(&s);
        assert!(r.len() >= 3);
        item_activate(&mut s, r[0], Click::Plain);
        item_activate(&mut s, r[2], Click::Range);
        assert_eq!(selected(&s), r[..3].to_vec());
        assert_eq!(s.active, Some(r[0]), "range select keeps the active object");
        /* Without extend the old selection is dropped. */
        item_activate(&mut s, r[1], Click::Range);
        assert_eq!(selected(&s), r[..2].to_vec());
    }

    #[test]
    fn range_without_a_selected_active_acts_like_a_click() {
        let mut s = Scene::startup();
        let r = ids(&s);
        s.select_all(SelectAction::Deselect);
        item_activate(&mut s, r[2], Click::Range);
        assert_eq!(selected(&s), vec![r[2]]);
        assert_eq!(s.active, Some(r[2]));
    }

    #[test]
    fn ctrl_shift_click_adds_the_range_to_the_selection() {
        let mut s = Scene::startup();
        let r = ids(&s);
        item_activate(&mut s, r[0], Click::Plain);
        item_activate(&mut s, r[2], Click::Extend);
        item_activate(&mut s, r[0], Click::Extend); /* active r[0], deselected */
        item_activate(&mut s, r[2], Click::Extend);
        item_activate(&mut s, r[1], Click::ExtendRange);
        assert!(selected(&s).contains(&r[1]));
    }

    #[test]
    fn hidden_objects_are_not_selected() {
        let mut s = Scene::startup();
        let r = ids(&s);
        s.select_all(SelectAction::Deselect);
        let k = s.index_of(r[1]).unwrap();
        s.objects[k].hidden = true;
        item_activate(&mut s, r[0], Click::Plain);
        item_activate(&mut s, r[2], Click::Range);
        assert_eq!(selected(&s), vec![r[0], r[2]]);
        item_activate(&mut s, r[1], Click::Plain);
        assert_eq!(selected(&s), vec![r[0], r[2]]);
    }
}
