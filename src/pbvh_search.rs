//! PBVH node search (source/blender/blenkernel/intern/pbvh.cc: `pbvh_iter_next`, `search_gather`,
//! `ray_aabb_intersect`) with `isect_aabb_aabb_v3`, `isect_ray_aabb_v3(_precalc)` (math_geom.cc).

use crate::pbvh_build::MeshNode;
use crate::pbvh_partition::V3;

/// `isect_aabb_aabb_v3` (strict overlap).
pub fn isect_aabb_aabb(min1: V3, max1: V3, min2: V3, max2: V3) -> bool {
    min1[0] < max2[0] && min1[1] < max2[1] && min1[2] < max2[2]
        && min2[0] < max1[0] && min2[1] < max1[1] && min2[2] < max1[2]
}

/// `IsectRayAABB_Precalc`.
#[derive(Clone, Copy, Debug)]
pub struct RayAabbPrecalc {
    pub origin: V3,
    pub inv_dir: V3,
    pub sign: [usize; 3],
}

/// `isect_ray_aabb_v3_precalc`.
pub fn ray_aabb_precalc(origin: V3, dir: V3) -> RayAabbPrecalc {
    let inv_dir = [1.0 / dir[0], 1.0 / dir[1], 1.0 / dir[2]];
    RayAabbPrecalc { origin, inv_dir, sign: [(inv_dir[0] < 0.0) as usize, (inv_dir[1] < 0.0) as usize, (inv_dir[2] < 0.0) as usize] }
}

/// `isect_ray_aabb_v3`: entry distance `tmin` along the ray, or `None` when missed.
pub fn isect_ray_aabb(d: &RayAabbPrecalc, bb_min: V3, bb_max: V3) -> Option<f32> {
    let bbox = [bb_min, bb_max];
    let mut tmin = (bbox[d.sign[0]][0] - d.origin[0]) * d.inv_dir[0];
    let mut tmax = (bbox[1 - d.sign[0]][0] - d.origin[0]) * d.inv_dir[0];
    let tymin = (bbox[d.sign[1]][1] - d.origin[1]) * d.inv_dir[1];
    let tymax = (bbox[1 - d.sign[1]][1] - d.origin[1]) * d.inv_dir[1];
    if tmin > tymax || tymin > tmax {
        return None;
    }
    tmin = tymin.max(tmin);
    tmax = tymax.min(tmax);
    let tzmin = (bbox[d.sign[2]][2] - d.origin[2]) * d.inv_dir[2];
    let tzmax = (bbox[1 - d.sign[2]][2] - d.origin[2]) * d.inv_dir[2];
    if tmin > tzmax || tzmin > tmax {
        return None;
    }
    Some(tzmin.max(tmin))
}

/// `search_gather`: stack walk from the root; subtrees whose node fails `scb` are skipped,
/// leaves that pass are collected (in Blender's child-0-first order).
pub fn search_gather<F: FnMut(usize) -> bool>(nodes: &[MeshNode], mut scb: F) -> Vec<usize> {
    let mut out = Vec::new();
    if nodes.is_empty() {
        return out;
    }
    let mut stack = vec![0usize];
    while let Some(i) = stack.pop() {
        if !scb(i) {
            continue;
        }
        if nodes[i].leaf {
            out.push(i);
            continue;
        }
        let c = nodes[i].children_offset;
        stack.push(c + 1);
        stack.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbvh_build::build_mesh_nodes;

    #[test]
    fn aabb_overlap_is_strict() {
        assert!(isect_aabb_aabb([0.0; 3], [1.0; 3], [0.5; 3], [2.0; 3]));
        assert!(!isect_aabb_aabb([0.0; 3], [1.0; 3], [1.0, 0.0, 0.0], [2.0, 1.0, 1.0]));
    }

    #[test]
    fn ray_hits_box_with_entry_distance() {
        let p = ray_aabb_precalc([-5.0, 0.5, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(isect_ray_aabb(&p, [0.0; 3], [1.0; 3]), Some(5.0));
        let miss = ray_aabb_precalc([-5.0, 3.0, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(isect_ray_aabb(&miss, [0.0; 3], [1.0; 3]), None);
        let neg = ray_aabb_precalc([5.0, 0.5, 0.5], [-1.0, 0.0, 0.0]);
        assert_eq!(isect_ray_aabb(&neg, [0.0; 3], [1.0; 3]), Some(4.0));
    }

    #[test]
    fn gather_all_leaves_in_order_and_prunes() {
        let c: Vec<V3> = (0..8).map(|i| [i as f32, 0.0, 0.0]).collect();
        let mut f: Vec<i32> = (0..8).collect();
        let nodes = build_mesh_nodes(&[], 2, &c, &mut f);
        let all = search_gather(&nodes, |_| true);
        assert_eq!(all.len(), 4);
        assert!(all.iter().all(|&i| nodes[i].leaf));
        let pruned = search_gather(&nodes, |i| i != 1);
        assert_eq!(pruned.len(), 2);
        assert!(search_gather(&[], |_| true).is_empty());
    }
}
