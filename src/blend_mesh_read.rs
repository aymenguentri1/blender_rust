//! Read a `Mesh` ID block of a `.blend` into the repo `Mesh`.
//!
//! Members are looked up by their on-disk names (`dna_rename_defs.h`: `totvert` = `verts_num`,
//! `totpoly` = `faces_num`, `totloop` = `corners_num`, `poly_offset_indices` = `face_offset_indices`,
//! `vdata` = `vert_data`, `ldata` = `corner_data`).
//! Layout from `DNA_mesh_types.h` (`verts_num`, `faces_num`, `corners_num`, `face_offset_indices`,
//! `vert_data`, `corner_data`) and `DNA_customdata_types.h` (`CustomData::layers/totlayer`,
//! `CustomDataLayer::type/name/data`). Geometry lives in the layers `"position"` (`CD_PROP_FLOAT3` = 48,
//! vert_data) and `".corner_vert"` (`CD_PROP_INT32` = 11, corner_data), see `mesh.cc:547`. Faces are
//! the `face_offset_indices` ranges over the corner verts (`Mesh::faces()` / `OffsetIndices`).

use crate::blend_dna_read::BlockReader;
use crate::blend_sdna::Sdna;
use crate::mesh::Mesh;

pub const CD_PROP_INT32: i32 = 11;
pub const CD_PROP_FLOAT3: i32 = 48;
/// `bke::AttrType::Int32` / `Float3` (BKE_attribute_enums.hh), the types used by `AttributeStorage`.
pub const ATTR_INT32: i16 = 3;
pub const ATTR_FLOAT3: i16 = 7;
const STORAGE_ARRAY: u8 = 0;

/// Resolves an old (file) address to the payload of the block stored there.
pub trait BlockSource {
    fn block(&self, ptr: u64) -> Option<&[u8]>;
}

type R<T> = Result<T, &'static str>;

fn off(sdna: &Sdna, st: &str, ty: &str, name: &str) -> R<usize> {
    sdna.member_offset(st, ty, name).ok_or("missing DNA member")
}

/// Find the data pointer of layer `(cd_type, name)` in the CustomData at `cd_off` of the Mesh.
fn layer_data(
    sdna: &Sdna,
    src: &dyn BlockSource,
    me: BlockReader,
    cd_off: usize,
    cd_type: i32,
    name: &str,
) -> R<u64> {
    let layers_ptr = me.ptr(cd_off + off(sdna, "CustomData", "CustomDataLayer", "*layers")?).ok_or("short mesh")?;
    let tot = me.i32(cd_off + off(sdna, "CustomData", "int", "totlayer")?).ok_or("short mesh")?;
    if tot <= 0 || layers_ptr == 0 {
        return Err("layer not found");
    }
    let stride = sdna.struct_size(sdna.struct_find("CustomDataLayer").ok_or("no CustomDataLayer")?) as usize;
    let arr = src.block(layers_ptr).ok_or("unresolved layers pointer")?;
    let rd = BlockReader { data: arr, ..me };
    let (o_type, o_name, o_data) = (
        off(sdna, "CustomDataLayer", "int", "type")?,
        off(sdna, "CustomDataLayer", "char", "name[68]")?,
        off(sdna, "CustomDataLayer", "void", "*data")?,
    );
    for i in 0..tot as usize {
        let base = i * stride;
        if rd.i32(base + o_type) == Some(cd_type) && rd.cstr(base + o_name, 68).as_deref() == Some(name) {
            return rd.ptr(base + o_data).ok_or("short layer");
        }
    }
    Err("layer not found")
}

