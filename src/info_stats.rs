//! Statistics overlay numbers (`space_info/info_stats.cc`: `stats_mesheval`, `format_stats`, `stats_row`,
//! `ED_info_draw_stats`) for object mode. Counts of a mesh are cached per object, so a million polygon mesh is not
//! walked on every redraw.

use crate::mesh::Mesh;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Vertices, unique edges, faces and triangles of one mesh (`stats_mesheval`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub verts: usize,
    pub edges: usize,
    pub faces: usize,
    pub tris: usize,
}

/// Counts `mesh`; the unique edges are the distinct vertex pairs of the face boundaries.
pub fn count_mesh(mesh: &Mesh) -> Counts {
    let mut keys: Vec<u64> = Vec::with_capacity(mesh.faces.iter().map(Vec::len).sum());
    let mut tris = 0;
    for f in &mesh.faces {
        tris += f.len().saturating_sub(2);
        for (k, &a) in f.iter().enumerate() {
            let b = f[(k + 1) % f.len()];
            if a != b {
                keys.push(((a.min(b) as u64) << 32) | a.max(b) as u64);
            }
        }
    }
    keys.sort_unstable();
    keys.dedup();
    Counts { verts: mesh.verts.len(), edges: keys.len(), faces: mesh.faces.len(), tris }
}

type Cache = Mutex<HashMap<u32, ((usize, usize, usize), Counts)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// `count_mesh()` of object `id`'s mesh, recomputed only when its vertex, face or corner count changed.
pub fn counts_cached(id: u32, mesh: &Mesh) -> Counts {
    let key = (mesh.verts.len(), mesh.faces.len(), mesh.faces.iter().map(Vec::len).sum());
    if let Some(&(k, c)) = cache().lock().unwrap().get(&id) {
        if k == key {
            return c;
        }
    }
    let c = count_mesh(mesh);
    cache().lock().unwrap().insert(id, (key, c));
    c
}

/// `BLI_str_format_int_grouped`: `1234567` is `1,234,567`.
pub fn group_int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One `stats_row()` as `label<TAB>value`; the value is `sel / total` when `total` is given. The overlay draws the
/// labels and the values in two columns.
fn row(label: &str, value: usize, total: Option<usize>) -> String {
    match total {
        Some(t) => format!("{label}\t{} / {}", group_int(value), group_int(t)),
        None => format!("{label}\t{}", group_int(value)),
    }
}

/// Selected and total counts over the visible objects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneStats {
    pub objects: usize,
    pub objects_sel: usize,
    pub all: Counts,
    pub sel: Counts,
}

impl SceneStats {
    /// The rows of `ED_info_draw_stats()` in object mode. `active_mesh` is "the active object is a mesh".
    pub fn rows(&self, active_mesh: bool) -> Vec<String> {
        let (a, s) = (self.all, self.sel);
        if self.objects == 0 {
            return vec![row("Objects", 0, None)];
        }
        if self.objects_sel == 0 {
            /* Scene totals if nothing is selected. */
            return vec![row("Objects", self.objects, None), row("Vertices", a.verts, None), row("Edges", a.edges, None), row("Faces", a.faces, None), row("Triangles", a.tris, None)];
        }
        let mut rows = vec![row("Objects", self.objects_sel, Some(self.objects))];
        if active_mesh {
            rows.push(row("Vertices", s.verts, Some(a.verts)));
            rows.push(row("Edges", s.edges, Some(a.edges)));
            rows.push(row("Faces", s.faces, Some(a.faces)));
            rows.push(row("Triangles", s.tris, Some(a.tris)));
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_counts() {
        let c = count_mesh(&Mesh::cube(2.0));
        assert_eq!(c, Counts { verts: 8, edges: 12, faces: 6, tris: 12 });
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_int(0), "0");
        assert_eq!(group_int(999), "999");
        assert_eq!(group_int(1000), "1,000");
        assert_eq!(group_int(1_234_567), "1,234,567");
    }

    #[test]
    fn rows_follow_the_selection() {
        let c = Counts { verts: 8, edges: 12, faces: 6, tris: 12 };
        let st = SceneStats { objects: 3, objects_sel: 1, all: c, sel: c };
        assert_eq!(st.rows(true), ["Objects\t1 / 3", "Vertices\t8 / 8", "Edges\t12 / 12", "Faces\t6 / 6", "Triangles\t12 / 12"]);
        assert_eq!(st.rows(false), ["Objects\t1 / 3"]);
        let none = SceneStats { objects_sel: 0, ..st };
        assert_eq!(none.rows(true), ["Objects\t3", "Vertices\t8", "Edges\t12", "Faces\t6", "Triangles\t12"]);
        assert_eq!(SceneStats::default().rows(true), ["Objects\t0"]);
    }
}
