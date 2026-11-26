//! Collection tree: new, move object, delete.
//!
//! `OUTLINER_OT_collection_new` (outliner_collections.cc:227 `collection_new_exec`), `OUTLINER_OT_collection_delete` and
//! `_hierarchy_delete` (:437, via `BKE_collection_delete`), and from collection.cc: `BKE_collection_add` (:481),
//! `BKE_collection_new_name_get` (:847), `BKE_collection_delete` (:587), `BKE_collection_object_move` (:1911),
//! `collection_child_add` (:2088: no duplicate child, no cycle) and `collection_object_add` (:1480: no duplicate object).
//! Not ported: unique-name suffixes, user counts and zero-user object deletion, library overrides, view layers.

/// Index of a collection in [`Collections`]; `Collections::MASTER` is the scene master collection.
pub type CollId = usize;
/// Object identity (an index the caller owns).
pub type ObjId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub name: String,
    /// `color_tag`, copied from the parent on creation.
    pub color_tag: u8,
    pub children: Vec<CollId>,
    pub objects: Vec<ObjId>,
}

/// All collections of a scene; deleted ones leave a `None` hole so ids stay stable.
#[derive(Clone, Debug)]
pub struct Collections {
    nodes: Vec<Option<Collection>>,
}

impl Default for Collections {
    fn default() -> Self {
        Collections { nodes: vec![Some(Collection { name: "Scene Collection".into(), color_tag: 0, children: vec![], objects: vec![] })] }
    }
}

impl Collections {
    /// `COLLECTION_IS_MASTER`.
    pub const MASTER: CollId = 0;

    pub fn get(&self, id: CollId) -> Option<&Collection> {
        self.nodes.get(id).and_then(|c| c.as_ref())
    }

    fn parents(&self, id: CollId) -> Vec<CollId> {
        (0..self.nodes.len()).filter(|&p| self.get(p).is_some_and(|c| c.children.contains(&id))).collect()
    }

    /// `collection_find_child_recursive()`.
    pub fn has_collection(&self, parent: CollId, child: CollId) -> bool {
        self.get(parent).is_some_and(|c| c.children.iter().any(|&k| k == child || self.has_collection(k, child)))
    }

    /// `BKE_collection_new_name_get()`.
    pub fn new_name(&self, parent: Option<CollId>) -> String {
        match parent.and_then(|p| self.get(p).map(|c| (p, c))) {
            None => "Collection".into(),
            Some((Self::MASTER, c)) => format!("Collection {}", c.children.len() + 1),
            Some((_, c)) => format!("{} {}", c.name, c.children.len() + 1),
        }
    }

    /// `collection_child_add()`: false if already a child or if it would make a cycle.
    pub fn child_add(&mut self, parent: CollId, child: CollId) -> bool {
        if self.get(parent).is_none() || self.get(child).is_none() || self.get(parent).unwrap().children.contains(&child) {
            return false;
        }
        if parent == child || self.has_collection(child, parent) {
            return false;
        }
        self.nodes[parent].as_mut().unwrap().children.push(child);
        true
    }

    /// `BKE_collection_add()`: a new child named by `new_name()` that takes the parent's `color_tag`.
    pub fn add(&mut self, parent: CollId) -> Option<CollId> {
        let name = self.new_name(Some(parent));
        let color_tag = self.get(parent)?.color_tag;
        self.nodes.push(Some(Collection { name, color_tag, children: vec![], objects: vec![] }));
        let id = self.nodes.len() - 1;
        self.child_add(parent, id);
        Some(id)
    }

    /// `collection_new_exec()`: with `nested` the new collection goes under `active` (if it exists, else the master
    /// collection); without it, under the master collection. Returns the new collection (it becomes the active one).
    pub fn new_collection(&mut self, nested: bool, active: CollId) -> CollId {
        let parent = if nested && self.get(active).is_some() { active } else { Self::MASTER };
        self.add(parent).expect("parent exists")
    }

    /// `collection_object_add()`: false if the collection already holds the object.
    pub fn object_add(&mut self, coll: CollId, ob: ObjId) -> bool {
        match self.nodes.get_mut(coll).and_then(|c| c.as_mut()) {
            Some(c) if !c.objects.contains(&ob) => {
                c.objects.push(ob);
                true
            }
            _ => false,
        }
    }

    pub fn object_remove(&mut self, coll: CollId, ob: ObjId) -> bool {
        match self.nodes.get_mut(coll).and_then(|c| c.as_mut()) {
            Some(c) => c.objects.iter().position(|&o| o == ob).map(|i| c.objects.remove(i)).is_some(),
            None => false,
        }
    }