/// Array of attribute `(attr_type, name)` in `Mesh::attribute_storage`, where Blender 5 files keep the
/// geometry (`AttributeStorage::blend_read`, attribute_storage.cc). `None` when it is not stored there.
fn storage_data(sdna: &Sdna, src: &dyn BlockSource, me: BlockReader, attr_type: i16, name: &str) -> R<Option<u64>> {
    let Some(storage) = sdna.member_offset("Mesh", "AttributeStorage", "attribute_storage") else { return Ok(None) };
    let list = me.ptr(storage + off(sdna, "AttributeStorage", "Attribute", "*dna_attributes")?).ok_or("short mesh")?;
    let num = me.i32(storage + off(sdna, "AttributeStorage", "int", "dna_attributes_num")?).ok_or("short mesh")?;
    if num <= 0 || list == 0 {
        return Ok(None);
    }
    let stride = sdna.struct_size(sdna.struct_find("Attribute").ok_or("no Attribute")?) as usize;
    let rd = BlockReader { data: src.block(list).ok_or("unresolved attributes pointer")?, ..me };
    let (o_name, o_type, o_storage, o_data) = (
        off(sdna, "Attribute", "char", "*name")?,
        off(sdna, "Attribute", "short", "data_type")?,
        off(sdna, "Attribute", "int8_t", "storage_type")?,
        off(sdna, "Attribute", "void", "*data")?,
    );
    for i in 0..num as usize {
        let base = i * stride;
        let name_ptr = rd.ptr(base + o_name).ok_or("short attribute")?;
        let stored = src.block(name_ptr).and_then(|d| d.split(|&c| c == 0).next());
        if stored != Some(name.as_bytes()) || rd.i16(base + o_type) != Some(attr_type) {
            continue;
        }
        if rd.data.get(base + o_storage) != Some(&STORAGE_ARRAY) {
            return Err("unsupported attribute storage type");
        }
        let array = rd.ptr(base + o_data).ok_or("short attribute")?;
        let array = BlockReader { data: src.block(array).ok_or("unresolved attribute array")?, ..me };
        return array.ptr(off(sdna, "AttributeArray", "void", "*data")?).ok_or("short attribute").map(Some);
    }
    Ok(None)
}

