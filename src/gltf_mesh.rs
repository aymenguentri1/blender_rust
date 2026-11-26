//! Binary glTF (.glb) writer for one mesh: POSITION, NORMAL and triangle indices.
//!
//! Modelled on the glTF-Blender-IO exporter:
//!  - `export.py save_gltf()` (io/exp/export.py:84-112): the GLB container, header `glTF` + version 2 + total length,
//!    a `JSON` chunk padded with spaces to 4 bytes, a `BIN\0` chunk padded with zeros to 4 bytes (omitted when empty)
//!  - `primitives.py __gather_indices()` (blender/exp/primitives.py:236-253): indices are `UnsignedShort` while the
//!    largest index is below 65535, else `UnsignedInt`
//!  - `BinaryData` / `Buffer.add_and_get_view()` (io/exp/binary_data.py, buffer.py:63): every view starts at a
//!    multiple of 4; vertex data uses target `ARRAY_BUFFER` (34962), indices `ELEMENT_ARRAY_BUFFER` (34963)
//!  - `constants.py`: component types 5123 (u16), 5125 (u32), 5126 (f32)
//!  - axes: Blender Z up -> glTF Y up, `(x, y, z) -> (x, z, -y)` (`gltf_node.rs`)
//! Not ported: materials, UVs, tangents, vertex colours, sparse accessors, meshopt and draco compression.

use crate::mesh::{Mesh, V3};

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const FLOAT: u32 = 5126;

/// Blender (Z up) to glTF (Y up).
pub fn swizzle(v: V3) -> V3 {
    [v[0], v[2], -v[1]]
}

fn pad4(buf: &mut Vec<u8>, byte: u8) {
    while buf.len() % 4 != 0 {
        buf.push(byte);
    }
}

fn f32s(buf: &mut Vec<u8>, vs: &[V3]) {
    for v in vs {
        for c in v {
            buf.extend_from_slice(&c.to_le_bytes());
        }
    }
}

fn num(x: f32) -> String {
    let x = if x == 0.0 { 0.0 } else { x };
    format!("{x}")
}

fn vec3(v: V3) -> String {
    format!("[{},{},{}]", num(v[0]), num(v[1]), num(v[2]))
}