    /// `BKE_collection_object_move()`: with a source, add to `dst` then remove from `src`; without one the object leaves
    /// every other collection even when it was already in `dst`.
    pub fn object_move(&mut self, dst: CollId, src: Option<CollId>, ob: ObjId) {
        match src {
            Some(s) => {
                if self.object_add(dst, ob) {
                    self.object_remove(s, ob);
                }
            }
            None => {
                self.object_add(dst, ob);
                for c in 0..self.nodes.len() {
                    if c != dst {
                        self.object_remove(c, ob);
                    }
                }
            }
        }
    }

    /// `BKE_collection_delete()`. False for the master collection or a missing one. With `hierarchy` the objects are
    /// unlinked and the children deleted recursively; otherwise children and objects move to every parent.
    pub fn delete(&mut self, id: CollId, hierarchy: bool) -> bool {
        if id == Self::MASTER || self.get(id).is_none() {
            return false;
        }
        let parents = self.parents(id);
        if hierarchy {
            self.nodes[id].as_mut().unwrap().objects.clear();
            while let Some(&child) = self.get(id).and_then(|c| c.children.first()) {
                self.delete(child, true);
            }
        } else {
            let c = self.get(id).unwrap().clone();
            for &p in &parents {
                for &k in &c.children {
                    self.child_add(p, k);
                }
                for &o in &c.objects {
                    self.object_add(p, o);
                }
            }
        }
        for p in parents {
            self.nodes[p].as_mut().unwrap().children.retain(|&k| k != id);
        }
        self.nodes[id] = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_names_follow_the_parent() {
        let mut c = Collections::default();
        assert_eq!(c.new_name(None), "Collection");
        let a = c.new_collection(false, 0);
        assert_eq!(c.get(a).unwrap().name, "Collection 1");
        let b = c.new_collection(false, 0);
        assert_eq!(c.get(b).unwrap().name, "Collection 2");
        let n = c.new_collection(true, a);
        assert_eq!(c.get(n).unwrap().name, "Collection 1 1");
        assert_eq!(c.get(a).unwrap().children, vec![n]);
    }

    #[test]
    fn new_inherits_color_and_falls_back_to_master() {
        let mut c = Collections::default();
        let a = c.new_collection(false, 0);
        c.nodes[a].as_mut().unwrap().color_tag = 3;
        let n = c.new_collection(true, a);
        assert_eq!(c.get(n).unwrap().color_tag, 3);
        let m = c.new_collection(true, 99);
        assert_eq!(c.get(Collections::MASTER).unwrap().children, vec![a, m]);
    }

    #[test]
    fn move_object_between_collections() {
        let mut c = Collections::default();
        let (a, b) = (c.new_collection(false, 0), c.new_collection(false, 0));
        c.object_add(a, 7);
        c.object_add(Collections::MASTER, 7);
        c.object_move(b, Some(a), 7);
        assert_eq!((c.get(a).unwrap().objects.clone(), c.get(b).unwrap().objects.clone()), (vec![], vec![7]));
        c.object_move(b, None, 7);
        assert_eq!(c.get(Collections::MASTER).unwrap().objects, Vec::<ObjId>::new());
        assert_eq!(c.get(b).unwrap().objects, vec![7]);
    }

    #[test]
    fn child_add_rejects_duplicates_and_cycles() {
        let mut c = Collections::default();
        let a = c.new_collection(false, 0);
        let b = c.new_collection(true, a);
        assert!(!c.child_add(a, b) && !c.child_add(b, a) && !c.child_add(a, a));
    }

    #[test]
    fn delete_keeps_content_in_the_parent() {
        let mut c = Collections::default();
        let a = c.new_collection(false, 0);
        let b = c.new_collection(true, a);
        c.object_add(a, 1);
        assert!(c.delete(a, false));
        assert!(c.get(a).is_none());
        assert_eq!(c.get(Collections::MASTER).unwrap().children, vec![b]);
        assert_eq!(c.get(Collections::MASTER).unwrap().objects, vec![1]);
        assert!(!c.delete(Collections::MASTER, false));
    }

    #[test]
    fn hierarchy_delete_removes_children_and_unlinks_objects() {
        let mut c = Collections::default();
        let a = c.new_collection(false, 0);
        let b = c.new_collection(true, a);
        c.object_add(b, 1);
        assert!(c.delete(a, true));
        assert!(c.get(a).is_none() && c.get(b).is_none());
        assert!(c.get(Collections::MASTER).unwrap().children.is_empty());
        assert!(c.get(Collections::MASTER).unwrap().objects.is_empty());
    }
}
