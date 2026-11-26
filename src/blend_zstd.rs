//! Decompress a zstd `.blend` into memory. Blender writes a sequence of zstd frames followed by a
//! skippable frame holding the seek table (`zstd_read_seek_table()`, filereader_zstd.cc); the whole
//! file is read here, so every frame is decoded in order and skippable frames are passed over.
//! Detection is `BLI_file_magic_is_zstd()` (fileops_c.cc). Decoding uses the `ruzstd` crate.

use std::io::Read;

use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
use ruzstd::decoding::StreamingDecoder;

const ERR: &str = "damaged compressed .blend file";

/// `BLI_file_magic_is_zstd()`: a zstd frame or a skippable frame.
pub fn is_zstd(bytes: &[u8]) -> bool {
    let Some(head) = bytes.get(..4).and_then(|h| h.try_into().ok()) else { return false };
    let magic = u32::from_le_bytes(head);
    magic == 0xFD2F_B528 || (magic >> 4) == 0x184D_2A5
}

pub fn decompress(mut input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::new();
    while !input.is_empty() {
        match StreamingDecoder::new(&mut input) {
            Ok(mut frame) => {
                frame.read_to_end(&mut out).map_err(|_| ERR)?;
            }
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame { length, .. })) => {
                input = input.get(length as usize..).ok_or(ERR)?;
            }
            Err(_) => return Err(ERR),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruzstd::encoding::{compress_to_vec, CompressionLevel};

    fn frame(data: &[u8]) -> Vec<u8> {
        compress_to_vec(data, CompressionLevel::Fastest)
    }

    #[test]
    fn decodes_all_frames_and_skips_skippable_ones() {
        let mut input = frame(b"BLENDER first ");
        input.extend(frame(b"second"));
        input.extend(0x184D_2A5Eu32.to_le_bytes());
        input.extend(3u32.to_le_bytes());
        input.extend([1, 2, 3]);
        assert!(is_zstd(&input));
        assert_eq!(decompress(&input).unwrap(), b"BLENDER first second");
    }

    #[test]
    fn damaged_input_is_an_error() {
        let good = frame(&[7; 1000]);
        assert_eq!(decompress(&good[..good.len() - 4]), Err(ERR));
        let mut short_skip = 0x184D_2A50u32.to_le_bytes().to_vec();
        short_skip.extend(9u32.to_le_bytes());
        assert_eq!(decompress(&short_skip), Err(ERR));
        let mut trailing = good.clone();
        trailing.extend(b"junk");
        assert_eq!(decompress(&trailing), Err(ERR));
    }
}