/// The `.glb` bytes of `mesh` as one node holding one mesh with one triangle primitive; `name` names node and mesh.
pub fn mesh_to_glb(mesh: &Mesh, name: &str) -> Vec<u8> {
    let positions: Vec<V3> = mesh.verts.iter().map(|&v| swizzle(v)).collect();
    let normals: Vec<V3> = mesh.vertex_normals().into_iter().map(swizzle).collect();
    let indices: Vec<u32> = mesh.triangles().into_iter().flat_map(|(_, t)| t).collect();
    let (pmin, pmax) = positions.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(mn, mx), v| {
        (std::array::from_fn(|k| mn[k].min(v[k])), std::array::from_fn(|k| mx[k].max(v[k])))
    });
    let wide = indices.iter().copied().max().unwrap_or(0) >= 65535;
    let (index_type, index_size) = if wide { (UNSIGNED_INT, 4) } else { (UNSIGNED_SHORT, 2) };

    let mut bin = Vec::new();
    let pos_len = positions.len() * 12;
    f32s(&mut bin, &positions);
    pad4(&mut bin, 0);
    let nor_off = bin.len();
    f32s(&mut bin, &normals);
    pad4(&mut bin, 0);
    let idx_off = bin.len();
    for &i in &indices {
        if wide {
            bin.extend_from_slice(&i.to_le_bytes());
        } else {
            bin.extend_from_slice(&(i as u16).to_le_bytes());
        }
    }
    let idx_len = indices.len() * index_size;
    pad4(&mut bin, 0);

    let name = name.replace('\\', "\\\\").replace('"', "\\\"");
    let json = format!(
        concat!(
            "{{\"asset\":{{\"generator\":\"blender_rust\",\"version\":\"2.0\"}},\"scene\":0,",
            "\"scenes\":[{{\"name\":\"Scene\",\"nodes\":[0]}}],",
            "\"nodes\":[{{\"mesh\":0,\"name\":\"{n}\"}}],",
            "\"meshes\":[{{\"name\":\"{n}\",\"primitives\":[{{\"attributes\":{{\"POSITION\":0,\"NORMAL\":1}},\"indices\":2,\"mode\":4}}]}}],",
            "\"accessors\":[",
            "{{\"bufferView\":0,\"componentType\":{f},\"count\":{vc},\"max\":{mx},\"min\":{mn},\"type\":\"VEC3\"}},",
            "{{\"bufferView\":1,\"componentType\":{f},\"count\":{vc},\"type\":\"VEC3\"}},",
            "{{\"bufferView\":2,\"componentType\":{it},\"count\":{ic},\"type\":\"SCALAR\"}}],",
            "\"bufferViews\":[",
            "{{\"buffer\":0,\"byteLength\":{pl},\"byteOffset\":0,\"target\":{ab}}},",
            "{{\"buffer\":0,\"byteLength\":{pl},\"byteOffset\":{no},\"target\":{ab}}},",
            "{{\"buffer\":0,\"byteLength\":{il},\"byteOffset\":{io},\"target\":{eab}}}],",
            "\"buffers\":[{{\"byteLength\":{bl}}}]}}"
        ),
        n = name,
        f = FLOAT,
        vc = positions.len(),
        mx = vec3(pmax),
        mn = vec3(pmin),
        it = index_type,
        ic = indices.len(),
        pl = pos_len,
        ab = ARRAY_BUFFER,
        no = nor_off,
        il = idx_len,
        io = idx_off,
        eab = ELEMENT_ARRAY_BUFFER,
        bl = bin.len(),
    );

    let mut json = json.into_bytes();
    pad4(&mut json, b' ');
    let mut out = Vec::new();
    let mut total = 12 + 8 + json.len();
    if !bin.is_empty() {
        total += 8 + bin.len();
    }
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    #[test]
    fn swizzle_turns_z_up_into_y_up() {
        assert_eq!(swizzle([1.0, 2.0, 3.0]), [1.0, 3.0, -2.0]);
    }

    #[test]
    fn cube_glb_is_structurally_valid() {
        let cube = Mesh::cube(2.0);
        let glb = mesh_to_glb(&cube, "Cube");
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32_at(&glb, 4), 2);
        assert_eq!(u32_at(&glb, 8) as usize, glb.len(), "header length covers the file");
        assert_eq!(glb.len() % 4, 0);
        let jl = u32_at(&glb, 12) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        assert_eq!(jl % 4, 0);
        let json = std::str::from_utf8(&glb[20..20 + jl]).unwrap();
        assert!(json.contains("\"POSITION\":0") && json.contains("\"NORMAL\":1") && json.contains("\"indices\":2"));
        let bo = 20 + jl;
        assert_eq!(&glb[bo + 4..bo + 8], b"BIN\0");
        let bl = u32_at(&glb, bo) as usize;
        assert_eq!(bo + 8 + bl, glb.len());
        /* 8 vertices (positions + normals) and 12 triangles as u16 indices. */
        let (vc, ic) = (cube.verts.len(), cube.triangles().len() * 3);
        assert_eq!((vc, ic), (8, 36));
        assert!(json.contains(&format!("\"count\":{vc}")) && json.contains(&format!("\"count\":{ic}")));
        assert_eq!(bl, vc * 12 * 2 + ic * 2, "positions + normals + indices, 4-byte aligned");
        assert!(json.contains("\"componentType\":5123"));
        assert!(json.contains("\"max\":[1,1,1]") && json.contains("\"min\":[-1,-1,-1]"));
        assert!(json.contains(&format!("\"byteLength\":{bl}")));
    }

    #[test]
    fn views_start_at_multiples_of_four_and_indices_are_in_range() {
        let glb = mesh_to_glb(&Mesh::uv_sphere(8, 5, 1.0), "S");
        let jl = u32_at(&glb, 12) as usize;
        let json = std::str::from_utf8(&glb[20..20 + jl]).unwrap();
        for part in json.split("\"byteOffset\":").skip(1) {
            let off: usize = part.split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
            assert_eq!(off % 4, 0);
        }
        let bin = &glb[20 + jl + 8..];
        let io: usize = json.split("\"byteOffset\":").last().unwrap().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
        let sphere = Mesh::uv_sphere(8, 5, 1.0);
        let n = sphere.triangles().len() * 3;
        let idx: Vec<u16> = bin[io..io + n * 2].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(idx.len(), n);
        assert!(idx.iter().all(|&i| (i as usize) < sphere.verts.len()));
    }

    #[test]
    fn empty_mesh_has_no_bin_chunk() {
        let glb = mesh_to_glb(&Mesh::default(), "E");
        assert_eq!(u32_at(&glb, 8) as usize, glb.len());
        assert_eq!(glb.len(), 20 + u32_at(&glb, 12) as usize);
    }
}
