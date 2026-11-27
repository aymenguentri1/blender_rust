//! Topology Slide brush (`mesh/brushes/topology_slide.cc`: `calc_translation_directions`,
//! `add_neighbor_influence`, `calc_neighbor_influence`, `do_topology_slide_brush`). A vertex slides along the edges
//! to its neighbours that point the same way as the wanted direction, by the length of the direction's projection on
//! each. Deviation: only the surface distance falloff is used (no cube tip), and the factors use the current positions.

use crate::mesh::{add, dot, normalized, scale, sub, V3};

/// `BRUSH_SLIDE_DEFORM_*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideType {
    /// Along the stroke.
    Drag,
    /// Towards the brush centre.
    Pinch,
    /// Away from the brush centre.
    Expand,
}

/// `calc_translation_directions()` for one vertex. `stroke_dir` is `location - last_location` (normalized).
pub fn direction(kind: SlideType, position: V3, center: V3, stroke_dir: V3) -> V3 {
    match kind {
        SlideType::Drag => normalized(stroke_dir),
        SlideType::Pinch => normalized(sub(center, position)),
        SlideType::Expand => normalized(sub(position, center)),
    }
}

/// `add_neighbor_influence()`: the part of the edge to `neighbor` that lies along `dir`, kept on the edge.
fn neighbor_influence(position: V3, dir: V3, neighbor: V3) -> V3 {
    let disp = sub(neighbor, position);
    let ndir = normalized(disp);
    if dot(dir, ndir) > 0.0 {
        scale(ndir, dot(dir, disp))
    } else {
        [0.0; 3]
    }
}

/// `calc_neighbor_influence()` for one vertex.
pub fn translation(position: V3, dir: V3, neighbors: impl Iterator<Item = V3>) -> V3 {
    neighbors.fold([0.0; 3], |acc, n| add(acc, neighbor_influence(position, dir, n)))
}

/// One dab: the `(vertex, factor)` pairs slide along their edges. `adj` are the vertex neighbours.
pub fn dab(verts: &mut [V3], adj: &[Vec<u32>], hit: &[(usize, f32)], kind: SlideType, center: V3, stroke_dir: V3, strength: f32) {
    if stroke_dir == [0.0; 3] && kind == SlideType::Drag {
        return;
    }
    let moves: Vec<(usize, V3)> = hit
        .iter()
        .map(|&(i, f)| {
            let dir = direction(kind, verts[i], center, stroke_dir);
            let t = translation(verts[i], dir, adj[i].iter().map(|&n| verts[n as usize]));
            (i, scale(t, f * strength))
        })
        .collect();
    for (i, t) in moves {
        verts[i] = add(verts[i], t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* A vertex at the origin with neighbours on +x, -x and +y. */
    fn star() -> (Vec<V3>, Vec<Vec<u32>>) {
        (vec![[0.0; 3], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 2.0, 0.0]], vec![vec![1, 2, 3], vec![0], vec![0], vec![0]])
    }

    #[test]
    fn drag_slides_along_the_edge_that_follows_the_stroke() {
        let (mut v, adj) = star();
        dab(&mut v, &adj, &[(0, 1.0)], SlideType::Drag, [0.0; 3], [1.0, 0.0, 0.0], 1.0);
        assert_eq!(v[0], [1.0, 0.0, 0.0]); /* the +x edge only, by dot(dir, disp) = 1 */
    }

    #[test]
    fn diagonal_stroke_sums_the_edges_in_front() {
        let (mut v, adj) = star();
        let d = normalized([1.0, 1.0, 0.0]);
        dab(&mut v, &adj, &[(0, 1.0)], SlideType::Drag, [0.0; 3], d, 0.5);
        let s = 0.5f32.sqrt();
        /* +x edge: x * s, +y edge: y * 2s; half strength. */
        assert!((v[0][0] - 0.5 * s).abs() < 1e-6 && (v[0][1] - 0.5 * 2.0 * s).abs() < 1e-6);
    }

    #[test]
    fn pinch_and_expand_follow_the_centre() {
        let (mut v, adj) = star();
        /* The vertex is left of the centre: pinch moves it towards +x. */
        v[0] = [0.0; 3];
        dab(&mut v, &adj, &[(0, 1.0)], SlideType::Pinch, [3.0, 0.0, 0.0], [0.0; 3], 1.0);
        assert!(v[0][0] > 0.0 && v[0][1].abs() < 1e-6);
        let (mut w, adj) = star();
        dab(&mut w, &adj, &[(0, 1.0)], SlideType::Expand, [3.0, 0.0, 0.0], [0.0; 3], 1.0);
        assert!(w[0][0] < 0.0);
    }

    #[test]
    fn nothing_moves_against_all_edges_or_without_a_stroke() {
        let (mut v, adj) = star();
        dab(&mut v, &adj, &[(0, 1.0)], SlideType::Drag, [0.0; 3], [0.0, 0.0, 1.0], 1.0);
        assert_eq!(v[0], [0.0; 3]);
        dab(&mut v, &adj, &[(0, 1.0)], SlideType::Drag, [0.0; 3], [0.0; 3], 1.0);
        assert_eq!(v[0], [0.0; 3]);
    }
}
