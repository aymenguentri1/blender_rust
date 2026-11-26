//! Member lookup and typed reads for `.blend` block payloads.
//!
//! Ports `elem_streq()` (dna_genfile.cc:1234), `elem_offset_impl()` (:1311) and
//! `DNA_struct_member_offset_by_name_without_alias()` (:1917). Member names keep their DNA
//! decoration (`*next`, `loc[3]`): callers pass the full name, only the size inside `[..]` may differ.

use crate::blend_sdna::Sdna;

/// `elem_streq()`: equality of two member names excluding any array-size suffix.
pub fn elem_streq(name: &str, oname: &str) -> bool {
    let (n, o) = (name.as_bytes(), oname.as_bytes());
    for a in 0..n.len().min(o.len()) {
        if n[a] != o[a] {
            return false;
        }
        if n[a] == b'[' || o[a] == b'[' {
            return true;
        }
    }
    n.len() == o.len()
}

impl Sdna {
    /// `DNA_struct_member_offset_by_name_without_alias()`: byte offset of the member `name` of
    /// type `vartype` in struct `stype`, or `None` (Blender's -1) when missing or of another type.
    pub fn member_offset(&self, stype: &str, vartype: &str, name: &str) -> Option<usize> {
        let st = &self.structs[self.struct_find(stype)?];
        let mut offset = 0usize;
        for m in &st.members {
            let otype = &self.types[m.type_index as usize];
            let oname = &self.members[m.member_index as usize];
            if elem_streq(name, oname) {
                return (vartype == otype).then_some(offset);
            }
            offset += self.member_size(m.type_index as usize, m.member_index as usize) as usize;
        }
        None
    }
}

/// Typed little/big endian reads from one block payload.
#[derive(Clone, Copy)]
pub struct BlockReader<'a> {
    pub data: &'a [u8],
    pub big_endian: bool,
    pub pointer_size: usize,
}

impl<'a> BlockReader<'a> {
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.data.get(at..at.checked_add(N)?)?.try_into().ok()
    }
    pub fn i32(&self, at: usize) -> Option<i32> {
        let b = self.bytes::<4>(at)?;
        Some(if self.big_endian { i32::from_be_bytes(b) } else { i32::from_le_bytes(b) })
    }
    pub fn i16(&self, at: usize) -> Option<i16> {
        let b = self.bytes::<2>(at)?;
        Some(if self.big_endian { i16::from_be_bytes(b) } else { i16::from_le_bytes(b) })
    }
    pub fn f32(&self, at: usize) -> Option<f32> {
        self.i32(at).map(|v| f32::from_bits(v as u32))
    }
    pub fn f32s<const N: usize>(&self, at: usize) -> Option<[f32; N]> {
        let mut out = [0.0; N];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.f32(at + 4 * i)?;
        }
        Some(out)
    }
    /// A pointer value (the old address), 4 or 8 bytes wide.
    pub fn ptr(&self, at: usize) -> Option<u64> {
        match self.pointer_size {
            4 => {
                let b = self.bytes::<4>(at)?;
                Some(if self.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) } as u64)
            }
            8 => {
                let b = self.bytes::<8>(at)?;
                Some(if self.big_endian { u64::from_be_bytes(b) } else { u64::from_le_bytes(b) })
            }
            _ => None,
        }
    }
    /// A fixed-size char array up to the first NUL.
    pub fn cstr(&self, at: usize, max: usize) -> Option<String> {
        let s = self.data.get(at..at.checked_add(max)?)?;
        let end = s.iter().position(|&c| c == 0).unwrap_or(max);
        Some(String::from_utf8_lossy(&s[..end]).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_sdna::{Struct, StructMember};

    fn sdna() -> Sdna {
        // struct Foo { int a; float loc[3]; char name[8]; Foo *next; }
        let members = ["a", "loc[3]", "name[8]", "*next"];
        Sdna {
            members: members.iter().map(|s| s.to_string()).collect(),
            types: ["int", "float", "char", "Foo"].iter().map(|s| s.to_string()).collect(),
            types_size: vec![4, 4, 1, 32],
            structs: vec![Struct {
                type_index: 3,
                members: vec![(0, 0), (1, 1), (2, 2), (3, 3)]
                    .into_iter()
                    .map(|(t, m)| StructMember { type_index: t, member_index: m })
                    .collect(),
            }],
            pointer_size: 8,
            members_array_num: vec![1, 3, 8, 1],
        }
    }

    #[test]
    fn streq_ignores_array_suffix() {
        // Both names are compared up to the first `[`: only the size part may differ.
        assert!(elem_streq("loc[3]", "loc[4]"));
        assert!(elem_streq("loc[3]", "loc[3]"));
        // Like Blender, a name without suffix does not match one with a suffix.
        assert!(!elem_streq("loc", "loc[3]"));
        assert!(!elem_streq("loc", "location"));
        assert!(!elem_streq("lo", "loc"));
        assert!(elem_streq("a", "a"));
    }

    #[test]
    fn offsets_by_name_and_type() {
        let s = sdna();
        assert_eq!(s.member_offset("Foo", "int", "a"), Some(0));
        assert_eq!(s.member_offset("Foo", "float", "loc[3]"), Some(4));
        assert_eq!(s.member_offset("Foo", "char", "name[8]"), Some(16));
        assert_eq!(s.member_offset("Foo", "Foo", "*next"), Some(24));
    }

    #[test]
    fn missing_struct_member_or_wrong_type_is_none() {
        let s = sdna();
        assert_eq!(s.member_offset("Bar", "int", "a"), None);
        assert_eq!(s.member_offset("Foo", "int", "nope"), None);
        assert_eq!(s.member_offset("Foo", "float", "a"), None);
    }

    fn payload(be: bool) -> Vec<u8> {
        let w32 = |v: u32| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let mut d = Vec::new();
        d.extend(w32(7));
        for f in [1.0f32, 2.5, -3.0] {
            d.extend(w32(f.to_bits()));
        }
        d.extend(b"Cube\0\0\0\0");
        d.extend(if be { 0xdead_beef_u64.to_be_bytes() } else { 0xdead_beef_u64.to_le_bytes() });
        d
    }

    #[test]
    fn typed_reads_both_endians() {
        for be in [false, true] {
            let p = payload(be);
            let r = BlockReader { data: &p, big_endian: be, pointer_size: 8 };
            assert_eq!(r.i32(0), Some(7));
            assert_eq!(r.f32s::<3>(4), Some([1.0, 2.5, -3.0]));
            assert_eq!(r.cstr(16, 8).as_deref(), Some("Cube"));
            assert_eq!(r.ptr(24), Some(0xdead_beef));
        }
    }

    #[test]
    fn out_of_range_reads_are_none() {
        let p = payload(false);
        let r = BlockReader { data: &p, big_endian: false, pointer_size: 8 };
        assert_eq!(r.i32(p.len() - 3), None);
        assert_eq!(r.ptr(p.len()), None);
        assert_eq!(r.cstr(p.len() - 2, 8), None);
        assert_eq!(r.i32(usize::MAX), None);
    }

    #[test]
    fn four_byte_pointers() {
        let d = [1u8, 2, 3, 4];
        let r = BlockReader { data: &d, big_endian: false, pointer_size: 4 };
        assert_eq!(r.ptr(0), Some(0x0403_0201));
    }
}
