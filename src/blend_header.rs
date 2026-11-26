//! Port of `source/blender/blenloader_core/intern/blo_core_blend_header.cc`
//! (`BLO_readfile_blender_header_decode` and `BlenderHeader::bhead_type`).

pub const MIN_SIZEOFBLENDERHEADER: usize = 12;
pub const MAX_SIZEOFBLENDERHEADER: usize = 17;
pub const BLEND_FILE_FORMAT_VERSION_0: i32 = 0;
pub const BLEND_FILE_FORMAT_VERSION_1: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BHeadType {
    BHead4,
    SmallBHead8,
    LargeBHead8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlenderHeader {
    /// 4 or 8.
    pub pointer_size: i32,
    pub endian: Endian,
    /// `BLENDER_FILE_VERSION`.
    pub file_version: i32,
    /// `BLEND_FILE_FORMAT_VERSION`.
    pub file_format_version: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderVariant {
    /// Not a Blender file.
    Invalid,
    /// Detected as a Blender file but could not be decoded.
    Unknown,
    Header(BlenderHeader),
}

impl BlenderHeader {
    pub fn bhead_type(&self) -> BHeadType {
        if self.pointer_size == 4 {
            return BHeadType::BHead4;
        }
        if self.file_format_version == BLEND_FILE_FORMAT_VERSION_0 {
            return BHeadType::SmallBHead8;
        }
        BHeadType::LargeBHead8
    }
}

/// Parses ASCII digits (all must be digits), like the `isdigit` + `atoi` sequence in Blender.
fn digits(b: &[u8]) -> Option<i32> {
    if b.iter().all(|c| c.is_ascii_digit()) {
        Some(b.iter().fold(0, |a, c| a * 10 + (c - b'0') as i32))
    } else {
        None
    }
}

/// `BLO_readfile_blender_header_decode()` on the leading bytes of a file.
pub fn decode_blender_header(bytes: &[u8]) -> HeaderVariant {
    if bytes.len() < MIN_SIZEOFBLENDERHEADER || &bytes[..7] != b"BLENDER" {
        return HeaderVariant::Invalid;
    }
    if matches!(bytes[7], b'_' | b'-') {
        let pointer_size = if bytes[7] == b'_' { 4 } else { 8 };
        let endian = match bytes[8] {
            b'v' => Endian::Little,
            b'V' => Endian::Big,
            _ => return HeaderVariant::Unknown,
        };
        return match digits(&bytes[9..12]) {
            Some(file_version) => HeaderVariant::Header(BlenderHeader {
                pointer_size,
                endian,
                file_version,
                file_format_version: BLEND_FILE_FORMAT_VERSION_0,
            }),
            None => HeaderVariant::Unknown,
        };
    }
    match digits(&bytes[7..9]) {
        Some(n) if n as usize == MAX_SIZEOFBLENDERHEADER => {}
        _ => return HeaderVariant::Unknown,
    }
    if bytes.len() < MAX_SIZEOFBLENDERHEADER || bytes[9] != b'-' {
        return HeaderVariant::Unknown;
    }
    if digits(&bytes[10..12]) != Some(BLEND_FILE_FORMAT_VERSION_1) || bytes[12] != b'v' {
        return HeaderVariant::Unknown;
    }
    match digits(&bytes[13..17]) {
        Some(file_version) => HeaderVariant::Header(BlenderHeader {
            pointer_size: 8,
            endian: Endian::Little,
            file_version,
            file_format_version: BLEND_FILE_FORMAT_VERSION_1,
        }),
        None => HeaderVariant::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(v: HeaderVariant) -> BlenderHeader {
        match v {
            HeaderVariant::Header(h) => h,
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn legacy_8_byte_little() {
        let h = hdr(decode_blender_header(b"BLENDER-v305"));
        assert_eq!((h.pointer_size, h.endian, h.file_version, h.file_format_version), (8, Endian::Little, 305, 0));
        assert_eq!(h.bhead_type(), BHeadType::SmallBHead8);
    }

    #[test]
    fn legacy_4_byte_big() {
        let h = hdr(decode_blender_header(b"BLENDER_V280"));
        assert_eq!((h.pointer_size, h.endian, h.file_version), (4, Endian::Big, 280));
        assert_eq!(h.bhead_type(), BHeadType::BHead4);
    }

    #[test]
    fn version_1_header() {
        let h = hdr(decode_blender_header(b"BLENDER17-01v0405"));
        assert_eq!((h.pointer_size, h.endian, h.file_version, h.file_format_version), (8, Endian::Little, 405, 1));
        assert_eq!(h.bhead_type(), BHeadType::LargeBHead8);
    }

    #[test]
    fn invalid_files() {
        assert_eq!(decode_blender_header(b"BLENDER"), HeaderVariant::Invalid);
        assert_eq!(decode_blender_header(b"NOTBLENDER12"), HeaderVariant::Invalid);
    }

    #[test]
    fn unknown_variants() {
        for s in [&b"BLENDER-x305"[..], b"BLENDER-v3a5", b"BLENDER18-01v0405", b"BLENDER17-02v0405",
                  b"BLENDER17+01v0405", b"BLENDER17-01x0405", b"BLENDER17-01v04x5", b"BLENDER17-01v04", b"BLENDERab-01v0405"] {
            assert_eq!(decode_blender_header(s), HeaderVariant::Unknown, "{:?}", s);
        }
    }
}
