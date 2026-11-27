//! Scene Project brush (editors/sculpt_paint/mesh/brushes/scene_project.cc): vertices are moved along the ray
//! direction onto the closest hit of the scene targets. Ports `absolute_min_distance()`, `scene_raycast()` and
//! `calc_translations()` (:60-215) and `calc_normal()` (:217-228). Deviation: the BVH ray cast is a closure
//! given by the caller (no `ProjectBrushTarget`, active-to-target matrix or threading).

/// `BVH_RAYCAST_DIST_MAX` (`FLT_MAX / 2.0f`, blenkernel/BKE_bvh.hh).
pub const BVH_RAYCAST_DIST_MAX: f32 = f32::MAX / 2.0;

/// `absolute_min_distance()`.
pub fn absolute_min_distance(d1: f32, d2: f32) -> f32 {
    if d1.abs() < d2.abs() { d1 } else { d2 }
}

/// `BRUSH_PROJECT_RAY_DIRECTION_VIEW_NORMAL` / `_PLANE_NORMAL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayDirection {
    ViewNormal,
    PlaneNormal,
}

/// `calc_normal()`: the negated symmetric view or sculpt normal.
pub fn calc_normal(dir: RayDirection, view_normal_symm: [f32; 3], sculpt_normal_symm: [f32; 3]) -> [f32; 3] {
    let n = match dir {
        RayDirection::ViewNormal => view_normal_symm,
        RayDirection::PlaneNormal => sculpt_normal_symm,
    };
    n.map(|c| -c)
}

/// `scene_raycast()` with a single ray cast function `cast(position, direction) -> Option<distance>`: rays are
/// skipped for zero factors, `bidirectional` also casts backwards (negating that distance), vertices without
/// a hit get distance 0 and the others `distance - minimum_distance`.
pub fn scene_raycast(
    bidirectional: bool,
    minimum_distance: f32,
    normal: [f32; 3],
    positions: &[[f32; 3]],
    factors: &[f32],
    cast: impl Fn([f32; 3], [f32; 3]) -> Option<f32>,
) -> Vec<f32> {
    let back = normal.map(|c| -c);
    positions
        .iter()
        .zip(factors)
        .map(|(&p, &f)| {
            let mut best = BVH_RAYCAST_DIST_MAX;
            if f != 0.0 {
                if let Some(d) = cast(p, normal) {
                    best = absolute_min_distance(best, d);
                }
                if bidirectional {
                    if let Some(d) = cast(p, back) {
                        best = absolute_min_distance(best, -d);
                    }
                }
            }
            if best.abs() == BVH_RAYCAST_DIST_MAX { 0.0 } else { best - minimum_distance }
        })
        .collect()
}

/// `calc_translations()`: `normal * hit_distance * factor`.
pub fn calc_translations(normal: [f32; 3], factors: &[f32], hit_distances: &[f32]) -> Vec<[f32; 3]> {
    factors.iter().zip(hit_distances).map(|(&f, &d)| normal.map(|c| c * d * f)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Plane z = 0 hit by rays that point down (-Z). */
    fn plane(p: [f32; 3], d: [f32; 3]) -> Option<f32> {
        if d[2] == 0.0 { return None; }
        let t = -p[2] / d[2];
        (t >= 0.0).then_some(t)
    }

    #[test]
    fn vertex_above_a_plane_projects_onto_it_with_falloff() {
        let n = calc_normal(RayDirection::ViewNormal, [0.0, 0.0, 1.0], [0.0; 3]);
        assert_eq!(n, [0.0, 0.0, -1.0]);
        let pos = [[0.0, 0.0, 2.0], [1.0, 0.0, 2.0]];
        let hits = scene_raycast(false, 0.0, n, &pos, &[1.0, 0.5], plane);
        assert_eq!(hits, vec![2.0, 2.0]);
        let tr = calc_translations(n, &[1.0, 0.5], &hits);
        assert_eq!(tr, vec![[0.0, 0.0, -2.0], [0.0, 0.0, -1.0]]);
    }

    #[test]
    fn zero_factor_or_miss_leaves_the_vertex_and_minimum_distance_offsets() {
        let n = [0.0, 0.0, -1.0];
        let below = [[0.0, 0.0, -1.0]];
        assert_eq!(scene_raycast(false, 0.0, n, &below, &[1.0], plane), vec![0.0]);
        assert_eq!(scene_raycast(false, 0.0, n, &[[0.0, 0.0, 2.0]], &[0.0], plane), vec![0.0]);
        assert_eq!(scene_raycast(false, 0.5, n, &[[0.0, 0.0, 2.0]], &[1.0], plane), vec![1.5]);
    }

    #[test]
    fn bidirectional_takes_the_closest_hit_and_negates_the_backward_one() {
        let n = [0.0, 0.0, -1.0];
        let r = scene_raycast(true, 0.0, n, &[[0.0, 0.0, -1.0]], &[1.0], plane);
        assert_eq!(r, vec![-1.0]);
        assert_eq!(absolute_min_distance(-1.0, 2.0), -1.0);
    }
}
