//! `SCULPT_OT_set_pivot_position` (sculpt_transform.cc:646-1010): the sculpt transform pivot (`SculptSession::pivot_pos`)
//! set to the origin, the mean of the unmasked points or the mean of the mask border. Mesh PBVH only. ACTIVE and SURFACE
//! need the cursor (`cursor_geometry_info_update()`, `stroke_get_location_bvh()`) and are not ported.
//!
//! `average_unmasked_position()` / `average_mask_border_position()`: every point of a face (the points of the PBVH
//! leaves) adds `position * factor`; the factor is `1 - mask` (0 when hidden) resp. 1 (0 when hidden or when
//! `|mask - 0.5| > 0.2`), 0 outside the symmetry area of the old pivot, and the sum is divided by the factor total
//! (`safe_divide`: 0 when the total is 0).

#![allow(dead_code)] // wired through Scene::set_pivot_active

use crate::mesh::Mesh;
use crate::sculpt_boundary_deform::check_vertex_pivot_symmetry;

/// `PivotPositionMode` (RNA ids ORIGIN, UNMASKED, BORDER, ACTIVE, SURFACE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotMode {
    Origin,
    Unmasked,
    MaskBorder,
    ActiveVert,
    CursorSurface,
}

impl PivotMode {
    pub fn from_rna(id: &str) -> Option<PivotMode> {
        Some(match id {
            "ORIGIN" => PivotMode::Origin,
            "UNMASKED" => PivotMode::Unmasked,
            "BORDER" => PivotMode::MaskBorder,
            "ACTIVE" => PivotMode::ActiveVert,
            "SURFACE" => PivotMode::CursorSurface,
            _ => return None,
        })
    }
}

/// `mask_border_weight_calc()` threshold.
const BORDER_THRESHOLD: f32 = 0.2;

fn weighted_average(mesh: &Mesh, pivot: [f32; 3], symm: [bool; 3], factor: impl Fn(usize) -> f32) -> [f32; 3] {
    let mut used = vec![false; mesh.verts.len()];
    for f in &mesh.faces {
        for &v in f {
            used[v as usize] = true;
        }
    }
    let mut sum = [0.0f64; 3];
    let mut total = 0.0f64;
    for (i, p) in mesh.verts.iter().enumerate() {
        if !used[i] {
            continue;
        }
        let w = if check_vertex_pivot_symmetry(*p, pivot, symm) { factor(i) } else { 0.0 };
        for k in 0..3 {
            sum[k] += (p[k] * w) as f64;
        }
        total += w as f64;
    }
    if total == 0.0 {
        return [0.0; 3];
    }
    [(sum[0] / total) as f32, (sum[1] / total) as f32, (sum[2] / total) as f32]
}

fn hidden(hide_vert: Option<&[bool]>, i: usize) -> bool {
    hide_vert.is_some_and(|h| h.get(i).copied().unwrap_or(false))
}

fn mask_of(mask: Option<&[f32]>, i: usize) -> f32 {
    mask.and_then(|m| m.get(i)).copied().unwrap_or(0.0)
}

/// `average_unmasked_position()`.
pub fn average_unmasked_position(mesh: &Mesh, mask: Option<&[f32]>, hide_vert: Option<&[bool]>, pivot: [f32; 3], symm: [bool; 3]) -> [f32; 3] {
    weighted_average(mesh, pivot, symm, |i| if hidden(hide_vert, i) { 0.0 } else { 1.0 - mask_of(mask, i) })
}

/// `average_mask_border_position()`.
pub fn average_mask_border_position(mesh: &Mesh, mask: Option<&[f32]>, hide_vert: Option<&[bool]>, pivot: [f32; 3], symm: [bool; 3]) -> [f32; 3] {
    weighted_average(mesh, pivot, symm, |i| {
        if hidden(hide_vert, i) || (mask_of(mask, i) - 0.5).abs() > BORDER_THRESHOLD {
            0.0
        } else {
            1.0
        }
    })
}

