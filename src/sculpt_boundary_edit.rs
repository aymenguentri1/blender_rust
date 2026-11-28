//! Sculpt boundary edit info and falloff (`mesh/sculpt_boundary.cc`: `edit_data_init_mesh`, `calc_boundary_falloff`,
//! `init_falloff_mesh`; `BRUSH_BOUNDARY_FALLOFF_*` in DNA_brush_enums.h).
//! Deviation: base mesh only; the brush curve (`BKE_brush_curve_strength`) is passed in as a closure `curve(value, max)`.

use std::collections::VecDeque;

use crate::mesh::{sub, V3};
use crate::sculpt_boundary::{Boundary, Topo};

/// `BOUNDARY_VERTEX_NONE` / `BOUNDARY_STEPS_NONE`.
pub const NONE: i32 = -1;

/// `SculptBoundary::edit_info` plus `max_propagation_steps` and `pivot_position`.
#[derive(Debug, Default)]
pub struct EditInfo {
    pub original_vertex_i: Vec<i32>,
    pub propagation_steps_num: Vec<i32>,
    pub strength_factor: Vec<f32>,
    pub max_propagation_steps: i32,
    pub pivot_position: V3,
}

fn dist(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// `edit_data_init_mesh()`: breadth first propagation from every boundary vertex until the distance walked from the
/// initial vertex exceeds `radius` or the mesh is exhausted.
pub fn edit_data_init(topo: &Topo, boundary: &Boundary, initial_vert_i: usize, radius: f32) -> EditInfo {
    let n = topo.mesh.verts.len();
    let pos = &topo.mesh.verts;
    let mut e = EditInfo {
        original_vertex_i: vec![NONE; n],
        propagation_steps_num: vec![NONE; n],
        strength_factor: vec![0.0; n],
        ..Default::default()
    };
    let mut current: VecDeque<usize> = VecDeque::new();
    for &v in &boundary.verts {
        e.original_vertex_i[v] = v as i32;
        e.propagation_steps_num[v] = 0;
        current.push_back(v);
    }
    let (mut steps, mut accum) = (0, 0.0f32);
    loop {
        if accum > radius || current.is_empty() {
            e.max_propagation_steps = steps;
            break;
        }
        let mut next = VecDeque::new();
        while let Some(from) = current.pop_front() {
            for nb in topo.neighbors(from) {
                if topo.hide_vert[from] || e.propagation_steps_num[nb] != NONE {
                    continue;
                }
                e.original_vertex_i[nb] = e.original_vertex_i[from];
                e.propagation_steps_num[nb] = e.propagation_steps_num[from] + 1;
                next.push_back(nb);
                if e.original_vertex_i[from] == initial_vert_i as i32 {
                    e.pivot_position = pos[nb];
                    accum += dist(pos[from], pos[nb]);
                }
            }
        }
        current = next;
        steps += 1;
    }
    e
}

/// `eBrushBoundaryFalloffType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FalloffType {
    Constant,
    Radius,
    Loop,
    LoopInvert,
}

/// `calc_boundary_falloff()`: `(falloff_distance, direction)` for a boundary distance.
pub fn calc_boundary_falloff(kind: FalloffType, boundary_distance: f32, radius: f32) -> (f32, f32) {
    let looped = || {
        let div = (boundary_distance / radius) as i32;
        let m = boundary_distance % radius;
        (div, if div % 2 == 0 { m } else { radius - m })
    };
    match kind {
        FalloffType::Radius => (boundary_distance, 1.0),
        FalloffType::Loop => (looped().1, 1.0),
        FalloffType::LoopInvert => {
            let (div, d) = looped();
            (d, if ((div - 1) & 2) == 0 { -1.0 } else { 1.0 })
        }
        FalloffType::Constant => (0.0, 1.0),
    }
}

