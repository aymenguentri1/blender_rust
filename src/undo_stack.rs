//! Undo stack core (`BKE_undosys_*` in `blenkernel/intern/undo_system.cc`), generic over the step payload.
//!
//! Ported: `BKE_undosys_step_push_with_type` (steps after the active one are freed, the new step becomes active, a
//! step pushed inside a group is marked `skip`) and `BKE_undosys_stack_limit_steps_and_memory` (walks back from the
//! newest step counting non-skipped steps / summing `data_size`, then frees every step before the one it stopped at;
//! the step it stops on and all newer ones are kept). The `WITH_GLOBAL_UNDO_KEEP_ONE` memfile exclusion only concerns
//! the memfile step type and is not represented.

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
}
