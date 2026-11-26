//! Action / F-curve data model and key insertion, ported from
//! source/blender/animrig/intern/fcurve.cc (insert_bezt_fcurve, insert_vert_fcurve),
//! source/blender/animrig/intern/keyframing.cc and makesdna/DNA_anim_types.h (FCurve, bAction).
//! Handle recalculation (BKE_fcurve_handles_recalc) is not ported yet.

use crate::fcurve_eval::{binarysearch_index, BezTriple, Ipo};

/// `BEZT_BINARYSEARCH_THRESH`.
pub const BEZT_BINARYSEARCH_THRESH: f32 = 0.01;

/// `eInsertKeyFlags` subset.
pub const INSERTKEY_REPLACE: u32 = 1 << 1;
pub const INSERTKEY_OVERWRITE_FULL: u32 = 1 << 2;

/// `FCurve` (only the fields needed for insertion).
#[derive(Clone, Debug, PartialEq)]
pub struct FCurve {
    pub rna_path: String,
    pub array_index: usize,
    pub bezt: Vec<BezTriple>,
}

impl FCurve {
    pub fn new(rna_path: &str, array_index: usize) -> Self {
        Self { rna_path: rna_path.to_string(), array_index, bezt: Vec::new() }
    }
}

/// `bAction`: a list of F-curves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Action {
    pub fcurves: Vec<FCurve>,
}

/// `replace_bezt_keyframe_ypos`: shift the key and both handles by the y delta.
fn replace_bezt_keyframe_ypos(dst: &mut BezTriple, bezt: &BezTriple) {
    let dy = bezt.vec[1][1] - dst.vec[1][1];
    for v in dst.vec.iter_mut() {
        v[1] += dy;
    }
}

/// `insert_bezt_fcurve`: returns the index of the key, or `None` when nothing could be added.
pub fn insert_bezt_fcurve(fcu: &mut FCurve, bezt: &BezTriple, flag: u32) -> Option<usize> {
    if fcu.bezt.is_empty() {
        if flag & INSERTKEY_REPLACE != 0 {
            return None;
        }
        fcu.bezt.push(*bezt);
        return Some(0);
    }
    let (i, replace) = binarysearch_index(&fcu.bezt, bezt.vec[1][0], BEZT_BINARYSEARCH_THRESH);
    if replace {
        if i < fcu.bezt.len() {
            if flag & INSERTKEY_OVERWRITE_FULL != 0 {
                fcu.bezt[i] = *bezt;
            } else {
                replace_bezt_keyframe_ypos(&mut fcu.bezt[i], bezt);
            }
        }
        Some(i)
    } else if flag & INSERTKEY_REPLACE == 0 {
        fcu.bezt.insert(i, *bezt);
        Some(i)
    } else {
        None
    }
}

/// `insert_vert_fcurve` (no cycle-aware / needed-only modes): new key at (frame, value);
/// handles start on the key (as `initialize_bezt`), interpolation is inherited from a neighbour.
pub fn insert_vert_fcurve(fcu: &mut FCurve, frame: f32, value: f32, ipo: Ipo, flag: u32) -> Option<usize> {
    let bezt = BezTriple { vec: [[frame, value]; 3], ipo };
    let old_tot = fcu.bezt.len();
    let a = insert_bezt_fcurve(fcu, &bezt, flag)?;
    if fcu.bezt.len() > 2 && flag & INSERTKEY_REPLACE == 0 && fcu.bezt.len() > old_tot {
        if a > 0 {
            fcu.bezt[a].ipo = fcu.bezt[a - 1].ipo;
        } else if a < fcu.bezt.len() - 1 {
            fcu.bezt[a].ipo = fcu.bezt[a + 1].ipo;
        }
    }
    Some(a)
}

impl Action {
    /// `action_fcurve_ensure`: find the curve for (path, index) or append a new one.
    pub fn fcurve_ensure(&mut self, rna_path: &str, array_index: usize) -> &mut FCurve {
        let pos = self.fcurves.iter().position(|f| f.rna_path == rna_path && f.array_index == array_index);
        let pos = pos.unwrap_or_else(|| {
            self.fcurves.push(FCurve::new(rna_path, array_index));
            self.fcurves.len() - 1
        });
        &mut self.fcurves[pos]
    }

    /// Key every component of a property at `frame` (one F-curve per array index).
    /// Returns the number of curves keyed.
    pub fn insert_keyframe(&mut self, rna_path: &str, values: &[f32], frame: f32, ipo: Ipo, flag: u32) -> usize {
        let mut n = 0;
        for (i, v) in values.iter().enumerate() {
            let fcu = self.fcurve_ensure(rna_path, i);
            if insert_vert_fcurve(fcu, frame, *v, ipo, flag).is_some() {
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_frames_make_three_curves_with_two_points() {
        let mut act = Action::default();
        act.insert_keyframe("location", &[1.0, 2.0, 3.0], 1.0, Ipo::Bez, 0);
        act.insert_keyframe("location", &[4.0, 5.0, 6.0], 10.0, Ipo::Bez, 0);
        assert_eq!(act.fcurves.len(), 3);
        assert!(act.fcurves.iter().all(|f| f.bezt.len() == 2));
        assert_eq!(act.fcurves[2].bezt[1].vec[1], [10.0, 6.0]);
    }

    #[test]
    fn insert_keeps_frame_order_and_replace_shifts_handles() {
        let mut f = FCurve::new("location", 0);
        insert_vert_fcurve(&mut f, 10.0, 1.0, Ipo::Bez, 0);
        insert_vert_fcurve(&mut f, 1.0, 0.0, Ipo::Bez, 0);
        assert_eq!(f.bezt[0].vec[1][0], 1.0);
        f.bezt[1].vec[2] = [12.0, 1.5];
        insert_vert_fcurve(&mut f, 10.0, 3.0, Ipo::Bez, 0);
        assert_eq!(f.bezt.len(), 2);
        assert_eq!(f.bezt[1].vec[1][1], 3.0);
        assert_eq!(f.bezt[1].vec[2], [12.0, 3.5]);
    }

    #[test]
    fn replace_only_flag_refuses_new_key_and_new_key_inherits_ipo() {
        let mut f = FCurve::new("scale", 1);
        assert!(insert_vert_fcurve(&mut f, 1.0, 0.0, Ipo::Bez, INSERTKEY_REPLACE).is_none());
        insert_vert_fcurve(&mut f, 1.0, 0.0, Ipo::Lin, 0);
        insert_vert_fcurve(&mut f, 5.0, 1.0, Ipo::Lin, 0);
        insert_vert_fcurve(&mut f, 3.0, 2.0, Ipo::Bez, 0);
        assert_eq!(f.bezt[1].ipo, Ipo::Lin);
    }

    #[test]
    fn undo_restores_snapshot() {
        let mut act = Action::default();
        act.insert_keyframe("location", &[0.0; 3], 1.0, Ipo::Bez, 0);
        let snap = act.clone();
        act.insert_keyframe("location", &[1.0; 3], 5.0, Ipo::Bez, 0);
        assert_ne!(act, snap);
        act = snap.clone();
        assert_eq!(act, snap);
    }
}
