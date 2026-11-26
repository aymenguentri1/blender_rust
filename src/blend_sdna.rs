//! `.blend` DNA1 (SDNA) block parser (makesdna/intern/dna_genfile.cc `init_structDNA` :381-690, helpers :137-168,
//! :280-380; `DNA_member_array_num` in dna_utils.cc:28). Not ported: legacy struct-rename hack
//! (`DNA_struct_rename_legacy_hack_static_from_alias`), alias maps, `mat4x4f` alignment, struct reconstruction.
//! The block data is little-endian when `big_endian` is false (header endianness decides, see the header reader).

#![allow(dead_code)]

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructMember {
    pub type_index: i16,
    pub member_index: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Struct {
    pub type_index: i16,
    pub members: Vec<StructMember>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sdna {
    pub members: Vec<String>,
    pub types: Vec<String>,
    pub types_size: Vec<i16>,
    pub structs: Vec<Struct>,
    pub pointer_size: usize,
    /// `DNA_member_array_num` of every entry of `members`.
    pub members_array_num: Vec<i16>,
}

/// `DNA_member_array_num`: product of all `[n]` factors in a member name.
pub fn member_array_num(name: &str) -> i32 {
    let (mut result, mut current) = (1i32, 0i32);
    for c in name.chars() {
        match c {
            '[' => current = 0,
            ']' => result = result.wrapping_mul(current),
            '0'..='9' => current = current.wrapping_mul(10).wrapping_add(c as i32 - '0' as i32),
            _ => {}
        }
    }
    result
}

/// `ispointer`: plain or function pointer member name.
pub fn is_pointer(name: &str) -> bool {
    let b = name.as_bytes();
    b.first() == Some(&b'*') || (b.first() == Some(&b'(') && b.get(1) == Some(&b'*'))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    big_endian: bool,
}

type R<T> = Result<T, &'static str>;

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or("Invalid data in SDNA file")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn tag(&mut self) -> R<[u8; 4]> {
        let b = self.take(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    fn i32(&mut self) -> R<i32> {
        let b = self.take(4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Ok(if self.big_endian { i32::from_be_bytes(a) } else { i32::from_le_bytes(a) })
    }

    fn i16(&mut self) -> R<i16> {
        let b = self.take(2)?;
        let a = [b[0], b[1]];
        Ok(if self.big_endian { i16::from_be_bytes(a) } else { i16::from_le_bytes(a) })
    }

    /// `sdna_data_validate_array_size`: a count may not exceed the bytes left.
    fn count(&mut self) -> R<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.data.len() - self.pos {
            return Err("Invalid array size data in SDNA file");
        }
        Ok(n as usize)
    }

    /// `sdna_data_pointer_read_string`: non-empty, NUL terminated.
    fn string(&mut self) -> R<String> {
        let rest = &self.data[self.pos..];
        let len = rest.iter().position(|&b| b == 0).filter(|&l| l > 0).ok_or("Invalid string data in SDNA file")?;
        self.pos += len + 1;
        Ok(String::from_utf8_lossy(&rest[..len]).into_owned())
    }

    fn pad_up_4(&mut self) {
        self.pos = (self.pos + 3) & !3;
    }
}

/// `DNA_sdna_from_data` without aliasing: parses the payload of the `DNA1` block (starts with `SDNA`).
pub fn parse_sdna(data: &[u8], big_endian: bool) -> Result<Sdna, &'static str> {
    let mut r = Reader { data, pos: 0, big_endian };
    if &r.tag()? != b"SDNA" {
        return Err("SDNA error in SDNA file");
    }

    /* NAME */
    if &r.tag()? != b"NAME" {
        return Err("NAME error in SDNA file");
    }
    let members_num = r.count()?;
    if members_num == 0 {
        return Err("NAME error in SDNA file");
    }
    let mut members: Vec<String> = Vec::with_capacity(members_num);
    let mut gravity_fix = false;
    for i in 0..members_num {
        let mut name = r.string()?;
        /* "float gravity [3]" parsed as "gravity" + "[3]" in pre-2011 files (#20330). */
        if name == "[3]" && i > 0 && members[i - 1] == "Cvi" {
            name = "gravity[3]".into();
            gravity_fix = true;
        }
        members.push(name);
    }
    r.pad_up_4();

    /* TYPE */
    if &r.tag()? != b"TYPE" {
        return Err("TYPE error in SDNA file");
    }
    let types_num = r.count()?;
    if types_num == 0 {
        return Err("TYPE error in SDNA file");
    }
    let types = (0..types_num).map(|_| r.string()).collect::<R<Vec<_>>>()?;
    r.pad_up_4();

    /* TLEN */
    if &r.tag()? != b"TLEN" {
        return Err("TLEN error in SDNA file");
    }
    let types_size = (0..types_num).map(|_| r.i16()).collect::<R<Vec<_>>>()?;
    if types_num & 1 == 1 {
        r.take(2)?; /* prevent BUS error */
    }

    /* STRC */
    if &r.tag()? != b"STRC" {
        return Err("STRC error in SDNA file");
    }
    let structs_num = r.count()?;
    if structs_num == 0 {
        return Err("STRC error in SDNA file");
    }
    let mut structs: Vec<Struct> = Vec::with_capacity(structs_num);
    let mut seen = HashSet::new();
    let mut gravity_struct = None;
    for si in 0..structs_num {
        let type_index = r.i16()?;
        let n = r.i16()?;
        let n_usize = if n < 0 { return Err("Invalid array size data in SDNA file") } else { n as usize };
        if n_usize * 2 * 2 > r.data.len() - r.pos {
            return Err("Invalid array size data in SDNA file");
        }
        let mut sm = Vec::with_capacity(n_usize);
        for _ in 0..n_usize {
            sm.push(StructMember { type_index: r.i16()?, member_index: r.i16()? });
        }
        if type_index < 0 || type_index as usize >= types_num {
            return Err("Invalid struct type index in SDNA file");
        }
        if !seen.insert(type_index) {
            return Err("Invalid duplicate struct type index in SDNA file");
        }
        for m in &sm {
            if m.type_index < 0 || m.type_index as usize >= types_num {
                return Err("Invalid struct member's type index in SDNA file");
            }
            if m.member_index < 0 || m.member_index as usize >= members_num {
                return Err("Invalid struct member's member index in SDNA file");
            }
        }
        /* Second part of the gravity fix: type of "gravity" goes from float (7) to void (9). */
        if gravity_fix && types[type_index as usize] == "ClothSimSettings" {
            let (float_ti, void_ti, g) = (7usize, 9usize, 4usize);
            if sm.len() < g + 2
                || sm[g].type_index as usize != float_ti
                || members[sm[g].member_index as usize] != "gravity"
                || members[sm[g + 1].member_index as usize] != "gravity[3]"
                || float_ti >= types_num
                || types[float_ti] != "float"
                || void_ti >= types_num
                || types[void_ti] != "void"
            {
                return Err("Invalid data in SDNA file");
            }
            sm[g].type_index = void_ti as i16;
            gravity_struct = Some(si);
        }
        structs.push(Struct { type_index, members: sm });
    }
    if r.pos != data.len() {
        return Err("Invalid data in SDNA file");
    }

    /* `sdna->pointer_size` = half the size of ListBase (two pointers). */
    let lb = structs
        .iter()
        .find(|s| types[s.type_index as usize] == "ListBase")
        .ok_or("ListBase struct error! Not found.")?;
    let pointer_size = (types_size[lb.type_index as usize] / 2) as usize;
    if lb.members.len() != 2 || !matches!(pointer_size, 4 | 8) {
        return Err("ListBase struct error: invalid computed pointer-size.");
    }

    let mut members_array_num = Vec::with_capacity(members_num);
    for m in &members {
        let n = member_array_num(m);
        if !(0..=i16::MAX as i32).contains(&n) {
            return Err("Invalid struct member array size in SDNA file");
        }
        members_array_num.push(n as i16);
    }

    /* `sdna_struct_size_check`: declared size equals the sum of the member sizes. */
    for (si, s) in structs.iter().enumerate() {
        let mut total: i64 = if gravity_struct == Some(si) { 4 } else { 0 };
        for m in &s.members {
            let name = &members[m.member_index as usize];
            let size = if is_pointer(name) { pointer_size as i64 } else { types_size[m.type_index as usize] as i64 };
            if size < 0 {
                return Err("Invalid struct member size in SDNA file");
            }
            total += size * members_array_num[m.member_index as usize] as i64;
        }
        if total != types_size[s.type_index as usize] as i64 {
            return Err("Invalid struct size in SDNA file");
        }
    }

    Ok(Sdna { members, types, types_size, structs, pointer_size, members_array_num })
}

impl Sdna {
    /// `DNA_struct_find_index_without_alias`.
    pub fn struct_find(&self, name: &str) -> Option<usize> {
        self.structs.iter().position(|s| self.types[s.type_index as usize] == name)
    }

    /// `DNA_struct_size`.
    pub fn struct_size(&self, struct_index: usize) -> i32 {
        self.types_size[self.structs[struct_index].type_index as usize] as i32
    }

    /// `DNA_struct_member_size`.
    pub fn member_size(&self, type_index: usize, member_index: usize) -> i32 {
        let arr = self.members_array_num[member_index] as i32;
        if is_pointer(&self.members[member_index]) {
            self.pointer_size as i32 * arr
        } else if self.types_size[type_index] != 0 {
            self.types_size[type_index] as i32 * arr
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an SDNA block: `structs` are `(type, [(member_type, member_name)])`.
    fn build(types: &[(&str, i16)], members: &[&str], structs: &[(usize, &[(usize, usize)])], be: bool) -> Vec<u8> {
        let i32b = |v: i32| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let i16b = |v: i16| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let pad = |o: &mut Vec<u8>| while !o.len().is_multiple_of(4) { o.push(0) };
        let mut o = b"SDNANAME".to_vec();
        o.extend(i32b(members.len() as i32));
        for m in members {
            o.extend(m.as_bytes());
            o.push(0);
        }
        pad(&mut o);
        o.extend(b"TYPE");
        o.extend(i32b(types.len() as i32));
        for (t, _) in types {
            o.extend(t.as_bytes());
            o.push(0);
        }
        pad(&mut o);
        o.extend(b"TLEN");
        for (_, s) in types {
            o.extend(i16b(*s));
        }
        pad(&mut o);
        o.extend(b"STRC");
        o.extend(i32b(structs.len() as i32));
        for (t, ms) in structs {
            o.extend(i16b(*t as i16));
            o.extend(i16b(ms.len() as i16));
            for (mt, mi) in *ms {
                o.extend(i16b(*mt as i16));
                o.extend(i16b(*mi as i16));
            }
        }
        o
    }

    /// types: 0 int(4) 1 ListBase(16) 2 Foo(20); members: 0 "*first" 1 "*last" 2 "a" 3 "v[2][3]"
    fn sample(be: bool) -> Vec<u8> {
        build(
            &[("int", 4), ("ListBase", 16), ("Foo", 28)],
            &["*first", "*last", "a", "v[2][3]"],
            &[(1, &[(1, 0), (1, 1)]), (2, &[(0, 2), (0, 3)])],
            be,
        )
    }

    #[test]
    fn parses_a_small_sdna() {
        let s = parse_sdna(&sample(false), false).unwrap();
        assert_eq!(s.pointer_size, 8);
        assert_eq!(s.types, ["int", "ListBase", "Foo"]);
        assert_eq!(s.members_array_num, [1, 1, 1, 6]);
        let foo = s.struct_find("Foo").unwrap();
        assert_eq!(s.struct_size(foo), 28);
        assert_eq!(s.member_size(0, 3), 24);
        assert_eq!(s.member_size(1, 0), 8, "pointer member");
        assert_eq!(s.struct_find("Nope"), None);
    }

    #[test]
    fn big_endian_data_parses_the_same() {
        assert_eq!(parse_sdna(&sample(true), true).unwrap(), parse_sdna(&sample(false), false).unwrap());
    }

    #[test]
    fn array_sizes_multiply() {
        assert_eq!((member_array_num("a"), member_array_num("v[4]"), member_array_num("m[4][4]"), member_array_num("*p[2]")), (1, 4, 16, 2));
        assert!(is_pointer("*x") && is_pointer("(*f)()") && !is_pointer("x") && !is_pointer("("));
    }

    #[test]
    fn rejects_bad_headers_and_truncation() {
        let mut d = sample(false);
        d[0] = b'X';
        assert_eq!(parse_sdna(&d, false), Err("SDNA error in SDNA file"));
        let d = sample(false);
        assert!(parse_sdna(&d[..d.len() - 2], false).is_err());
        let mut d = sample(false);
        d.push(0);
        assert_eq!(parse_sdna(&d, false), Err("Invalid data in SDNA file"));
        let mut d = sample(false);
        d[4..8].copy_from_slice(b"NAMX");
        assert_eq!(parse_sdna(&d, false), Err("NAME error in SDNA file"));
    }

    #[test]
    fn rejects_inconsistent_structs() {
        /* Foo declared 20 bytes but its members add up to 28. */
        let d = build(
            &[("int", 4), ("ListBase", 16), ("Foo", 20)],
            &["*first", "*last", "a", "v[2][3]"],
            &[(1, &[(1, 0), (1, 1)]), (2, &[(0, 2), (0, 3)])],
            false,
        );
        assert_eq!(parse_sdna(&d, false), Err("Invalid struct size in SDNA file"));
        /* Duplicate struct type. */
        let d = build(&[("int", 4), ("ListBase", 16)], &["*first", "*last"], &[(1, &[(1, 0), (1, 1)]), (1, &[(1, 0), (1, 1)])], false);
        assert_eq!(parse_sdna(&d, false), Err("Invalid duplicate struct type index in SDNA file"));
        /* No ListBase. */
        let d = build(&[("int", 4)], &["a"], &[(0, &[])], false);
        assert_eq!(parse_sdna(&d, false), Err("ListBase struct error! Not found."));
        /* Member index out of range. */
        let d = build(&[("int", 4), ("ListBase", 16)], &["*first", "*last"], &[(1, &[(1, 0), (1, 5)])], false);
        assert_eq!(parse_sdna(&d, false), Err("Invalid struct member's member index in SDNA file"));
    }

    #[test]
    fn four_byte_pointers_are_derived_from_listbase() {
        let d = build(&[("ListBase", 8)], &["*first", "*last"], &[(0, &[(0, 0), (0, 1)])], false);
        assert_eq!(parse_sdna(&d, false).unwrap().pointer_size, 4);
    }
}
