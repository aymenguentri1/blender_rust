//! PBVH mesh node build (source/blender/blenkernel/intern/pbvh.cc: `partition_material_indices`,
//! `leaf_needs_material_split`, `build_nodes_recursive_mesh`).

use crate::pbvh_partition::{center_bounds, dominant_axis, partition_along_axis, V3};

/// `STACK_FIXED_DEPTH` in pbvh.cc.
pub const STACK_FIXED_DEPTH: usize = 100;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshNode {
    pub parent: i32,
    pub leaf: bool,
    pub children_offset: usize,
    /// Range into the (reordered) face array; only meaningful for leaves.
    pub faces: std::ops::Range<usize>,
}

/// `partition_material_indices`: faces sharing the first face's material first; returns the split index.
pub fn partition_material_indices(material_indices: &[i32], faces: &mut [i32]) -> usize {
    let first = material_indices[faces[0] as usize];
    let mut split = 0;
    for i in 0..faces.len() {
        if material_indices[faces[i] as usize] == first {
            faces.swap(split, i);
            split += 1;
        }
    }
    split
}

/// `leaf_needs_material_split`.
pub fn leaf_needs_material_split(faces: &[i32], material_indices: &[i32]) -> bool {
    if material_indices.is_empty() || faces.is_empty() {
        return false;
    }
    let first = material_indices[faces[0] as usize];
    faces.iter().any(|&f| material_indices[f as usize] != first)
}

/// `build_nodes_recursive_mesh`. `nodes` must already hold the node at `node_index` (the root for the first call).
/// `base` is the offset of `faces` inside the full face array (Blender stores a span; we store ranges).
#[allow(clippy::too_many_arguments)]
pub fn build_nodes_recursive_mesh(
    material_indices: &[i32],
    leaf_limit: usize,
    node_index: usize,
    parent_index: i32,
    bounds_precalc: Option<(V3, V3)>,
    face_centers: &[V3],
    depth: usize,
    faces: &mut [i32],
    base: usize,
    nodes: &mut Vec<MeshNode>,
) {
    nodes[node_index].parent = parent_index;
    let below_leaf_limit = faces.len() <= leaf_limit || depth >= STACK_FIXED_DEPTH - 1;
    if below_leaf_limit && !leaf_needs_material_split(faces, material_indices) {
        nodes[node_index].leaf = true;
        nodes[node_index].faces = base..base + faces.len();
        return;
    }
    let children = nodes.len();
    nodes[node_index].children_offset = children;
    nodes.resize(children + 2, MeshNode::default());

    let split = if !below_leaf_limit {
        let (lo, hi) = bounds_precalc
            .or_else(|| center_bounds(face_centers, faces))
            .unwrap_or(([0.0; 3], [0.0; 3]));
        let axis = dominant_axis([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]);
        let middle = (lo[axis] + hi[axis]) * 0.5;
        partition_along_axis(face_centers, faces, axis, middle)
    } else {
        partition_material_indices(material_indices, faces)
    };

    let (front, back) = faces.split_at_mut(split);
    build_nodes_recursive_mesh(material_indices, leaf_limit, children, node_index as i32, None, face_centers, depth + 1, front, base, nodes);
    build_nodes_recursive_mesh(material_indices, leaf_limit, children + 1, node_index as i32, None, face_centers, depth + 1, back, base + split, nodes);
}

/// Root call as in `Tree::from_mesh`: one default root node, parent -1, depth 0.
pub fn build_mesh_nodes(material_indices: &[i32], leaf_limit: usize, face_centers: &[V3], faces: &mut [i32]) -> Vec<MeshNode> {
    let mut nodes = vec![MeshNode::default()];
    if faces.is_empty() {
        return nodes;
    }
    build_nodes_recursive_mesh(material_indices, leaf_limit, 0, -1, None, face_centers, 0, faces, 0, &mut nodes);
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize) -> Vec<V3> {
        (0..n).map(|i| [i as f32, 0.0, 0.0]).collect()
    }

    #[test]
    fn small_input_is_single_leaf() {
        let c = line(3);
        let mut f = [0, 1, 2];
        let nodes = build_mesh_nodes(&[], 4, &c, &mut f);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].leaf && nodes[0].parent == -1 && nodes[0].faces == (0..3));
    }

    #[test]
    fn spatial_split_until_leaf_limit_covers_all_faces() {
        let c = line(8);
        let mut f: Vec<i32> = (0..8).collect();
        let nodes = build_mesh_nodes(&[], 2, &c, &mut f);
        assert_eq!(nodes.len(), 7);
        let mut covered = vec![0; 8];
        for n in nodes.iter().filter(|n| n.leaf) {
            assert!(n.faces.len() <= 2);
            for i in n.faces.clone() {
                covered[f[i] as usize] += 1;
            }
        }
        assert!(covered.iter().all(|&c| c == 1));
        assert_eq!(nodes[1].parent, 0);
        assert_eq!(nodes[0].children_offset, 1);
    }

    #[test]
    fn material_split_forces_children_below_leaf_limit() {
        let c = line(4);
        let mut f = [0, 1, 2, 3];
        let mats = [0, 1, 0, 1];
        let nodes = build_mesh_nodes(&mats, 8, &c, &mut f);
        assert_eq!(nodes.len(), 3);
        assert!(!nodes[0].leaf && nodes[1].leaf && nodes[2].leaf);
        assert!(!leaf_needs_material_split(&f[nodes[1].faces.clone()], &mats));
        assert!(!leaf_needs_material_split(&f[nodes[2].faces.clone()], &mats));
    }

    #[test]
    fn partition_material_puts_first_material_first() {
        let mats = [5, 7, 5];
        let mut f = [0, 1, 2];
        assert_eq!(partition_material_indices(&mats, &mut f), 2);
        assert_eq!(mats[f[0] as usize], 5);
        assert_eq!(mats[f[2] as usize], 7);
    }
}
