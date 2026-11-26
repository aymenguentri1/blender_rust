//! Outliner rename (`OUTLINER_OT_item_rename`, editors/space_outliner/outliner_edit.cc:326-467).
//!
//! `do_item_rename()` only opens the text field (`TSE_TEXTBUT`) when the element has an editable name: the scene
//! collection (`COLLECTION_IS_MASTER`, "Not an editable name") does not. Double click renames the element under
//! the cursor, F2 (`use_active`) the active one (outliner_edit.cc:405-417, keymap blender_default.py:1365-1367).

use crate::scene::{RowKind, Scene};

/// An open rename field: the object being renamed and the text edited so far. `pos`, `selsta` and `selend` are
/// character indices (`Button::pos/selsta/selend`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub id: u32,
    pub text: String,
    pub pos: usize,
    pub selsta: usize,
    pub selend: usize,
}

/// The keys `ui_do_but_textedit()` (interface_handlers.cc:4290-4470) handles that this field supports; word jumps,
/// the clipboard and the text undo stack are not ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    /// Ctrl + A.
    SelectAll,
    Char(char),
}

/// `do_item_rename()` for the tree row `kind`: the object it names, if that name is editable here. Collections are
/// not objects in `Scene`, so their rows (and the master Scene Collection) open no field.
pub fn start(scene: &Scene, kind: &RowKind) -> Option<Rename> {
    let RowKind::Object(id) = *kind else { return None };
    let o = scene.index_of(id).map(|i| &scene.objects[i])?;
    /* `ui_textedit_begin()` (interface_handlers.cc:3900-3912): the cursor is at the end and everything is selected. */
    let len = o.name.chars().count();
    Some(Rename { id, text: o.name.clone(), pos: len, selsta: 0, selend: len })
}

/// `outliner_item_rename_find_active()`: F2 renames the active object; without one nothing happens
/// ("No active item to rename").
pub fn start_active(scene: &Scene) -> Option<Rename> {
    let id = scene.active?;
    start(scene, &RowKind::Object(id))
}

impl Rename {
    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }

    fn set(&mut self, c: Vec<char>) {
        self.text = c.into_iter().collect();
    }

    /// `textedit_delete_selection()`: removes the selection and puts the cursor where it was. Returns whether
    /// anything was removed.
    fn delete_selection(&mut self) -> bool {
        let mut c = self.chars();
        let changed = self.selsta != self.selend && !c.is_empty();
        if changed {
            c.drain(self.selsta..self.selend.min(c.len()));
            self.set(c);
        }
        self.pos = self.selsta;
        self.selend = self.selsta;
        changed
    }

    /// `textedit_move()`: one character left or right, or to the start or end with `all`; `select` extends the
    /// selection. Without `select` a selection collapses to its start (left) or end (right).
    fn step(&mut self, forward: bool, select: bool, all: bool) {
        let len = self.chars().len();
        let prev = self.pos;
        let has_sel = self.selend > self.selsta;
        if has_sel && !select {
            self.pos = if all { if forward { len } else { 0 } } else if forward { self.selend } else { self.selsta };
            self.selsta = self.pos;
            self.selend = self.pos;
            return;
        }
        self.pos = if all {
            if forward { len } else { 0 }
        } else if forward {
            (self.pos + 1).min(len)
        } else {
            self.pos.saturating_sub(1)
        };
        if select {
            if !has_sel {
                self.selsta = self.pos;
                self.selend = prev;
            } else if self.selsta == prev {
                self.selsta = self.pos;
            } else {
                self.selend = self.pos;
            }
            if self.selend < self.selsta {
                std::mem::swap(&mut self.selsta, &mut self.selend);
            }
        } else {
            self.selsta = self.pos;
            self.selend = self.pos;
        }
    }

    /// One key press (`shift`: the Shift modifier). Returns whether the text changed.
    pub fn key(&mut self, key: Key, shift: bool) -> bool {
        match key {
            Key::Left => self.step(false, shift, false),
            Key::Right => self.step(true, shift, false),
            Key::Home => self.step(false, shift, true),
            Key::End => self.step(true, shift, true),
            Key::SelectAll => {
                /* Ctrl + A: `textedit_move()` to the start, then selecting to the end. */
                self.step(false, false, true);
                self.step(true, true, true);
            }
            Key::Delete | Key::Backspace => {
                if self.selend > self.selsta {
                    return self.delete_selection();
                }
                let mut c = self.chars();
                if key == Key::Delete && self.pos < c.len() {
                    c.remove(self.pos);
                } else if key == Key::Backspace && self.pos > 0 {
                    self.pos -= 1;
                    c.remove(self.pos);
                } else {
                    return false;
                }
                self.set(c);
                self.selsta = self.pos;
                self.selend = self.pos;
                return true;
            }
            Key::Char(ch) => {
                /* `textedit_insert_buf()`: types over the selection. */
                if ch.is_control() {
                    return false;
                }
                self.delete_selection();
                let mut c = self.chars();
                c.insert(self.pos, ch);
                self.pos += 1;
                self.set(c);
                self.selsta = self.pos;
                self.selend = self.pos;
                return true;
            }
        }
        false
    }
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
    fn typing_replaces_the_initial_selection() {
        let mut s = Scene::startup();
        let mut r = start_active(&s).unwrap();
        assert_eq!((r.pos, r.selsta, r.selend), (4, 0, 4));
        assert!(r.key(Key::Char('A'), false));
        assert!(r.key(Key::Char('b'), false));
        assert_eq!((r.text.as_str(), r.pos), ("Ab", 2));
        assert!(r.key(Key::Backspace, false));
        assert_eq!(r.text, "A");
        assert!(!r.key(Key::Delete, false));
        assert!(r.key(Key::Home, false) == false && r.pos == 0);
        assert!(r.key(Key::Delete, false));
        assert_eq!(r.text, "");
        assert!(!commit(&mut s, &r));
    }

    #[test]
    fn cursor_and_selection_moves() {
        let s = Scene::startup();
        let mut r = start_active(&s).unwrap(); /* "Cube", all selected, cursor at 4 */
        r.key(Key::Left, false); /* collapses to the selection start */
        assert_eq!((r.pos, r.selsta, r.selend), (0, 0, 0));
        r.key(Key::Right, true);
        r.key(Key::Right, true);
        assert_eq!((r.pos, r.selsta, r.selend), (2, 0, 2));
        r.key(Key::Left, true);
        assert_eq!((r.pos, r.selsta, r.selend), (1, 0, 1));
        assert!(r.key(Key::Char('X'), false)); /* replaces "C" */
        assert_eq!(r.text, "Xube");
        r.key(Key::End, false);
        r.key(Key::Left, false);
        r.key(Key::Left, true);
        assert_eq!((r.pos, r.selsta, r.selend), (2, 2, 3));
        r.key(Key::SelectAll, false);
        assert_eq!((r.selsta, r.selend), (0, 4));
        assert!(r.key(Key::Backspace, false));
        assert_eq!((r.text.as_str(), r.pos), ("", 0));
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
