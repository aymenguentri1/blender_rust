//! Per-vertex vertex-group weights (`MDeformVert` / `MDeformWeight`).
//!
//! Ported from Blender `blenkernel/intern/deform.cc`: `BKE_defvert_find_weight`, `_find_index`,
//! `_ensure_index`, `_add_index_notest`, `_remove_group`, `_clear`, `_is_weight_zero`.

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeformWeight {
    pub def_nr: i32,
    pub weight: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeformVert {
    pub dw: Vec<DeformWeight>,
}

impl DeformVert {
    /// `BKE_defvert_find_index`: position of the weight for `defgroup` (None when missing or negative).
    pub fn find_index(&self, defgroup: i32) -> Option<usize> {
        if defgroup < 0 {
            return None;
        }
        // Blender walks the array from the start.
        self.dw.iter().position(|w| w.def_nr == defgroup)
    }

    /// `BKE_defvert_find_weight`: 0.0 when the group is missing.
    pub fn find_weight(&self, defgroup: i32) -> f32 {
        self.find_index(defgroup).map_or(0.0, |i| self.dw[i].weight)
    }

    /// `BKE_defvert_ensure_index`: finds the group or appends it with weight 0. None for a negative group.
    pub fn ensure_index(&mut self, defgroup: i32) -> Option<usize> {
        if defgroup < 0 {
            return None;
        }
        if let Some(i) = self.find_index(defgroup) {
            return Some(i);
        }
        self.dw.push(DeformWeight { def_nr: defgroup, weight: 0.0 });
        Some(self.dw.len() - 1)
    }

    /// `BKE_defvert_add_index_notest`: appends without checking for an existing entry.
    pub fn add_index_notest(&mut self, defgroup: i32, weight: f32) {
        if defgroup < 0 {
            return;
        }
        self.dw.push(DeformWeight { def_nr: defgroup, weight });
    }

    /// `BKE_defvert_remove_group`: removes entry `i`; the last entry fills the hole (order is not kept).
    pub fn remove_group(&mut self, i: usize) {
        if i >= self.dw.len() {
            return;
        }
        self.dw.swap_remove(i);
    }

    /// `BKE_defvert_clear`.
    pub fn clear(&mut self) {
        self.dw.clear();
    }

    /// `BKE_defvert_is_weight_zero`: true when no in-range group (`def_nr < defgroup_tot`) has a non-zero weight.
    pub fn is_weight_zero(&self, defgroup_tot: i32) -> bool {
        !self.dw.iter().any(|w| w.weight != 0.0 && w.def_nr < defgroup_tot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DeformVert {
        let mut d = DeformVert::default();
        d.add_index_notest(2, 0.5);
        d.add_index_notest(5, 0.25);
        d.add_index_notest(7, 1.0);
        d
    }

    #[test]
    fn find_weight_and_index() {
        let d = sample();
        assert_eq!(d.find_weight(5), 0.25);
        assert_eq!(d.find_index(7), Some(2));
        assert_eq!(d.find_weight(3), 0.0);
        assert_eq!(d.find_index(3), None);
        assert_eq!(d.find_index(-1), None);
    }

    #[test]
    fn ensure_adds_zero_weight_once() {
        let mut d = sample();
        assert_eq!(d.ensure_index(5), Some(1));
        assert_eq!(d.dw.len(), 3);
        assert_eq!(d.ensure_index(9), Some(3));
        assert_eq!(d.dw[3], DeformWeight { def_nr: 9, weight: 0.0 });
        assert_eq!(d.ensure_index(-1), None);
        assert_eq!(d.dw.len(), 4);
    }

    #[test]
    fn add_notest_allows_duplicates_and_rejects_negative() {
        let mut d = sample();
        d.add_index_notest(2, 0.9);
        assert_eq!(d.dw.len(), 4);
        assert_eq!(d.find_weight(2), 0.5); // first match wins
        d.add_index_notest(-1, 1.0);
        assert_eq!(d.dw.len(), 4);
    }

    #[test]
    fn remove_keeps_others() {
        let mut d = sample();
        d.remove_group(0);
        assert_eq!(d.dw.len(), 2);
        assert_eq!(d.find_weight(2), 0.0);
        assert_eq!(d.find_weight(5), 0.25);
        assert_eq!(d.find_weight(7), 1.0);
        // last entry moved into the hole
        assert_eq!(d.dw[0].def_nr, 7);
        d.remove_group(10); // out of range: no-op
        assert_eq!(d.dw.len(), 2);
        d.remove_group(0);
        d.remove_group(0);
        assert!(d.dw.is_empty());
    }

    #[test]
    fn clear_empties() {
        let mut d = sample();
        d.clear();
        assert!(d.dw.is_empty());
        assert_eq!(d.find_weight(2), 0.0);
    }

    #[test]
    fn weight_zero_checks_range() {
        let mut d = DeformVert::default();
        assert!(d.is_weight_zero(4));
        d.add_index_notest(1, 0.0);
        assert!(d.is_weight_zero(4));
        d.add_index_notest(6, 0.5);
        assert!(d.is_weight_zero(4)); // out of range group ignored
        assert!(!d.is_weight_zero(7));
        d.add_index_notest(2, 0.1);
        assert!(!d.is_weight_zero(4));
    }
}
