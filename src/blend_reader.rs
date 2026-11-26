//! Block iteration of a `.blend` file and listing of its Object IDs.
//! Ports `BLO_readfile_read_bhead()` (blenloader_core/intern/blo_core_bhead.cc, the `BHead4`,
//! `SmallBHead8` and `LargeBHead8` layouts of BLO_core_bhead.hh) and the `ID` layout of
//! DNA_ID.h (`name` follows five pointers: `next`, `prev`, `newid`, `lib`, `asset_data`).
//! Object blocks use the code `ID_OB` (DNA_ID_enums.h). Block codes are compared as the raw
//! four bytes of the file (`BLEND_MAKE_ID` lays them out in file order for either endianness).

use crate::blend_header::{decode_blender_header, BHeadType, Endian, HeaderVariant, MAX_SIZEOFBLENDERHEADER, MIN_SIZEOFBLENDERHEADER, BLEND_FILE_FORMAT_VERSION_1};

pub const CODE_ENDB: [u8; 4] = *b"ENDB";
pub const CODE_ID_OB: [u8; 4] = [b'O', b'B', 0, 0];

/// `BHead` plus where its payload sits in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub code: [u8; 4],
    pub sdna_nr: i32,
    pub old: u64,
    pub len: i64,
    pub nr: i64,
    /// Offset of the payload in the file bytes.
    pub data: usize,
}

#[derive(Clone, Copy)]
struct Rd<'a> {
    b: &'a [u8],
    big: bool,
}

impl Rd<'_> {
    fn i32(&self, at: usize) -> i32 {
        let a = [self.b[at], self.b[at + 1], self.b[at + 2], self.b[at + 3]];
        if self.big { i32::from_be_bytes(a) } else { i32::from_le_bytes(a) }
    }
    fn u32(&self, at: usize) -> u32 {
        self.i32(at) as u32
    }
    fn u64(&self, at: usize) -> u64 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&self.b[at..at + 8]);
        if self.big { u64::from_be_bytes(a) } else { u64::from_le_bytes(a) }
    }
    fn i64(&self, at: usize) -> i64 {
        self.u64(at) as i64
    }
}

fn bhead_size(t: BHeadType) -> usize {
    match t {
        BHeadType::BHead4 => 20,
        BHeadType::SmallBHead8 => 24,
        BHeadType::LargeBHead8 => 32,
    }
}

/// Reads one block header at `pos`. Like Blender, a short read is accepted only for `ENDB`.
fn read_bhead(rd: Rd, pos: usize, t: BHeadType) -> Option<Block> {
    let size = bhead_size(t);
    let avail = rd.b.len().saturating_sub(pos);
    if avail < 4 {
        return None;
    }
    let code = [rd.b[pos], rd.b[pos + 1], rd.b[pos + 2], rd.b[pos + 3]];
    if avail < size && code != CODE_ENDB {
        return None;
    }
    if avail < size {
        // ENDB with a truncated header: Blender's zero-initialised remainder.
        return Some(Block { code, sdna_nr: 0, old: 0, len: 0, nr: 0, data: rd.b.len() });
    }
    let p = pos;
    Some(match t {
        BHeadType::BHead4 => Block { code, len: rd.i32(p + 4) as i64, old: rd.u32(p + 8) as u64, sdna_nr: rd.i32(p + 12), nr: rd.i32(p + 16) as i64, data: p + size },
        BHeadType::SmallBHead8 => Block { code, len: rd.i32(p + 4) as i64, old: rd.u64(p + 8), sdna_nr: rd.i32(p + 16), nr: rd.i32(p + 20) as i64, data: p + size },
        BHeadType::LargeBHead8 => Block { code, sdna_nr: rd.i32(p + 4), old: rd.u64(p + 8), len: rd.i64(p + 16), nr: rd.i64(p + 24), data: p + size },
    })
}

/// All blocks of the file up to and including `ENDB`.
pub fn read_blocks(bytes: &[u8]) -> Result<Vec<Block>, &'static str> {
    let h = match decode_blender_header(bytes) {
        HeaderVariant::Header(h) => h,
        HeaderVariant::Invalid => return Err("not a Blender file"),
        HeaderVariant::Unknown => return Err("unknown Blender file header"),
    };
    let rd = Rd { b: bytes, big: h.endian == Endian::Big };
    let t = h.bhead_type();
    let mut pos = if h.file_format_version == BLEND_FILE_FORMAT_VERSION_1 { MAX_SIZEOFBLENDERHEADER } else { MIN_SIZEOFBLENDERHEADER };
    let mut out = Vec::new();
    loop {
        let b = read_bhead(rd, pos, t).ok_or("truncated block header")?;
        out.push(b);
        if b.code == CODE_ENDB {
            return Ok(out);
        }
        if b.len < 0 {
            return Err("invalid block length");
        }
        pos = b.data.checked_add(b.len as usize).filter(|&e| e <= bytes.len()).ok_or("truncated block data")?;
    }
}

