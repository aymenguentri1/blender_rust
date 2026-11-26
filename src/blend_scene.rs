//! Assemble the mesh objects of a `.blend`: Object blocks linked to Mesh blocks by `Object::data`.
//!
//! As in `readfile.cc` (`read_data_into_datamap`), the `DATA` blocks that follow an ID block belong to
//! that ID, so their old addresses are only looked up inside it. Compression is detected like
//! `BLI_file_magic_is_gzip()` / `BLI_file_magic_is_zstd()` (fileops_c.cc) and rejected.

use std::collections::HashMap;

use crate::blend_dna_read::BlockReader;
use crate::blend_header::{decode_blender_header, Endian, HeaderVariant};
use crate::blend_mesh_read::{read_mesh, BlockSource};
use crate::blend_object_read::{read_object, OB_MESH, ROT_MODE_EUL};
use crate::blend_reader::{read_blocks, Block, CODE_ID_OB};
use crate::blend_sdna::{parse_sdna, Sdna};
use crate::mesh::Mesh;

const CODE_ID_ME: [u8; 4] = [b'M', b'E', 0, 0];
const CODE_DATA: [u8; 4] = *b"DATA";
const CODE_DNA1: [u8; 4] = *b"DNA1";

#[derive(Clone, Debug, PartialEq)]
pub struct BlendSceneObject {
    pub name: String,
    pub mesh: Mesh,
    pub loc: [f32; 3],
    /// XYZ Euler angles in radians; zero for the other rotation modes (not converted yet).
    pub rot: [f32; 3],
    pub scale: [f32; 3],
}

struct DataMap<'a>(HashMap<u64, &'a [u8]>);

impl BlockSource for DataMap<'_> {
    fn block(&self, ptr: u64) -> Option<&[u8]> {
        self.0.get(&ptr).copied()
    }
}

fn payload<'a>(bytes: &'a [u8], b: &Block) -> Result<&'a [u8], &'static str> {
    bytes.get(b.data..b.data + b.len as usize).ok_or("truncated block data")
}

fn is_compressed(bytes: &[u8]) -> bool {
    let Some(&[a, b, c, d]) = bytes.get(..4) else { return false };
    let magic = u32::from_le_bytes([a, b, c, d]);
    [a, b, c] == [0x1f, 0x8b, 0x08] || magic == 0xFD2F_B528 || (magic >> 4) == 0x184D_2A5
}

/// Mesh objects of the file, in file order. Other object types, and objects whose mesh is not stored
/// in this file (linked libraries), are skipped.
pub fn load_scene(bytes: &[u8]) -> Result<Vec<BlendSceneObject>, &'static str> {
    if is_compressed(bytes) {
        return Err("compressed .blend files are not supported: save without Compress");
    }
    let header = match decode_blender_header(bytes) {
        HeaderVariant::Header(h) => h,
        _ => return Err("not a Blender file"),
    };
    let big = header.endian == Endian::Big;
    let blocks = read_blocks(bytes)?;
    let dna = blocks.iter().find(|b| b.code == CODE_DNA1).ok_or("missing DNA1 block")?;
    let sdna = parse_sdna(payload(bytes, dna)?, big)?;
    assemble(&sdna, bytes, &blocks, big, header.pointer_size as usize)
}

