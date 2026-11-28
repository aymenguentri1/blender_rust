//! PBVH face partitioning (source/blender/blenkernel/intern/pbvh.cc: `partition_along_axis`,
//! `build_nodes_recursive_mesh` split choice) with `math::dominant_axis` (BLI_math_vector.hh).

pub type V3 = [f32; 3];

/// `math::dominant_axis`: index of the component with the greatest magnitude (ties resolve like Blender's ternary).
pub fn dominant_axis(a: V3) -> usize {
    let b = [a[0].abs(), a[1].abs(), a[2].abs()];
    if b[0] > b[1] {
        if b[0] > b[2] { 0 } else { 2 }
    } else if b[1] > b[2] {
        1
    } else {
        2
    }
}

/// `std::partition` over `faces` with `pred`: faces satisfying it first; returns the split index.
fn partition<F: Fn(i32) -> bool>(faces: &mut [i32], pred: F) -> usize {
    let mut split = 0;
    for i in 0..faces.len() {
        if pred(faces[i]) {
            faces.swap(split, i);
            split += 1;
        }
    }
    split
}

/// `partition_along_axis`: faces whose center is `>= middle` on `axis` first; returns the split index.
pub fn partition_along_axis(face_centers: &[V3], faces: &mut [i32], axis: usize, middle: f32) -> usize {
    partition(faces, |f| face_centers[f as usize][axis] >= middle)
}

/// Bounds of the centers of `faces` (`math::min_max` reduce); `None` when empty.
pub fn center_bounds(face_centers: &[V3], faces: &[i32]) -> Option<(V3, V3)> {
    let mut it = faces.iter().map(|&f| face_centers[f as usize]);
    let first = it.next()?;
    Some(it.fold((first, first), |(mut lo, mut hi), c| {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
        (lo, hi)
    }))
}

/// Split choice of `build_nodes_recursive_mesh`: dominant axis of the center bounds, midpoint of it, partition.
/// Returns `(axis, split)`; `None` for no faces.
pub fn split_faces(face_centers: &[V3], faces: &mut [i32]) -> Option<(usize, usize)> {
    let (lo, hi) = center_bounds(face_centers, faces)?;
    let axis = dominant_axis([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]);
    let middle = (lo[axis] + hi[axis]) * 0.5;
    Some((axis, partition_along_axis(face_centers, faces, axis, middle)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_axis_matches_blender_ties() {
        assert_eq!(dominant_axis([3.0, 1.0, 2.0]), 0);
        assert_eq!(dominant_axis([-1.0, -5.0, 2.0]), 1);
        assert_eq!(dominant_axis([1.0, 2.0, -9.0]), 2);
        assert_eq!(dominant_axis([1.0, 1.0, 1.0]), 2);
        assert_eq!(dominant_axis([2.0, 2.0, 1.0]), 1);
    }

    #[test]
    fn partition_splits_at_middle_without_losing_faces() {
        let centers = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let mut faces = [0, 1, 2, 3];
        let split = partition_along_axis(&centers, &mut faces, 0, 1.5);
        assert_eq!(split, 2);
        assert!(faces[..2].iter().all(|&f| centers[f as usize][0] >= 1.5));
        assert!(faces[2..].iter().all(|&f| centers[f as usize][0] < 1.5));
        let mut sorted = faces;
        sorted.sort();
        assert_eq!(sorted, [0, 1, 2, 3]);
    }

    #[test]
    fn middle_equal_goes_to_first_half() {
        let centers = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let mut faces = [1, 0];
        assert_eq!(partition_along_axis(&centers, &mut faces, 0, 1.0), 1);
        assert_eq!(faces[0], 0);
    }

    #[test]
    fn split_uses_dominant_axis_and_midpoint() {
        let centers = [[0.0, 0.0, 0.0], [0.5, 10.0, 0.0], [0.2, 4.0, 0.0], [0.1, 9.0, 0.0]];
        let mut faces = [0, 1, 2, 3];
        let (axis, split) = split_faces(&centers, &mut faces).unwrap();
        assert_eq!((axis, split), (1, 2));
        assert!(faces[..2].contains(&1) && faces[..2].contains(&3));
        assert_eq!(split_faces(&centers, &mut []), None);
    }
}
