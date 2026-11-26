//! Read an `Object` ID block of a `.blend`.
//!
//! Members from `DNA_object_types.h`: `ID id` (:473), `ObjectType type` (`enum ObjectType : short`,
//! `OB_MESH = 1`, :99), `ID *data` (:496), `Object *parent` (:483), `float loc[3]` (:534),
//! `scale[3]` (:536), `rot[3]` (:542), `short rotmode` (:585, `ROT_MODE_EUL = 1`).
//! `ID::name` is `char name[258]` (`DNA_ID.h:434`): a two letter ID code (`OB`) and the name.

use crate::blend_dna_read::BlockReader;
use crate::blend_sdna::Sdna;

pub const OB_MESH: i16 = 1;
pub const ROT_MODE_EUL: i16 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct BlendObject {
    /// `ID::name` without the two letter code.
    pub name: String,
    pub ob_type: i16,
    /// Old address of `Object::data` (0 when none).
    pub data: u64,
    /// Old address of `Object::parent` (0 when none).
    pub parent: u64,
    pub loc: [f32; 3],
    /// Euler angles in radians (only meaningful when `rotmode == ROT_MODE_EUL`).
    pub rot: [f32; 3],
    pub scale: [f32; 3],
    pub rotmode: i16,
}

type R<T> = Result<T, &'static str>;

fn off(sdna: &Sdna, st: &str, ty: &str, name: &str) -> R<usize> {
    sdna.member_offset(st, ty, name).ok_or("missing DNA member")
}

/// Read one `Object` block payload.
pub fn read_object(sdna: &Sdna, ob: BlockReader) -> R<BlendObject> {
    let id_off = off(sdna, "Object", "ID", "id")?;
    let name_off = id_off + off(sdna, "ID", "char", "name[258]")?;
    let full = ob.cstr(name_off, 258).ok_or("short object")?;
    let name = full.get(2..).ok_or("bad ID name")?.to_string();
    let short = |n: &str| ob.i16(off(sdna, "Object", "short", n)?).ok_or("short object");
    let v3 = |n: &str| ob.f32s::<3>(off(sdna, "Object", "float", n)?).ok_or("short object");
    let ptr = |ty: &str, n: &str| ob.ptr(off(sdna, "Object", ty, n)?).ok_or("short object");
    Ok(BlendObject {
        name,
        ob_type: short("type")?,
        data: ptr("ID", "*data")?,
        parent: ptr("Object", "*parent")?,
        loc: v3("loc[3]")?,
        rot: v3("rot[3]")?,
        scale: v3("scale[3]")?,
        rotmode: short("rotmode")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_sdna::{Struct, StructMember};

    /// ID { char name[258] } (264 bytes); Object { ID id; short type; short rotmode; float loc[3],
    /// rot[3], scale[3]; ID *data; Object *parent; } -> loc 268, rot 280, scale 292, data 304, parent 312.
    fn sdna() -> Sdna {
        let st = |ty: i16, ms: &[(i16, i16)]| Struct {
            type_index: ty,
            members: ms.iter().map(|&(t, m)| StructMember { type_index: t, member_index: m }).collect(),
        };
        Sdna {
            members: ["name[258]", "id", "type", "rotmode", "loc[3]", "rot[3]", "scale[3]", "*data", "*parent"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            types: ["char", "short", "float", "ID", "Object"].iter().map(|s| s.to_string()).collect(),
            types_size: vec![1, 2, 4, 264, 320],
            structs: vec![
                st(3, &[(0, 0)]),
                st(4, &[(3, 1), (1, 2), (1, 3), (2, 4), (2, 5), (2, 6), (3, 7), (4, 8)]),
            ],
            pointer_size: 8,
            members_array_num: vec![258, 1, 1, 1, 3, 3, 3, 1, 1],
        }
    }

    fn payload(name: &str) -> Vec<u8> {
        let mut d = vec![0u8; 320];
        d[..name.len()].copy_from_slice(name.as_bytes());
        d[264..266].copy_from_slice(&OB_MESH.to_le_bytes());
        d[266..268].copy_from_slice(&ROT_MODE_EUL.to_le_bytes());
        let mut put = |at: usize, v: [f32; 3]| {
            for (i, f) in v.iter().enumerate() {
                d[at + 4 * i..at + 4 * i + 4].copy_from_slice(&f.to_bits().to_le_bytes());
            }
        };
        put(268, [1.0, 2.0, 3.0]);
        put(280, [0.5, 0.0, -1.0]);
        put(292, [1.0, 2.0, 1.0]);
        d[304..312].copy_from_slice(&0xabc0u64.to_le_bytes());
        d[312..320].copy_from_slice(&0x77u64.to_le_bytes());
        d
    }

    #[test]
    fn reads_object_fields() {
        let p = payload("OBCube");
        let rd = BlockReader { data: &p, big_endian: false, pointer_size: 8 };
        let o = read_object(&sdna(), rd).unwrap();
        assert_eq!(o.name, "Cube");
        assert_eq!((o.ob_type, o.rotmode), (OB_MESH, ROT_MODE_EUL));
        assert_eq!((o.loc, o.rot, o.scale), ([1.0, 2.0, 3.0], [0.5, 0.0, -1.0], [1.0, 2.0, 1.0]));
        assert_eq!((o.data, o.parent), (0xabc0, 0x77));
    }

    #[test]
    fn short_payload_is_an_error() {
        let p = payload("OBCube");
        let rd = BlockReader { data: &p[..300], big_endian: false, pointer_size: 8 };
        assert_eq!(read_object(&sdna(), rd), Err("short object"));
    }

    #[test]
    fn missing_member_is_an_error() {
        let mut s = sdna();
        s.members[4] = "location[3]".to_string();
        let p = payload("OBCube");
        let rd = BlockReader { data: &p, big_endian: false, pointer_size: 8 };
        assert_eq!(read_object(&s, rd), Err("missing DNA member"));
    }
}