/// `init_falloff_mesh()`: fill `strength_factor` from the propagation steps, the mask and (unless constant) the
/// boundary distance of the originating boundary vertex.
pub fn init_falloff(
    e: &mut EditInfo,
    boundary: &Boundary,
    initial_vert_i: usize,
    mask: &[f32],
    kind: FalloffType,
    radius: f32,
    curve: impl Fn(f32, f32) -> f32,
) {
    for i in 0..e.strength_factor.len() {
        if e.propagation_steps_num[i] != NONE {
            let mask_factor = if mask.is_empty() { 1.0 } else { 1.0 - mask[i] };
            e.strength_factor[i] = mask_factor * curve(e.propagation_steps_num[i] as f32, e.max_propagation_steps as f32);
        }
        if e.original_vertex_i[i] == initial_vert_i as i32 || kind == FalloffType::Constant {
            continue;
        }
        let orig = e.original_vertex_i[i];
        if orig < 0 {
            continue;
        }
        let bd = boundary.distance.get(&(orig as usize)).copied().unwrap_or(0.0);
        let (fd, dir) = calc_boundary_falloff(kind, bd, radius);
        e.strength_factor[i] *= dir * curve(fd, radius);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;
    use crate::sculpt_boundary::indices_init;

    fn setup(radius: f32) -> (Mesh, usize, f32) {
        (Mesh::grid(8, 8, 4.0), 0, radius)
    }

    fn start_vert(t: &Topo) -> usize {
        (0..t.mesh.verts.len()).find(|&i| t.is_boundary(i) && t.is_in_editable_boundary(i)).unwrap()
    }

    #[test]
    fn steps_grow_from_the_boundary_and_stop_at_radius() {
        let (m, _, r) = setup(1.2);
        let t = Topo::new(&m);
        let s = start_vert(&t);
        let b = indices_init(&t, s);
        let e = edit_data_init(&t, &b, s, r);
        for &v in &b.verts {
            assert_eq!(e.propagation_steps_num[v], 0);
            assert_eq!(e.original_vertex_i[v], v as i32);
        }
        assert!(e.max_propagation_steps >= 1);
        let far = (0..m.verts.len()).filter(|&i| e.propagation_steps_num[i] == NONE).count();
        assert!(far > 0, "propagation must stop before covering the whole grid");
        assert!(e.propagation_steps_num.iter().all(|&s| s <= e.max_propagation_steps));
    }

    #[test]
    fn large_radius_covers_the_mesh() {
        let (m, _, _) = setup(100.0);
        let t = Topo::new(&m);
        let s = start_vert(&t);
        let b = indices_init(&t, s);
        let e = edit_data_init(&t, &b, s, 100.0);
        assert!(e.propagation_steps_num.iter().all(|&s| s != NONE));
    }

    #[test]
    fn falloff_types_match_blender() {
        assert_eq!(calc_boundary_falloff(FalloffType::Radius, 3.0, 2.0), (3.0, 1.0));
        assert_eq!(calc_boundary_falloff(FalloffType::Loop, 0.5, 2.0), (0.5, 1.0));
        assert_eq!(calc_boundary_falloff(FalloffType::Loop, 3.0, 2.0), (1.0, 1.0)); // div 1: radius - mod
        assert_eq!(calc_boundary_falloff(FalloffType::LoopInvert, 0.5, 2.0), (0.5, 1.0)); // div 0: (-1 & 2) != 0
        assert_eq!(calc_boundary_falloff(FalloffType::LoopInvert, 3.0, 2.0), (1.0, -1.0)); // div 1: (0 & 2) == 0
        assert_eq!(calc_boundary_falloff(FalloffType::Constant, 3.0, 2.0), (0.0, 1.0));
    }

    #[test]
    fn strength_factor_uses_curve_mask_and_constant_mode() {
        let m = Mesh::grid(8, 8, 4.0);
        let t = Topo::new(&m);
        let s = start_vert(&t);
        let b = indices_init(&t, s);
        let mut e = edit_data_init(&t, &b, s, 100.0);
        let lin = |v: f32, max: f32| 1.0 - v / max.max(1.0);
        init_falloff(&mut e, &b, s, &[], FalloffType::Constant, 1.0, lin);
        for &v in &b.verts {
            assert_eq!(e.strength_factor[v], 1.0);
        }
        let mut mask = vec![0.0; m.verts.len()];
        mask[b.verts[1]] = 1.0;
        init_falloff(&mut e, &b, s, &mask, FalloffType::Constant, 1.0, lin);
        assert_eq!(e.strength_factor[b.verts[1]], 0.0);
        let deepest = (0..m.verts.len()).max_by_key(|&i| e.propagation_steps_num[i]).unwrap();
        // `max_propagation_steps` is one past the deepest step (the loop counts the final empty iteration).
        let expect = 1.0 - e.propagation_steps_num[deepest] as f32 / e.max_propagation_steps as f32;
        assert!((e.strength_factor[deepest] - expect).abs() < 1e-6);
    }
}
