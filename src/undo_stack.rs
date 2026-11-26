//! Undo stack core (`BKE_undosys_*` in `blenkernel/intern/undo_system.cc`), generic over the step payload.
//!
//! Ported: `BKE_undosys_step_push_with_type` (steps after the active one are freed, the new step becomes active, a
//! step pushed inside a group is marked `skip`) and `BKE_undosys_stack_limit_steps_and_memory` (walks back from the
//! newest step counting non-skipped steps / summing `data_size`, then frees every step before the one it stopped at;
//! the step it stops on and all newer ones are kept). The `WITH_GLOBAL_UNDO_KEEP_ONE` memfile exclusion only concerns
//! the memfile step type and is not represented.

/// `eUndoStepDir`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDir {
    Undo,
    Redo,
    Invalid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndoStep<T> {
    pub name: String,
    pub data: T,
    pub data_size: usize,
    pub skip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndoStack<T> {
    pub steps: Vec<UndoStep<T>>,
    /// `ustack->step_active`: index into `steps`, `None` when empty.
    pub active: Option<usize>,
    /// `ustack->group_level`.
    pub group_level: i32,
}

impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        UndoStack { steps: Vec::new(), active: None, group_level: 0 }
    }
}

impl<T> UndoStack<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `BKE_undosys_step_push_with_type`.
    pub fn push(&mut self, name: &str, data: T, data_size: usize) {
        // "Remove all undo-steps after (also when `step_active == nullptr`)".
        self.steps.truncate(self.active.map_or(0, |a| a + 1));
        self.steps.push(UndoStep { name: name.to_string(), data, data_size, skip: false });
        self.active = Some(self.steps.len() - 1);
        if self.group_level > 0 {
            // "Temporarily set skip for the active step" until the last group ends.
            self.steps.last_mut().unwrap().skip = true;
        }
    }

    /// `BKE_undosys_stack_has_undo`: with a name, the newest step of that name must have a step before it.
    pub fn has_undo(&self, name: Option<&str>) -> bool {
        match name {
            Some(n) => self.steps.iter().rposition(|s| s.name == n).is_some_and(|i| i > 0),
            None => !self.steps.is_empty(),
        }
    }

    /// `BKE_undosys_stack_has_redo`.
    pub fn has_redo(&self) -> bool {
        self.active.is_some_and(|a| a + 1 < self.steps.len())
    }

    /// `BKE_undosys_step_calc_direction`: the reference defaults to the active step; the active step itself counts as
    /// an undo target.
    pub fn calc_direction(&self, target: Option<usize>, reference: Option<usize>) -> StepDir {
        let Some(r) = reference.or(self.active) else { return StepDir::Invalid };
        if target == Some(r) || (r > 0 && target == Some(r - 1)) {
            return StepDir::Undo;
        }
        if target == Some(r + 1) {
            return StepDir::Redo;
        }
        match target {
            Some(t) if t > r && t < self.steps.len() => StepDir::Redo,
            Some(t) if t < r => StepDir::Undo,
            _ => StepDir::Invalid,
        }
    }

    /// `BKE_undosys_step_load_data_ex`: walks from the reference step to the target, calling `decode(step, dir,
    /// is_final)` for each step read (`step_decode`) and moving the active step onto it. With `use_skip` the walk
    /// continues past `skip` steps; it fails (false) when it runs off the stack. `decode_active_step` is
    /// `UNDOTYPE_FLAG_DECODE_ACTIVE_STEP` (an undo re-reads the reference step itself).
    pub fn load_data_ex(
        &mut self,
        target: Option<usize>,
        reference: Option<usize>,
        use_skip: bool,
        decode_active_step: bool,
        mut decode: impl FnMut(&UndoStep<T>, StepDir, bool),
    ) -> bool {
        let Some(target) = target.filter(|&t| t < self.steps.len()) else { return false };
        let Some(reference) = reference.or(self.active) else { return false };
        let dir = self.calc_direction(Some(target), Some(reference));
        if dir == StepDir::Invalid {
            return false;
        }
        let step_on = |i: usize| -> Option<usize> {
            if dir == StepDir::Undo { i.checked_sub(1) } else { (i + 1 < self.steps.len()).then_some(i + 1) }
        };
        let mut target_active = Some(target);
        if use_skip {
            while let Some(t) = target_active {
                if !self.steps[t].skip {
                    break;
                }
                target_active = step_on(t);
            }
            if target_active.is_none() {
                return false; // "undo limit exceeded"
            }
        }
        // `undosys_step_iter_first`
        let mut iter = match (dir, decode_active_step) {
            (StepDir::Undo, true) => Some(reference),
            _ => step_on(reference),
        };
        while let Some(i) = iter {
            let is_final = Some(i) == target_active;
            decode(&self.steps[i], dir, is_final);
            self.active = Some(i);
            if is_final {
                return true;
            }
            iter = step_on(i);
        }
        false
    }

    /// `BKE_undosys_step_load_data`: skipped steps are not stepped over.
    pub fn load_data(&mut self, target: Option<usize>, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) -> bool {
        self.load_data_ex(target, None, false, false, decode)
    }

    /// `BKE_undosys_step_load_from_index`.
    pub fn load_from_index(&mut self, index: usize, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) {
        if index < self.steps.len() && Some(index) != self.active {
            self.load_data(Some(index), decode);
        }
    }

    /// `BKE_undosys_step_undo`: loads the step before the active one, stepping over skipped steps.
    pub fn undo(&mut self, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) -> bool {
        match self.active {
            Some(a) => self.load_data_ex(a.checked_sub(1), Some(a), true, false, decode),
            None => false,
        }
    }

    /// `BKE_undosys_step_redo`: loads the step after the active one, stepping over skipped steps.
    pub fn redo(&mut self, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) -> bool {
        match self.active {
            Some(a) => self.load_data_ex(Some(a + 1), Some(a), true, false, decode),
            None => false,
        }
    }

    /// `BKE_undosys_step_find_by_name`: the newest step with that name (`BLI_rfindstring`).
    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        self.steps.iter().rposition(|s| s.name == name)
    }

    /// `ed_undo_step_by_name`: undoes the step called `name`, i.e. loads the step before it (stepping over skipped
    /// steps, in whichever direction that lies). True = `OPERATOR_FINISHED`, false = `OPERATOR_CANCELLED` (unknown
    /// name, or the first step which cannot be undone).
    pub fn step_by_name(&mut self, name: &str, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) -> bool {
        let Some(found) = self.find_by_name(name) else { return false };
        let Some(target) = found.checked_sub(1) else { return false };
        self.load_data_ex(Some(target), None, true, false, decode);
        true
    }

    /// `ed_undo_step_by_index`: loads the step at `index` (several steps at once). Cancelled when it is already the
    /// active step; an out of range index is refused (Blender would assert on a null step).
    pub fn step_by_index(&mut self, index: usize, decode: impl FnMut(&UndoStep<T>, StepDir, bool)) -> bool {
        if index >= self.steps.len() || Some(index) == self.active {
            return false;
        }
        self.load_data(Some(index), decode);
        true
    }

    /// `BKE_undosys_stack_clear_active`: removes the active step and all following ones; the new active step is the
    /// one before it after rewinding over `skip` steps.
    pub fn clear_active(&mut self) {
        let Some(a) = self.active else { return };
        let mut us = a.checked_sub(1);
        while let Some(i) = us {
            if !self.steps[i].skip {
                break;
            }
            us = i.checked_sub(1);
        }
        self.active = us;
        self.steps.truncate(us.map_or(0, |i| i + 1));
    }

    /// `ed_undo_grouped_check_by_name`: the active step has this name.
    pub fn grouped_check_by_name(&self, name: &str) -> bool {
        self.active.is_some_and(|a| self.steps[a].name == name)
    }

    /// `ED_undo_grouped_push`: a push with the name of the active step replaces that step (it is cleared first), so
    /// repeated pushes of one operator stay a single step.
    pub fn grouped_push(&mut self, name: &str, data: T, data_size: usize) {
        if self.grouped_check_by_name(name) {
            self.clear_active();
        }
        self.push(name, data, data_size);
    }

    /// `BKE_undosys_stack_group_begin`.
    pub fn group_begin(&mut self) {
        debug_assert!(self.group_level >= 0);
        self.group_level += 1;
    }

    /// `BKE_undosys_stack_group_end`: the last end makes the active step a normal (non-skip) step again.
    pub fn group_end(&mut self) {
        self.group_level -= 1;
        debug_assert!(self.group_level >= 0);
        if self.group_level == 0 {
            if let Some(a) = self.active {
                self.steps[a].skip = false;
            }
        }
    }

    /// `BKE_undosys_stack_limit_steps_and_memory`; `steps == -1` and `memory_limit == 0` mean unlimited.
    pub fn limit_steps_and_memory(&mut self, steps: i32, memory_limit: usize) {
        if steps == -1 && memory_limit == 0 {
            return;
        }
        let (mut data_size_all, mut us_count) = (0usize, 0i64);
        // `for (us = last; us && us->prev; us = us->prev)`: `us` ends on the step the loop stopped at.
        let mut us = self.steps.len().checked_sub(1);
        while let Some(i) = us {
            if i == 0 {
                break;
            }
            if memory_limit != 0 {
                data_size_all += self.steps[i].data_size;
                if data_size_all > memory_limit {
                    break;
                }
            }
            if steps != -1 {
                if us_count == steps as i64 {
                    break;
                }
                if !self.steps[i].skip {
                    us_count += 1;
                }
            }
            us = Some(i - 1);
        }
        // `undosys_stack_clear_all_first(ustack, us->prev, ..)`: free from the first step up to `us->prev`.
        if let Some(i) = us {
            self.steps.drain(..i);
            self.active = self.active.map(|a| a.saturating_sub(i));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(n: usize) -> UndoStack<usize> {
        let mut s = UndoStack::new();
        for i in 0..n {
            s.push(&format!("s{i}"), i, 10);
        }
        s
    }

    #[test]
    fn push_sets_active_and_name() {
        let s = stack(3);
        assert_eq!((s.steps.len(), s.active), (3, Some(2)));
        assert_eq!(s.steps[2].name, "s2");
        assert!(!s.steps[2].skip);
    }

    #[test]
    fn push_after_undo_drops_redo_steps() {
        let mut s = stack(4);
        s.active = Some(1);
        s.push("new", 9, 10);
        assert_eq!(s.steps.iter().map(|x| x.data).collect::<Vec<_>>(), vec![0, 1, 9]);
        assert_eq!(s.active, Some(2));
        // active == None (before the first step) frees everything
        s.active = None;
        s.push("only", 7, 1);
        assert_eq!((s.steps.len(), s.active), (1, Some(0)));
    }

    #[test]
    fn push_inside_group_is_skip() {
        let mut s = stack(1);
        s.group_level = 1;
        s.push("grouped", 5, 1);
        assert!(s.steps[1].skip);
    }

    #[test]
    fn step_limit_removes_oldest_keeps_active() {
        let mut s = stack(6);
        s.limit_steps_and_memory(2, 0);
        // the walk counts steps 5 and 4, stops on step 3 (`us`) and frees everything before it (`us->prev` and older)
        assert_eq!(s.steps.iter().map(|x| x.data).collect::<Vec<_>>(), vec![3, 4, 5]);
        assert_eq!(s.active, Some(2));
        assert_eq!(s.steps[s.active.unwrap()].data, 5);
    }

    #[test]
    fn step_limit_counts_only_non_skipped_and_noop_unlimited() {
        let mut s = stack(6);
        s.steps[5].skip = true;
        s.limit_steps_and_memory(2, 0);
        assert_eq!(s.steps.len(), 4); // the skipped newest step is not counted, so the walk reaches step 2
        let mut t = stack(6);
        t.limit_steps_and_memory(-1, 0);
        assert_eq!(t.steps.len(), 6);
    }

    #[test]
    fn memory_limit_removes_oldest() {
        let mut s = stack(6); // 10 bytes each
        s.limit_steps_and_memory(-1, 25);
        assert_eq!(s.steps.iter().map(|x| x.data).collect::<Vec<_>>(), vec![3, 4, 5]);
        let mut t = stack(6);
        t.limit_steps_and_memory(-1, 1); // a tiny limit stops on the newest step, which always stays
        assert_eq!(t.steps.len(), 1);
        let mut u = stack(2);
        u.limit_steps_and_memory(0, 0);
        assert_eq!(u.steps.len(), 1); // steps == 0: the walk stops on the newest step and frees the older one
    }

    #[test]
    fn has_undo_redo() {
        let mut s = stack(3);
        assert!(s.has_undo(None) && !s.has_redo());
        assert!(s.has_undo(Some("s2")) && !s.has_undo(Some("s0")) && !s.has_undo(Some("nope")));
        s.active = Some(1);
        assert!(s.has_redo());
        assert!(!UndoStack::<u8>::new().has_undo(None));
    }

    #[test]
    fn undo_redo_move_active_and_stop_at_ends() {
        let mut s = stack(3);
        let mut seen = Vec::new();
        assert!(s.undo(|st, d, fin| seen.push((st.data, d, fin))));
        assert_eq!((s.active, seen.clone()), (Some(1), vec![(1, StepDir::Undo, true)]));
        assert!(s.undo(|_, _, _| {}));
        assert!(!s.undo(|_, _, _| {})); // nothing before the initial step
        assert_eq!(s.active, Some(0));
        assert!(s.redo(|_, _, _| {}) && s.redo(|_, _, _| {}));
        assert!(!s.redo(|_, _, _| {}));
        assert_eq!(s.active, Some(2));
    }

    #[test]
    fn undo_steps_over_skipped_steps() {
        let mut s = stack(4);
        s.steps[2].skip = true;
        let mut seen = Vec::new();
        assert!(s.undo(|st, _, fin| seen.push((st.data, fin))));
        // from step 3 the target 2 is skipped: it is decoded (not final), then step 1 is the final state
        assert_eq!((s.active, &seen[..]), (Some(1), &[(2, false), (1, true)][..]));
        s.steps[0].skip = true;
        assert!(!s.undo(|_, _, _| {})); // target 0 is skipped and nothing lies before it
    }

    #[test]
    fn load_from_index_jumps_both_directions() {
        let mut s = stack(5);
        let mut seen = Vec::new();
        s.load_from_index(1, |st, d, fin| seen.push((st.data, d, fin)));
        assert_eq!(s.active, Some(1));
        assert_eq!(seen, vec![(3, StepDir::Undo, false), (2, StepDir::Undo, false), (1, StepDir::Undo, true)]);
        seen.clear();
        s.load_from_index(4, |st, d, fin| seen.push((st.data, d, fin)));
        assert_eq!((s.active, seen.len(), seen[2]), (Some(4), 3, (4, StepDir::Redo, true)));
        s.load_from_index(4, |_, _, _| panic!("already active"));
        assert_eq!(s.calc_direction(Some(4), None), StepDir::Undo); // the active step counts as undo
        assert_eq!(s.calc_direction(Some(9), None), StepDir::Invalid);
    }

    #[test]
    fn find_by_name_returns_newest_match() {
        let mut s = stack(3);
        s.push("s1", 9, 1);
        assert_eq!(s.find_by_name("s1"), Some(3));
        assert_eq!(s.find_by_name("s0"), Some(0));
        assert_eq!(s.find_by_name("nope"), None);
    }

    #[test]
    fn step_by_name_loads_previous_step_or_cancels() {
        let mut s = stack(5);
        assert!(s.step_by_name("s3", |_, _, _| {})); // undoing "s3" lands on s2
        assert_eq!(s.active, Some(2));
        assert!(s.step_by_name("s4", |_, _, _| {})); // "s4" is ahead: redo up to the step before it
        assert_eq!(s.active, Some(3));
        assert!(!s.step_by_name("s0", |_, _, _| panic!("no step before it")));
        assert!(!s.step_by_name("unknown", |_, _, _| panic!("not found")));
        assert_eq!(s.active, Some(3));
    }

    #[test]
    fn step_by_index_checks_range_and_active() {
        let mut s = stack(4);
        assert!(!s.step_by_index(3, |_, _, _| panic!("already active")));
        assert!(!s.step_by_index(4, |_, _, _| panic!("out of range")));
        assert!(s.step_by_index(0, |_, _, _| {}));
        assert_eq!(s.active, Some(0));
        assert!(s.step_by_index(2, |_, _, _| {}));
        assert_eq!(s.active, Some(2));
    }

    #[test]
    fn grouped_push_same_name_replaces_active_step() {
        let mut s = stack(3); // s0 s1 s2
        s.grouped_push("s2", 7, 1);
        assert_eq!(s.steps.iter().map(|x| x.data).collect::<Vec<_>>(), vec![0, 1, 7]);
        assert_eq!(s.steps.len(), 3); // no new step
        s.grouped_push("other", 8, 1);
        assert_eq!(s.steps.len(), 4);
        assert!(s.grouped_check_by_name("other") && !s.grouped_check_by_name("s2"));
        assert!(!UndoStack::<u8>::new().grouped_check_by_name("x"));
    }

    #[test]
    fn clear_active_rewinds_over_skip_and_drops_redo() {
        let mut s = stack(5);
        s.steps[2].skip = true;
        s.active = Some(3);
        s.clear_active(); // prev of 3 is skipped 2, so the active step becomes 1; 3 and 4 are freed
        assert_eq!((s.active, s.steps.len()), (Some(1), 2));
        let mut t = stack(1);
        t.clear_active();
        assert_eq!((t.active, t.steps.len()), (None, 0));
    }

    #[test]
    fn group_begin_end_nesting() {
        let mut s = UndoStack::new();
        s.push("a", 0, 1);
        s.group_begin();
        s.group_begin();
        s.push("b", 1, 1);
        s.push("c", 2, 1);
        assert!(s.steps[1].skip && s.steps[2].skip);
        s.group_end();
        assert!(s.steps[2].skip, "inner end keeps the group open");
        s.group_end();
        // only the last step of the group becomes visible
        assert_eq!((s.group_level, s.steps[1].skip, s.steps[2].skip), (0, true, false));
        s.group_begin();
        s.group_end(); // no push inside the group: nothing changes
        assert!(!s.steps[2].skip);
    }
}