/// `set_pivot_position_exec()` for the modes that need no cursor; `None` for ACTIVE and SURFACE.
pub fn set_pivot(mesh: &Mesh, mode: PivotMode, mask: Option<&[f32]>, hide_vert: Option<&[bool]>, pivot: [f32; 3], symm: [bool; 3]) -> Option<[f32; 3]> {
    match mode {
        PivotMode::Origin => Some([0.0; 3]),
        PivotMode::Unmasked => Some(average_unmasked_position(mesh, mask, hide_vert, pivot, symm)),
        PivotMode::MaskBorder => Some(average_mask_border_position(mesh, mask, hide_vert, pivot, symm)),
        PivotMode::ActiveVert | PivotMode::CursorSurface => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit square at z = 1 (points 0..=3) plus a loose point 4 that no face uses.
    fn mesh() -> Mesh {
        Mesh {
            verts: vec![[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [2.0, 2.0, 1.0], [0.0, 2.0, 1.0], [50.0, 50.0, 50.0]],
            faces: vec![vec![0, 1, 2, 3]],
        }
    }

    const NO: [bool; 3] = [false; 3];

    #[test]
    fn rna_ids() {
        assert_eq!(PivotMode::from_rna("BORDER"), Some(PivotMode::MaskBorder));
        assert_eq!(PivotMode::from_rna("SURFACE"), Some(PivotMode::CursorSurface));
        assert_eq!(PivotMode::from_rna("MASK"), None);
    }

    #[test]
    fn origin_is_zero_and_cursor_modes_are_not_ported() {
        let m = mesh();
        assert_eq!(set_pivot(&m, PivotMode::Origin, None, None, [3.0, 4.0, 5.0], NO), Some([0.0; 3]));
        assert_eq!(set_pivot(&m, PivotMode::ActiveVert, None, None, [0.0; 3], NO), None);
        assert_eq!(set_pivot(&m, PivotMode::CursorSurface, None, None, [0.0; 3], NO), None);
    }

    #[test]
    fn unmasked_is_the_mean_of_face_points_and_ignores_loose_points() {
        assert_eq!(average_unmasked_position(&mesh(), None, None, [0.0; 3], NO), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn unmasked_weights_by_one_minus_mask_and_skips_hidden() {
        let m = mesh();
        /* Points 0 and 1 fully masked: the mean of 2 and 3. */
        assert_eq!(average_unmasked_position(&m, Some(&[1.0, 1.0, 0.0, 0.0, 0.0]), None, [0.0; 3], NO), [1.0, 2.0, 1.0]);
        /* Point 3 hidden: the mean of 0, 1, 2. */
        let r = average_unmasked_position(&m, None, Some(&[false, false, false, true, false]), [0.0; 3], NO);
        assert!((r[0] - 4.0 / 3.0).abs() < 1e-6 && (r[1] - 2.0 / 3.0).abs() < 1e-6 && r[2] == 1.0);
        /* Everything masked: safe_divide gives 0. */
        assert_eq!(average_unmasked_position(&m, Some(&[1.0; 5]), None, [0.0; 3], NO), [0.0; 3]);
    }

    #[test]
    fn symmetry_drops_points_on_the_other_side_of_the_pivot() {
        let m = Mesh { verts: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], faces: vec![vec![0, 1, 2]] };
        /* Pivot x > 0 with X symmetry: the point at x = -1 is outside. */
        let r = average_unmasked_position(&m, None, None, [1.0, 0.0, 0.0], [true, false, false]);
        assert_eq!(r, [1.0, 0.5, 0.0]);
    }

    #[test]
    fn mask_border_takes_points_with_mask_within_point_two_of_half() {
        let m = mesh();
        let mask = [0.5, 0.7, 0.71, 0.29, 0.5];
        /* 0.7 is inside (|0.2| is not > 0.2), 0.71 and 0.29 are out: the mean of points 0 and 1. */
        let r = average_mask_border_position(&m, Some(&mask), None, [0.0; 3], NO);
        assert!((r[0] - 1.0).abs() < 1e-6 && r[1] == 0.0 && r[2] == 1.0);
        assert_eq!(average_mask_border_position(&m, None, None, [0.0; 3], NO), [0.0; 3], "no mask: all 0, outside the band");
    }
}