/// Read one `Mesh` block payload (`me`) into a `Mesh`.
pub fn read_mesh(sdna: &Sdna, src: &dyn BlockSource, me: BlockReader) -> R<Mesh> {
    let get = |name: &str| me.i32(off(sdna, "Mesh", "int", name)?).ok_or("short mesh");
    let (verts_num, faces_num, corners_num) = (get("totvert")?, get("totpoly")?, get("totloop")?);
    if verts_num < 0 || faces_num < 0 || corners_num < 0 {
        return Err("negative element count");
    }
    let (nv, nf, nc) = (verts_num as usize, faces_num as usize, corners_num as usize);

    let vert_off = off(sdna, "Mesh", "CustomData", "vdata")?;
    let corner_off = off(sdna, "Mesh", "CustomData", "ldata")?;

    let mut mesh = Mesh::default();
    if nv > 0 {
        let p = match storage_data(sdna, src, me, ATTR_FLOAT3, "position")? {
            Some(p) => p,
            None => layer_data(sdna, src, me, vert_off, CD_PROP_FLOAT3, "position")?,
        };
        let rd = BlockReader { data: src.block(p).ok_or("unresolved position data")?, ..me };
        for i in 0..nv {
            mesh.verts.push(rd.f32s::<3>(i * 12).ok_or("short position data")?);
        }
    }
    if nf > 0 {
        let offs_ptr = me.ptr(off(sdna, "Mesh", "int", "*poly_offset_indices")?).ok_or("short mesh")?;
        let offs = BlockReader { data: src.block(offs_ptr).ok_or("unresolved face offsets")?, ..me };
        let p = match storage_data(sdna, src, me, ATTR_INT32, ".corner_vert")? {
            Some(p) => p,
            None => layer_data(sdna, src, me, corner_off, CD_PROP_INT32, ".corner_vert")?,
        };
        let cv = BlockReader { data: src.block(p).ok_or("unresolved corner data")?, ..me };
        let mut start = offs.i32(0).ok_or("short face offsets")?;
        if start != 0 {
            return Err("face offsets must start at 0");
        }
        for f in 0..nf {
            let end = offs.i32((f + 1) * 4).ok_or("short face offsets")?;
            if end < start || end as usize > nc {
                return Err("invalid face offsets");
            }
            let mut face = Vec::with_capacity((end - start) as usize);
            for c in start..end {
                let v = cv.i32(c as usize * 4).ok_or("short corner data")?;
                if v < 0 || v as usize >= nv {
                    return Err("corner vertex out of range");
                }
                face.push(v as u32);
            }
            mesh.faces.push(face);
            start = end;
        }
        if start as usize != nc {
            return Err("face offsets do not cover all corners");
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_sdna::{Struct, StructMember};
    use std::collections::HashMap;

    struct Map(HashMap<u64, Vec<u8>>);
    impl BlockSource for Map {
        fn block(&self, ptr: u64) -> Option<&[u8]> {
            self.0.get(&ptr).map(|v| v.as_slice())
        }
    }

    fn sdna() -> Sdna {
        let types = ["int", "char", "void", "CustomDataLayer", "CustomData", "Mesh"];
        let members = [
            "type", "name[68]", "*data", "_pad[4]", "*layers", "totlayer", "totvert", "totpoly",
            "totloop", "*poly_offset_indices", "vdata", "ldata",
        ];
        let st = |ty: i16, ms: &[(i16, i16)]| Struct {
            type_index: ty,
            members: ms.iter().map(|&(t, m)| StructMember { type_index: t, member_index: m }).collect(),
        };
        Sdna {
            members: members.iter().map(|s| s.to_string()).collect(),
            types: types.iter().map(|s| s.to_string()).collect(),
            types_size: vec![4, 1, 0, 80, 16, 48],
            structs: vec![
                st(3, &[(0, 0), (1, 1), (2, 2)]),
                st(4, &[(3, 4), (0, 5)]),
                st(5, &[(0, 6), (0, 7), (0, 8), (1, 3), (0, 9), (4, 10), (4, 11)]),
            ],
            pointer_size: 8,
            members_array_num: vec![1, 68, 1, 4, 1, 1, 1, 1, 1, 1, 1, 1],
        }
    }

    fn le(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn layer(ty: i32, name: &str, data: u64) -> Vec<u8> {
        let mut l = (ty as u32).to_le_bytes().to_vec();
        let mut n = name.as_bytes().to_vec();
        n.resize(68, 0);
        l.extend(n);
        l.extend(data.to_le_bytes());
        l
    }

    /// Mesh payload: ints 0..12 (+pad 4), face_offset_indices @16, vert_data @24, corner_data @40.
    fn build(nv: u32, nf: u32, nc: u32, offs: u64, vptr: u64, cptr: u64) -> Vec<u8> {
        let mut m = le(&[nv, nf, nc, 0]);
        m.extend(offs.to_le_bytes());
        m.extend(vptr.to_le_bytes());
        m.extend(le(&[1, 0]));
        m.extend(cptr.to_le_bytes());
        m.extend(le(&[1, 0]));
        m
    }

    fn quad_file() -> (Map, Vec<u8>) {
        let pos: Vec<u8> = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .flatten()
            .flat_map(|f| f.to_bits().to_le_bytes())
            .collect();
        let mut map = HashMap::new();
        map.insert(0x100, layer(CD_PROP_FLOAT3, "position", 0x110));
        map.insert(0x110, pos);
        map.insert(0x200, layer(CD_PROP_INT32, ".corner_vert", 0x210));
        map.insert(0x210, le(&[0, 1, 2, 3]));
        map.insert(0x300, le(&[0, 4]));
        (Map(map), build(4, 1, 4, 0x300, 0x100, 0x200))
    }

    #[test]
    fn reads_a_quad() {
        let (map, me) = quad_file();
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        let mesh = read_mesh(&sdna(), &map, rd).unwrap();
        assert_eq!(mesh.verts, vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]);
        assert_eq!(mesh.faces, vec![vec![0, 1, 2, 3]]);
    }

    #[test]
    fn reads_two_faces_of_different_sizes() {
        let (mut map, _) = quad_file();
        map.0.insert(0x210, le(&[0, 1, 2, 0, 2, 3]));
        map.0.insert(0x300, le(&[0, 3, 6]));
        let me = build(4, 2, 6, 0x300, 0x100, 0x200);
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        let mesh = read_mesh(&sdna(), &map, rd).unwrap();
        assert_eq!(mesh.faces, vec![vec![0, 1, 2], vec![0, 2, 3]]);
    }

    /// `sdna()` plus Attribute (24 bytes), AttributeArray, and `Mesh::attribute_storage` @56.
    fn sdna_with_storage() -> Sdna {
        let mut s = sdna();
        let ty = s.types.len() as i16;
        for (name, size) in [("short", 2), ("int8_t", 1), ("Attribute", 24), ("AttributeArray", 8), ("AttributeStorage", 16)] {
            s.types.push(name.to_string());
            s.types_size.push(size);
        }
        let m = s.members.len() as i16;
        for (name, num) in [("*name", 1), ("data_type", 1), ("domain", 1), ("storage_type", 1), ("_pad[4]", 4),
            ("*data", 1), ("*dna_attributes", 1), ("dna_attributes_num", 1), ("_pad[4]", 4), ("attribute_storage", 1)] {
            s.members.push(name.to_string());
            s.members_array_num.push(num);
        }
        let st = |t: i16, ms: &[(i16, i16)]| Struct {
            type_index: t,
            members: ms.iter().map(|&(ty, i)| StructMember { type_index: ty, member_index: m + i }).collect(),
        };
        let (short, int8, attr, array, storage) = (ty, ty + 1, ty + 2, ty + 3, ty + 4);
        s.structs.push(st(attr, &[(1, 0), (short, 1), (int8, 2), (int8, 3), (1, 4), (2, 5)]));
        s.structs.push(st(array, &[(2, 5)]));
        s.structs.push(st(storage, &[(attr, 6), (0, 7), (1, 8)]));
        s.structs[2].members.push(StructMember { type_index: storage, member_index: m + 9 });
        s
    }

    fn attribute(name: u64, ty: i16, array: u64) -> Vec<u8> {
        let mut a = name.to_le_bytes().to_vec();
        a.extend(ty.to_le_bytes());
        a.extend([0; 6]);
        a.extend(array.to_le_bytes());
        a
    }

    #[test]
    fn reads_geometry_from_attribute_storage() {
        let (mut map, _) = quad_file();
        let mut list = attribute(0x400, ATTR_FLOAT3, 0x500);
        list.extend(attribute(0x410, ATTR_INT32, 0x510));
        map.0.insert(0x420, list);
        map.0.insert(0x400, b"position\0".to_vec());
        map.0.insert(0x410, b".corner_vert\0".to_vec());
        map.0.insert(0x500, 0x110u64.to_le_bytes().to_vec());
        map.0.insert(0x510, 0x210u64.to_le_bytes().to_vec());
        let mut me = build(4, 1, 4, 0x300, 0, 0);
        me.extend(0x420u64.to_le_bytes());
        me.extend(le(&[2, 0]));
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        let mesh = read_mesh(&sdna_with_storage(), &map, rd).unwrap();
        assert_eq!((mesh.verts.len(), mesh.faces), (4, vec![vec![0, 1, 2, 3]]));
    }

    #[test]
    fn missing_layer_is_an_error() {
        let (mut map, me) = quad_file();
        map.0.insert(0x200, layer(CD_PROP_INT32, ".corner_edge", 0x210));
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        assert_eq!(read_mesh(&sdna(), &map, rd), Err("layer not found"));
    }

    #[test]
    fn bad_indices_and_offsets_are_errors() {
        let (mut map, me) = quad_file();
        map.0.insert(0x210, le(&[0, 1, 2, 9]));
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        assert_eq!(read_mesh(&sdna(), &map, rd), Err("corner vertex out of range"));
        map.0.insert(0x210, le(&[0, 1, 2, 3]));
        map.0.insert(0x300, le(&[0, 9]));
        assert_eq!(read_mesh(&sdna(), &map, rd), Err("invalid face offsets"));
        map.0.insert(0x300, le(&[0, 3]));
        assert_eq!(read_mesh(&sdna(), &map, rd), Err("face offsets do not cover all corners"));
    }

    #[test]
    fn empty_mesh_reads_without_layers() {
        let map = Map(HashMap::new());
        let me = build(0, 0, 0, 0, 0, 0);
        let rd = BlockReader { data: &me, big_endian: false, pointer_size: 8 };
        assert_eq!(read_mesh(&sdna(), &map, rd), Ok(Mesh::default()));
    }
}
