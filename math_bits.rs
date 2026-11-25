//! Port of `source/blender/blenlib/intern/math_bits_inline.cc` and `BLI_math_bits.hh`
//! (`bitscan_reverse_uint`, `bitscan_reverse_clear_uint`, `count_bits_i`, `count_bits_uint64`).
//! Tests are the `BitscanReverseClearUint` / `PopCount` cases of `tests/BLI_math_bits_test.cc`.

/// `bitscan_reverse_uint()`: index of the highest set bit counted from the MSB
/// (the number of leading zeros). `a` must not be 0.
pub fn bitscan_reverse_uint(a: u32) -> u32 {
    debug_assert!(a != 0);
    a.leading_zeros()
}

/// `bitscan_reverse_clear_uint()`: like `bitscan_reverse_uint()`, and clears that bit.
pub fn bitscan_reverse_clear_uint(a: &mut u32) -> u32 {
    let i = bitscan_reverse_uint(*a);
    *a &= !(0x8000_0000u32 >> i);
    i
}

/// `count_bits_i()`: number of set bits (`__builtin_popcount`).
pub fn count_bits_i(i: u32) -> u32 {
    i.count_ones()
}

/// `count_bits_uint64()`: number of set bits (`__builtin_popcountll`).
pub fn count_bits_uint64(i: u64) -> u32 {
    i.count_ones()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_count_cases() {
        assert_eq!(count_bits_i(0), 0);
        assert_eq!(count_bits_uint64(0), 0);

        let value: u32 = (1 << 0) | (1 << 5) | (1 << 7);
        assert_eq!(count_bits_i(value), 3);
        assert_eq!(count_bits_uint64(value as u64), 3);

        assert_eq!(count_bits_uint64((1u64 << 0) | (1u64 << 50)), 2);
        assert_eq!(count_bits_i(-1i32 as u32), 32);
        assert_eq!(count_bits_uint64(u64::MAX), 64);
    }

    #[test]
    fn bitscan_reverse_clear_uint_cases() {
        let mut a = 1234u32;
        for (index, rest) in [(21, 210), (24, 82), (25, 18), (27, 2), (30, 0)] {
            assert_eq!(bitscan_reverse_clear_uint(&mut a), index);
            assert_eq!(a, rest);
        }

        let mut a = 3563987529u32;
        let expected = [
            (0, 1416503881u32),
            (1, 342762057),
            (3, 74326601),
            (5, 7217737),
            (9, 3023433),
            (10, 926281),
            (12, 401993),
            (13, 139849),
            (14, 8777),
            (18, 585),
            (22, 73),
            (25, 9),
            (28, 1),
            (31, 0),
        ];
        for (index, rest) in expected {
            assert_eq!(bitscan_reverse_clear_uint(&mut a), index);
            assert_eq!(a, rest);
        }
    }
}