/// Names of the Object IDs (`ID::name` without the two-letter code prefix), in file order.
pub fn list_object_names(bytes: &[u8]) -> Result<Vec<String>, &'static str> {
    let ptr = match decode_blender_header(bytes) {
        HeaderVariant::Header(h) => h.pointer_size as usize,
        _ => return Err("not a Blender file"),
    };
    let name_ofs = 5 * ptr;
    let mut names = Vec::new();
    for b in read_blocks(bytes)? {
        if b.code != CODE_ID_OB {
            continue;
        }
        let data = &bytes[b.data..b.data + b.len as usize];
        let raw = data.get(name_ofs..).ok_or("object block too small")?;
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        let full = String::from_utf8_lossy(&raw[..end]);
        names.push(full.get(2..).unwrap_or("").to_string());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum Fmt { V4, Small8, Large8, Big4 }

    fn header(f: Fmt) -> Vec<u8> {
        match f {
            Fmt::V4 => b"BLENDER_v405".to_vec(),
            Fmt::Big4 => b"BLENDER_V405".to_vec(),
            Fmt::Small8 => b"BLENDER-v405".to_vec(),
            Fmt::Large8 => b"BLENDER17-01v0500".to_vec(),
        }
    }

    fn block(f: Fmt, code: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut o = code.to_vec();
        let be = matches!(f, Fmt::Big4);
        let i32b = |v: i32| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let u64b = |v: u64| v.to_le_bytes();
        match f {
            Fmt::V4 | Fmt::Big4 => {
                o.extend(i32b(payload.len() as i32));
                o.extend(i32b(0x1000)); // old
                o.extend(i32b(7)); // SDNAnr
                o.extend(i32b(1)); // nr
            }
            Fmt::Small8 => {
                o.extend(i32b(payload.len() as i32));
                o.extend(u64b(0x1000));
                o.extend(i32b(7));
                o.extend(i32b(1));
            }
            Fmt::Large8 => {
                o.extend(7i32.to_le_bytes());
                o.extend(u64b(0x1000));
                o.extend((payload.len() as i64).to_le_bytes());
                o.extend(1i64.to_le_bytes());
            }
        }
        o.extend(payload);
        o
    }

    fn id_payload(ptr: usize, name: &str) -> Vec<u8> {
        let mut p = vec![0u8; 5 * ptr];
        p.extend(name.as_bytes());
        p.resize(5 * ptr + 258 + 16, 0);
        p
    }

    fn file(f: Fmt, objs: &[&str]) -> Vec<u8> {
        let ptr = if matches!(f, Fmt::V4 | Fmt::Big4) { 4 } else { 8 };
        let mut o = header(f);
        o.extend(block(f, *b"REND", &[0; 8]));
        for n in objs {
            o.extend(block(f, CODE_ID_OB, &id_payload(ptr, n)));
            o.extend(block(f, *b"DATA", &[1, 2, 3]));
        }
        o.extend(block(f, *b"ME\0\0", &id_payload(ptr, "MEMesh")));
        o.extend(block(f, CODE_ENDB, &[]));
        o
    }

    #[test]
    fn lists_objects_in_every_header_layout() {
        for f in [Fmt::V4, Fmt::Big4, Fmt::Small8, Fmt::Large8] {
            let bytes = file(f, &["OBCube", "OBLight", "OBCamera"]);
            assert_eq!(list_object_names(&bytes).unwrap(), ["Cube", "Light", "Camera"]);
        }
    }

    #[test]
    fn block_headers_are_decoded() {
        let bytes = file(Fmt::Large8, &["OBCube"]);
        let b = read_blocks(&bytes).unwrap();
        assert_eq!(b[0].code, *b"REND");
        assert_eq!((b[0].len, b[0].sdna_nr, b[0].old, b[0].nr), (8, 7, 0x1000, 1));
        assert_eq!(b.last().unwrap().code, CODE_ENDB);
        assert_eq!(b.len(), 5);
        let bytes = file(Fmt::Big4, &["OBCube"]);
        let b = read_blocks(&bytes).unwrap();
        assert_eq!((b[1].len, b[1].old, b[1].sdna_nr), (4 * 5 + 258 + 16, 0x1000, 7));
    }

    #[test]
    fn no_objects_gives_an_empty_list() {
        assert!(list_object_names(&file(Fmt::V4, &[])).unwrap().is_empty());
    }

    #[test]
    fn errors() {
        assert!(read_blocks(b"not a blend file").is_err());
        let mut bytes = file(Fmt::V4, &["OBCube"]);
        bytes.truncate(60);
        assert_eq!(read_blocks(&bytes), Err("truncated block data"));
        let mut bytes = file(Fmt::V4, &[]);
        bytes.truncate(bytes.len() - 20 - 3); // cut into the ME block
        assert!(read_blocks(&bytes).is_err());
    }

    #[test]
    fn endb_may_be_a_short_read() {
        let mut bytes = header(Fmt::V4);
        bytes.extend(b"ENDB");
        assert_eq!(read_blocks(&bytes).unwrap().len(), 1);
    }
}