fn assemble(
    sdna: &Sdna,
    bytes: &[u8],
    blocks: &[Block],
    big_endian: bool,
    pointer_size: usize,
) -> Result<Vec<BlendSceneObject>, &'static str> {
    let reader = |data| BlockReader { data, big_endian, pointer_size };
    // Mesh ID old address to its block index.
    let meshes: HashMap<u64, usize> =
        blocks.iter().enumerate().filter(|(_, b)| b.code == CODE_ID_ME).map(|(i, b)| (b.old, i)).collect();
    let mut read: HashMap<u64, Mesh> = HashMap::new();
    let mut out = Vec::new();
    for b in blocks.iter().filter(|b| b.code == CODE_ID_OB) {
        let ob = read_object(sdna, reader(payload(bytes, b)?))?;
        if ob.ob_type != OB_MESH {
            continue;
        }
        let Some(&at) = meshes.get(&ob.data) else { continue };
        let mesh = match read.get(&ob.data) {
            Some(m) => m.clone(),
            None => {
                let mut data = DataMap(HashMap::new());
                for d in blocks[at + 1..].iter().take_while(|d| d.code == CODE_DATA) {
                    data.0.insert(d.old, payload(bytes, d)?);
                }
                let m = read_mesh(sdna, &data, reader(payload(bytes, &blocks[at])?))?;
                read.insert(ob.data, m.clone());
                m
            }
        };
        let rot = if ob.rotmode == ROT_MODE_EUL { ob.rot } else { [0.0; 3] };
        out.push(BlendSceneObject { name: ob.name, mesh, loc: ob.loc, rot, scale: ob.scale });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_mesh_read::CD_PROP_FLOAT3;
    use crate::blend_sdna::{Struct, StructMember};

    /// ID { char name[258] }; Object { ID id; short type, rotmode; float loc[3], rot[3], scale[3];
    /// ID *data; Object *parent }; the Mesh part follows blend_mesh_read's layout.
    fn sdna() -> Sdna {
        let members = [
            "name[258]", "id", "type", "rotmode", "loc[3]", "rot[3]", "size[3]", "*data", "*parent", "type",
            "name[68]", "*data", "*layers", "totlayer", "totvert", "totpoly", "totloop",
            "*poly_offset_indices", "vdata", "ldata", "_pad[4]",
        ];
        let types =
            ["char", "short", "float", "int", "void", "ID", "Object", "CustomDataLayer", "CustomData", "Mesh"];
        let st = |ty: i16, ms: &[(i16, i16)]| Struct {
            type_index: ty,
            members: ms.iter().map(|&(t, m)| StructMember { type_index: t, member_index: m }).collect(),
        };
        Sdna {
            members: members.iter().map(|s| s.to_string()).collect(),
            types: types.iter().map(|s| s.to_string()).collect(),
            types_size: vec![1, 2, 4, 4, 0, 264, 320, 80, 16, 48],
            structs: vec![
                st(5, &[(0, 0)]),
                st(6, &[(5, 1), (1, 2), (1, 3), (2, 4), (2, 5), (2, 6), (5, 7), (6, 8)]),
                st(7, &[(3, 9), (0, 10), (4, 11)]),
                st(8, &[(7, 12), (3, 13)]),
                st(9, &[(3, 14), (3, 15), (3, 16), (0, 20), (3, 17), (8, 18), (8, 19)]),
            ],
            pointer_size: 8,
            members_array_num: vec![258, 1, 1, 1, 3, 3, 3, 1, 1, 1, 68, 1, 1, 1, 1, 1, 1, 1, 1, 1, 4],
        }
    }

    fn object(name: &str, ty: i16, rotmode: i16, data: u64) -> Vec<u8> {
        let mut d = vec![0u8; 320];
        d[..name.len()].copy_from_slice(name.as_bytes());
        d[264..266].copy_from_slice(&ty.to_le_bytes());
        d[266..268].copy_from_slice(&rotmode.to_le_bytes());
        for (at, v) in [(268, 2.0f32), (280, 0.5), (292, 1.0)] {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        d[304..312].copy_from_slice(&data.to_le_bytes());
        d
    }

    /// A one-vertex mesh at x. The same DATA addresses are reused by every mesh.
    fn push_mesh(file: &mut File, old: u64, x: f32) {
        let mut me = vec![0u8; 48];
        me[..4].copy_from_slice(&1i32.to_le_bytes());
        me[24..32].copy_from_slice(&0x10u64.to_le_bytes());
        me[32..36].copy_from_slice(&1i32.to_le_bytes());
        file.push(CODE_ID_ME, old, &me);
        let mut layer = CD_PROP_FLOAT3.to_le_bytes().to_vec();
        layer.extend(b"position");
        layer.resize(72, 0);
        layer.extend(0x20u64.to_le_bytes());
        file.push(CODE_DATA, 0x10, &layer);
        let pos: Vec<u8> = [x, 0.0, 0.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        file.push(CODE_DATA, 0x20, &pos);
    }

    #[derive(Default)]
    struct File {
        bytes: Vec<u8>,
        blocks: Vec<Block>,
    }

    impl File {
        fn push(&mut self, code: [u8; 4], old: u64, p: &[u8]) {
            self.blocks.push(Block { code, sdna_nr: 0, old, len: p.len() as i64, nr: 1, data: self.bytes.len() });
            self.bytes.extend(p);
        }
        fn load(&self) -> Vec<BlendSceneObject> {
            assemble(&sdna(), &self.bytes, &self.blocks, false, 8).unwrap()
        }
    }

    #[test]
    fn objects_share_or_own_meshes() {
        let mut f = File::default();
        f.push(CODE_ID_OB, 1, &object("OBA", OB_MESH, ROT_MODE_EUL, 0x100));
        f.push(CODE_ID_OB, 2, &object("OBB", OB_MESH, 0, 0x100));
        f.push(CODE_ID_OB, 3, &object("OBC", OB_MESH, ROT_MODE_EUL, 0x200));
        push_mesh(&mut f, 0x100, 1.0);
        push_mesh(&mut f, 0x200, 7.0);
        let s = f.load();
        assert_eq!(s.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
        assert_eq!(s[0].mesh, s[1].mesh);
        assert_eq!((s[0].mesh.verts[0][0], s[2].mesh.verts[0][0]), (1.0, 7.0));
        assert_eq!((s[0].loc[0], s[0].rot[0], s[0].scale[0]), (2.0, 0.5, 1.0));
        assert_eq!(s[1].rot, [0.0; 3]);
    }

    #[test]
    fn non_mesh_and_dataless_objects_are_skipped() {
        let mut f = File::default();
        f.push(CODE_ID_OB, 1, &object("OBCamera", 11, ROT_MODE_EUL, 0x100));
        f.push(CODE_ID_OB, 2, &object("OBLinked", OB_MESH, ROT_MODE_EUL, 0x999));
        push_mesh(&mut f, 0x100, 1.0);
        assert!(f.load().is_empty());
    }

    #[test]
    fn compressed_and_foreign_files_are_errors() {
        for magic in [[0x1f, 0x8b, 0x08, 0], [0x28, 0xb5, 0x2f, 0xfd], [0x50, 0x2a, 0x4d, 0x18]] {
            assert!(load_scene(&magic).unwrap_err().starts_with("compressed"), "{magic:?}");
        }
        assert_eq!(load_scene(b"hello world!"), Err("not a Blender file"));
        assert_eq!(load_scene(b"BLENDER-v405ENDB"), Err("missing DNA1 block"));
    }
}
