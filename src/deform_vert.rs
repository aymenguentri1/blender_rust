//! Per-vertex vertex-group weights (`MDeformVert` / `MDeformWeight`).
//!
//! Ported from Blender `blenkernel/intern/deform.cc`: `BKE_defvert_find_weight`, `_find_index`,
//! `_ensure_index`, `_add_index_notest`, `_remove_group`, `_clear`, `_is_weight_zero`,
//! `_total_selected_weight`, `_multipaint_collective_weight`, `_calc_lock_relative_weight`,
//! `_lock_relative_weight`.

#![allow(dead_code)]

/// `VERTEX_WEIGHT_LOCK_EPSILON` (BKE_deform.hh).
pub const VERTEX_WEIGHT_LOCK_EPSILON: f32 = 1e-6;

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

    /// `BKE_defvert_total_selected_weight`: sum of weights of groups `< defbase_num` flagged in `defbase_sel`.
    /// `None` selection gives 0.
    pub fn total_selected_weight(&self, defbase_num: i32, defbase_sel: Option<&[bool]>) -> f32 {
        let Some(sel) = defbase_sel else { return 0.0 };
        self.dw
            .iter()
            .filter(|w| w.def_nr < defbase_num && w.def_nr >= 0 && sel.get(w.def_nr as usize).copied().unwrap_or(false))
            .map(|w| w.weight)
            .sum()
    }

    /// `BKE_defvert_multipaint_collective_weight`: the sum when auto-normalize is active, else the average.
    pub fn multipaint_collective_weight(
        &self,
        defbase_num: i32,
        defbase_sel: Option<&[bool]>,
        defbase_sel_num: i32,
        is_normalized: bool,
    ) -> f32 {
        let mut total = self.total_selected_weight(defbase_num, defbase_sel);
        if !is_normalized {
            total /= defbase_sel_num as f32;
        }
        total
    }

    /// `BKE_defvert_lock_relative_weight`.
    pub fn lock_relative_weight(
        &self,
        weight: f32,
        defbase_num: i32,
        defbase_locked: Option<&[bool]>,
        defbase_unlocked: Option<&[bool]>,
    ) -> f32 {
        let unlocked = self.total_selected_weight(defbase_num, defbase_unlocked);
        if unlocked > 0.0 {
            return weight / unlocked;
        }
        let locked = self.total_selected_weight(defbase_num, defbase_locked);
        calc_lock_relative_weight(weight, locked, unlocked)
    }
}

/// `BKE_defvert_calc_lock_relative_weight`.
pub fn calc_lock_relative_weight(weight: f32, locked_weight: f32, unlocked_weight: f32) -> f32 {
    if unlocked_weight > 0.0 {
        return weight / unlocked_weight;
    }
    if locked_weight <= 0.0 {
        return weight;
    }
    if locked_weight >= 1.0 - VERTEX_WEIGHT_LOCK_EPSILON {
        return if weight != 0.0 { 1.0 } else { 0.0 };
    }
    weight / (1.0 - locked_weight)
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

    #[test]
    fn total_selected_respects_mask_and_range() {
        let d = sample(); // groups 2:0.5, 5:0.25, 7:1.0
        let sel = [false, false, true, false, false, true, false, true];
        assert_eq!(d.total_selected_weight(8, Some(&sel)), 1.75);
        assert_eq!(d.total_selected_weight(6, Some(&sel)), 0.75); // group 7 out of range
        assert_eq!(d.total_selected_weight(8, None), 0.0);
    }

    #[test]
    fn multipaint_sum_or_average() {
        let d = sample();
        let sel = [false, false, true, false, false, true, false, false];
        assert_eq!(d.multipaint_collective_weight(8, Some(&sel), 2, true), 0.75);
        assert_eq!(d.multipaint_collective_weight(8, Some(&sel), 2, false), 0.375);
    }

    #[test]
    fn calc_lock_relative_cases() {
        assert_eq!(calc_lock_relative_weight(0.3, 0.5, 0.6), 0.5); // unlocked normalizes
        assert_eq!(calc_lock_relative_weight(0.3, 0.0, 0.0), 0.3); // nothing locked
        assert_eq!(calc_lock_relative_weight(0.3, 1.0, 0.0), 1.0); // fully locked, non-zero
        assert_eq!(calc_lock_relative_weight(0.0, 1.0, 0.0), 0.0); // 0/0 -> 0
        assert_eq!(calc_lock_relative_weight(0.25, 0.5, 0.0), 0.5); // 0.25 / (1 - 0.5)
    }

    #[test]
    fn lock_relative_uses_masks() {
        let d = sample(); // 2:0.5, 5:0.25, 7:1.0
        let locked = [false, false, true, false, false, false, false, false];
        let unlocked = [false, false, false, false, false, true, false, false];
        assert_eq!(d.lock_relative_weight(0.125, 8, Some(&locked), Some(&unlocked)), 0.5);
        // no unlocked weight present: falls back to locked (0.5)
        let none = [false; 8];
        assert_eq!(d.lock_relative_weight(0.25, 8, Some(&locked), Some(&none)), 0.5);
    }
}
