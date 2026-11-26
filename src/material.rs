//! Material data and per-object material slots.
//!
//! `Material` defaults are the in-class initialisers of `struct Material` (DNA_material_types.h:380-470): base colour
//! `r, g, b, a` = 0.8, 0.8, 0.8, 1.0, `spec` 0.5, `roughness` 0.4, `metallic` 0. Slots follow `material.cc`:
//! `BKE_object_material_slot_add` (:1497), `BKE_object_material_slot_remove` (:1515), `object_material_assign` (:1183)
//! and `BKE_object_material_active_index_sanitize` (:1039). `actcol` is 1-based, 0 meaning no slot.
//! Not ported: user counts, shared mesh data between objects, the `material_index` face attribute remap.

/// `MAXMAT` (DNA_material_types.h:77).
pub const MAXMAT: usize = 32767 - 1;

/// `Material` fields the editor uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    /// `r, g, b, a`: the base colour.
    pub base_color: [f32; 4],
    /// `spec`.
    pub specular: f32,
    pub roughness: f32,
    pub metallic: f32,
}

impl Material {
    pub fn new(name: &str) -> Material {
        Material { name: name.to_string(), base_color: [0.8, 0.8, 0.8, 1.0], specular: 0.5, roughness: 0.4, metallic: 0.0 }
    }
}

/// `matbits`: whether a slot's material is stored on the object data (0) or on the object (1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SlotLink {
    #[default]
    Data,
    Object,
}

/// One material slot: an index into the scene's material list, or empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Slot {
    pub material: Option<usize>,
    pub link: SlotLink,
}

/// `Object::mat`/`matbits`/`totcol`/`actcol`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MaterialSlots {
    pub slots: Vec<Slot>,
    /// `actcol`: 1-based active slot, 0 when there are none.
    pub active: usize,
}

impl MaterialSlots {
    /// `BKE_object_material_active_index_sanitize()`.
    pub fn sanitize_active(&mut self) {
        let min = if self.slots.is_empty() { 0 } else { 1 };
        self.active = self.active.clamp(min, self.slots.len());
    }

    /// `BKE_object_material_slot_add()`: appends an empty slot whose link copies the active slot's (`USERPREF` with a
    /// valid `actcol`; the user-preference default is the object data), and makes it active when `set_active`.
    /// Returns false at `MAXMAT`.
    pub fn slot_add(&mut self, set_active: bool) -> bool {
        if self.slots.len() >= MAXMAT {
            return false;
        }
        let link = if self.active >= 1 && self.active <= self.slots.len() { self.slots[self.active - 1].link } else { SlotLink::Data };
        self.slots.push(Slot { material: None, link });
        if set_active {
            self.active = self.slots.len();
        }
        true
    }

    /// `BKE_object_material_slot_remove()`: removes the active slot. False when there is no slot or `actcol <= 0`.
    pub fn slot_remove(&mut self) -> bool {
        if self.slots.is_empty() || self.active == 0 {
            return false;
        }
        self.sanitize_active();
        self.slots.remove(self.active - 1);
        self.sanitize_active();
        true
    }

    /// `object_material_assign()` with `BKE_MAT_ASSIGN_EXISTING`: slots up to `act` (1-based, at most `MAXMAT`, at least
    /// 1) are created if missing, and the slot keeps its link.
    pub fn assign(&mut self, material: Option<usize>, act: usize) {
        if act > MAXMAT {
            return;
        }
        let act = act.max(1);
        if self.slots.len() < act {
            self.slots.resize(act, Slot::default());
        }
        self.slots[act - 1].material = material;
    }

    /// `BKE_object_material_get()` for a 1-based slot.
    pub fn get(&self, act: usize) -> Option<usize> {
        act.checked_sub(1).and_then(|i| self.slots.get(i)).and_then(|s| s.material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_defaults_follow_dna() {
        let m = Material::new("Material");
        assert_eq!(m.base_color, [0.8, 0.8, 0.8, 1.0]);
        assert_eq!((m.specular, m.roughness, m.metallic), (0.5, 0.4, 0.0));
    }

    #[test]
    fn add_makes_the_new_slot_active() {
        let mut s = MaterialSlots::default();
        assert!(s.slot_add(true));
        assert!(s.slot_add(true));
        assert_eq!((s.slots.len(), s.active), (2, 2));
        assert!(s.slot_add(false));
        assert_eq!((s.slots.len(), s.active), (3, 2));
    }

    #[test]
    fn add_copies_the_link_of_the_active_slot() {
        let mut s = MaterialSlots::default();
        s.slot_add(true);
        s.slots[0].link = SlotLink::Object;
        s.slot_add(true);
        assert_eq!(s.slots[1].link, SlotLink::Object);
        s.active = 0;
        s.slot_add(true);
        assert_eq!(s.slots[2].link, SlotLink::Data);
    }

    #[test]
    fn add_stops_at_maxmat() {
        let mut s = MaterialSlots { slots: vec![Slot::default(); MAXMAT], active: 1 };
        assert!(!s.slot_add(true));
        assert_eq!(s.slots.len(), MAXMAT);
    }

    #[test]
    fn remove_drops_the_active_slot_and_clamps() {
        let mut s = MaterialSlots::default();
        assert!(!s.slot_remove());
        for m in 0..3 {
            s.slot_add(true);
            s.assign(Some(m), s.slots.len());
        }
        s.active = 3;
        assert!(s.slot_remove());
        assert_eq!((s.slots.len(), s.active, s.get(2)), (2, 2, Some(1)));
        s.active = 1;
        assert!(s.slot_remove());
        assert_eq!((s.slots.len(), s.active, s.get(1)), (1, 1, Some(1)));
        assert!(s.slot_remove());
        assert_eq!((s.slots.len(), s.active), (0, 0));
        s.slot_add(false);
        assert_eq!(s.active, 0);
        assert!(!s.slot_remove(), "actcol 0 is invalid");
    }

    #[test]
    fn assign_grows_the_list() {
        let mut s = MaterialSlots::default();
        s.assign(Some(4), 3);
        assert_eq!(s.slots.len(), 3);
        assert_eq!((s.get(1), s.get(3), s.get(0), s.get(9)), (None, Some(4), None, None));
        s.assign(Some(1), 0);
        assert_eq!(s.get(1), Some(1));
        s.assign(Some(2), MAXMAT + 1);
        assert_eq!(s.slots.len(), 3);
    }
}
