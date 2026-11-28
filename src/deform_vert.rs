//! Per-vertex vertex-group weights (`MDeformVert` / `MDeformWeight`).
//!
//! Ported from Blender `blenkernel/intern/deform.cc`: `BKE_defvert_find_weight`, `_find_index`,
//! `_ensure_index`, `_add_index_notest`, `_remove_group`, `_clear`, `_is_weight_zero`,
//! `_total_selected_weight`, `_multipaint_collective_weight`, `_calc_lock_relative_weight`,
//! `_lock_relative_weight`, `_remap`, `_flip`, `_flip_merged`, `_copy_index`, `_copy_subset`, `_sync`,
//! `_normalize`, `_normalize_subset`, `_normalize_lock_map`, `_normalize_ex`, `_weight_to_rgb`.

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

    /// `BKE_defvert_remap`: group `n < map.len()` becomes `map[n]`. Blender asserts `map[n] >= 0`;
    /// negative entries are left untouched here.
    pub fn remap(&mut self, map: &[i32]) {
        for w in &mut self.dw {
            if w.def_nr >= 0 && (w.def_nr as usize) < map.len() && map[w.def_nr as usize] >= 0 {
                w.def_nr = map[w.def_nr as usize];
            }
        }
    }

    /// `BKE_defvert_flip`: like remap, but negative map entries keep the group.
    pub fn flip(&mut self, flip_map: &[i32]) {
        self.remap(flip_map);
    }

    /// `BKE_defvert_flip_merged`: each weight and its flipped group end up with their average.
    pub fn flip_merged(&mut self, flip_map: &[i32]) {
        let total = self.dw.len();
        for i in 0..total {
            let nr = self.dw[i].def_nr;
            if nr < 0 || nr as usize >= flip_map.len() || flip_map[nr as usize] < 0 {
                continue;
            }
            let cpy = self.ensure_index(flip_map[nr as usize]).unwrap();
            let weight = 0.5 * (self.dw[cpy].weight + self.dw[i].weight);
            self.dw[cpy].weight = weight;
            self.dw[i].weight = weight;
        }
    }

    /// `BKE_defvert_copy_index`: copies the source weight (creating the destination group); a missing source
    /// zeroes an existing destination weight.
    pub fn copy_index(&mut self, defgroup_dst: i32, src: &DeformVert, defgroup_src: i32) {
        if let Some(s) = src.find_index(defgroup_src) {
            if let Some(d) = self.ensure_index(defgroup_dst) {
                self.dw[d].weight = src.dw[s].weight;
            }
        } else if let Some(d) = self.find_index(defgroup_dst) {
            self.dw[d].weight = 0.0;
        }
    }

    /// `BKE_defvert_copy_subset`: copy_index for every flagged group `< vgroup_subset.len()`.
    pub fn copy_subset(&mut self, src: &DeformVert, vgroup_subset: &[bool]) {
        for (g, &on) in vgroup_subset.iter().enumerate() {
            if on {
                self.copy_index(g as i32, src, g as i32);
            }
        }
    }

    /// `BKE_defvert_sync`: copies source weights onto matching destination groups (adding missing ones when
    /// `use_ensure`). Nothing happens unless both verts have weights.
    pub fn sync(&mut self, src: &DeformVert, use_ensure: bool) {
        if src.dw.is_empty() || self.dw.is_empty() {
            return;
        }
        for w in &src.dw {
            let d = if use_ensure { self.ensure_index(w.def_nr) } else { self.find_index(w.def_nr) };
            if let Some(d) = d {
                self.dw[d].weight = w.weight;
            }
        }
    }

    /// `BKE_defvert_normalize`.
    pub fn normalize(&mut self) {
        self.normalize_ex(&[], &[], &[]);
    }

    /// `BKE_defvert_normalize_subset`.
    pub fn normalize_subset(&mut self, subset_flags: &[bool]) {
        self.normalize_ex(subset_flags, &[], &[]);
    }

    /// `BKE_defvert_normalize_lock_map`.
    pub fn normalize_lock_map(&mut self, subset_flags: &[bool], lock_flags: &[bool]) {
        self.normalize_ex(subset_flags, lock_flags, &[]);
    }

    /// `BKE_defvert_normalize_ex`: scales the (subset) weights so the vertex total is 1. Locked groups keep
    /// their weight and reduce what is available; soft-locked groups are only scaled when the regular ones
    /// cannot absorb the change. An empty flag slice means "not used".
    pub fn normalize_ex(&mut self, subset_flags: &[bool], lock_flags: &[bool], soft_lock_flags: &[bool]) {
        let flag = |f: &[bool], nr: i32| nr >= 0 && f.get(nr as usize).copied().unwrap_or(false);
        let use_subset = !subset_flags.is_empty();
        let use_locks = !lock_flags.is_empty();
        let use_soft = !soft_lock_flags.is_empty();

        if self.dw.is_empty() {
            return;
        }
        if self.dw.len() == 1 {
            let dw = &mut self.dw[0];
            if use_subset && !flag(subset_flags, dw.def_nr) {
                return;
            }
            if !(use_locks && flag(lock_flags, dw.def_nr)) {
                dw.weight = 1.0;
            }
            return;
        }

        let (mut locked, mut soft_locked, mut regular) = (0.0f32, 0.0f32, 0.0f32);
        let mut soft_count = 0;
        for dw in &self.dw {
            if use_subset && !flag(subset_flags, dw.def_nr) {
                continue;
            }
            if use_locks && flag(lock_flags, dw.def_nr) {
                locked += dw.weight;
            } else if use_soft && flag(soft_lock_flags, dw.def_nr) {
                soft_locked += dw.weight;
                soft_count += 1;
            } else {
                regular += dw.weight;
            }
        }
        let available = (1.0 - locked).max(0.0);

        if regular <= 0.0 && soft_locked <= 0.0 {
            if soft_count == 0 {
                return;
            }
            let weight = available / soft_count as f32;
            for dw in &mut self.dw {
                if use_subset && !flag(subset_flags, dw.def_nr) {
                    continue;
                }
                if use_locks && flag(lock_flags, dw.def_nr) {
                    continue;
                }
                if use_soft && flag(soft_lock_flags, dw.def_nr) {
                    dw.weight = weight;
                }
            }
            return;
        }

        let (soft_scale, regular_scale) = if soft_locked >= available || regular <= 0.0 {
            (available / soft_locked, 0.0)
        } else {
            (1.0, (available - soft_locked) / regular)
        };
        for dw in &mut self.dw {
            if use_subset && !flag(subset_flags, dw.def_nr) {
                continue;
            }
            if use_locks && flag(lock_flags, dw.def_nr) {
                continue;
            }
            if use_soft && flag(soft_lock_flags, dw.def_nr) {
                dw.weight *= soft_scale;
            } else {
                dw.weight *= regular_scale;
            }
            dw.weight = dw.weight.clamp(0.0, 1.0);
        }
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

/// `BKE_defvert_weight_to_rgb()` (deform.cc:1559): the weight paint colour ramp blue, cyan, green, yellow, red; weights
/// above 1 and NaN are magenta (negative weights follow the first branch).
pub fn weight_to_rgb(weight: f32) -> [f32; 3] {
    let blend = (weight / 2.0) + 0.5;
    if weight <= 0.25 {
        [0.0, blend * weight * 4.0, blend]
    } else if weight <= 0.50 {
        [0.0, blend, blend * (1.0 - ((weight - 0.25) * 4.0))]
    } else if weight <= 0.75 {
        [blend * ((weight - 0.50) * 4.0), blend, 0.0]
    } else if weight <= 1.0 {
        [blend, blend * (1.0 - ((weight - 0.75) * 4.0)), 0.0]
    } else {
        [1.0, 0.0, 1.0]
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

    #[test]
    fn remap_and_flip() {
        let mut d = sample(); // 2,5,7
        d.remap(&[0, 1, 4, 3, 3, 9]); // groups 0..5; 7 is out of range
        assert_eq!(d.dw.iter().map(|w| w.def_nr).collect::<Vec<_>>(), vec![4, 9, 7]);
        let mut f = sample();
        f.flip(&[0, 1, 5, 3, 4, 2]); // 2<->5, 7 out of range
        assert_eq!(f.find_weight(5), 0.5);
        assert_eq!(f.find_weight(2), 0.25);
        assert_eq!(f.find_weight(7), 1.0);
        let mut n = sample();
        n.flip(&[0, 1, -1, 3, 4, -1]); // negative: group kept
        assert_eq!(n, sample());
    }

    #[test]
    fn flip_merged_averages_pairs() {
        let mut d = sample(); // 2:0.5, 5:0.25
        d.flip_merged(&[0, 1, 5, 3, 4, 2]);
        assert_eq!(d.find_weight(2), 0.375);
        assert_eq!(d.find_weight(5), 0.375);
        let mut e = DeformVert::default();
        e.add_index_notest(2, 0.5);
        e.flip_merged(&[0, 1, 5, 3, 4, 2]); // only one side assigned: halved
        assert_eq!(e.find_weight(2), 0.25);
        assert_eq!(e.find_weight(5), 0.25);
    }

    #[test]
    fn copy_subset_and_index() {
        let src = sample();
        let mut dst = DeformVert::default();
        dst.add_index_notest(3, 0.8);
        dst.copy_subset(&src, &[false, false, true, true, false, true]);
        assert_eq!(dst.find_weight(2), 0.5);
        assert_eq!(dst.find_weight(5), 0.25);
        assert_eq!(dst.find_weight(3), 0.0); // source missing: existing dst zeroed
        assert_eq!(dst.find_index(0), None); // not in subset
        let mut e = DeformVert::default();
        e.copy_index(1, &src, 4); // missing source does not create
        assert!(e.dw.is_empty());
    }

    #[test]
    fn sync_with_and_without_ensure() {
        let src = sample(); // 2,5,7
        let mut dst = DeformVert::default();
        dst.add_index_notest(5, 0.0);
        dst.sync(&src, false);
        assert_eq!(dst.dw.len(), 1);
        assert_eq!(dst.find_weight(5), 0.25);
        dst.sync(&src, true);
        assert_eq!(dst.dw.len(), 3);
        assert_eq!(dst.find_weight(7), 1.0);
        let mut empty = DeformVert::default();
        empty.sync(&src, true); // dst without weights: nothing
        assert!(empty.dw.is_empty());
    }

    fn dv(ws: &[(i32, f32)]) -> DeformVert {
        let mut d = DeformVert::default();
        for &(g, w) in ws {
            d.add_index_notest(g, w);
        }
        d
    }
    fn total(d: &DeformVert) -> f32 {
        d.dw.iter().map(|w| w.weight).sum()
    }

    #[test]
    fn normalize_sums_to_one_keeping_ratios() {
        let mut d = dv(&[(0, 0.2), (1, 0.6)]);
        d.normalize();
        assert!((d.find_weight(0) - 0.25).abs() < 1e-6);
        assert!((d.find_weight(1) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn normalize_single_group_becomes_one_even_if_zero() {
        let mut d = dv(&[(3, 0.0)]);
        d.normalize();
        assert_eq!(d.find_weight(3), 1.0);
        let mut e = dv(&[(3, 0.4)]);
        e.normalize_subset(&[false, false, false, false]); // not in subset: untouched
        assert_eq!(e.find_weight(3), 0.4);
        let mut l = dv(&[(1, 0.4)]);
        l.normalize_lock_map(&[], &[false, true]);
        assert_eq!(l.find_weight(1), 0.4);
    }

    #[test]
    fn normalize_all_zero_is_left_alone() {
        let mut d = dv(&[(0, 0.0), (1, 0.0)]);
        d.normalize();
        assert_eq!(total(&d), 0.0);
        let mut e = DeformVert::default();
        e.normalize();
        assert!(e.dw.is_empty());
    }

    #[test]
    fn normalize_subset_only_touches_subset() {
        let mut d = dv(&[(0, 0.5), (1, 0.5), (2, 0.4)]);
        d.normalize_subset(&[true, true, false]);
        assert_eq!(d.find_weight(2), 0.4);
        assert!((d.find_weight(0) + d.find_weight(1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_lock_map_keeps_locked_and_fills_rest() {
        let mut d = dv(&[(0, 0.5), (1, 0.2), (2, 0.2)]);
        d.normalize_lock_map(&[], &[true, false, false]);
        assert_eq!(d.find_weight(0), 0.5);
        assert!((d.find_weight(1) - 0.25).abs() < 1e-6);
        assert!((d.find_weight(2) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn normalize_locked_over_one_zeroes_the_rest() {
        let mut d = dv(&[(0, 1.5), (1, 0.3)]);
        d.normalize_lock_map(&[], &[true, false]);
        assert_eq!(d.find_weight(0), 1.5);
        assert_eq!(d.find_weight(1), 0.0);
    }

    #[test]
    fn normalize_soft_lock_scenarios() {
        // regular absorbs the change: soft keeps 0.3
        let mut d = dv(&[(0, 0.3), (1, 0.2), (2, 0.2)]);
        d.normalize_ex(&[], &[], &[true, false, false]);
        assert!((d.find_weight(0) - 0.3).abs() < 1e-6);
        assert!((d.find_weight(1) - 0.35).abs() < 1e-6);
        // soft too large: it is scaled and regular goes to zero
        let mut e = dv(&[(0, 1.5), (1, 0.2)]);
        e.normalize_ex(&[], &[], &[true, false]);
        assert!((e.find_weight(0) - 1.0).abs() < 1e-6);
        assert_eq!(e.find_weight(1), 0.0);
        // all zero but soft-locked groups exist: split equally
        let mut z = dv(&[(0, 0.0), (1, 0.0), (2, 0.0)]);
        z.normalize_ex(&[], &[], &[true, true, false]);
        assert_eq!(z.find_weight(0), 0.5);
        assert_eq!(z.find_weight(1), 0.5);
        assert_eq!(z.find_weight(2), 0.0);
    }

    #[test]
    fn weight_to_rgb_ramp_stops() {
        assert_eq!(weight_to_rgb(0.0), [0.0, 0.0, 0.5]);
        assert_eq!(weight_to_rgb(0.25), [0.0, 0.625, 0.625]);
        assert_eq!(weight_to_rgb(0.5), [0.0, 0.75, 0.0]);
        assert_eq!(weight_to_rgb(0.75), [0.875, 0.875, 0.0]);
        assert_eq!(weight_to_rgb(1.0), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn weight_to_rgb_out_of_range_is_magenta() {
        assert_eq!(weight_to_rgb(1.5), [1.0, 0.0, 1.0]);
        assert_eq!(weight_to_rgb(f32::NAN), [1.0, 0.0, 1.0]);
        assert_eq!(weight_to_rgb(f32::INFINITY), [1.0, 0.0, 1.0]);
    }
}
