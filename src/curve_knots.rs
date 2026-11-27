//! NURBS knot vectors: `calcknots()` and the sizes used by `makeknots()` (curve.cc, `KNOTSU`).

/// `Nurb::flagu` bits (DNA_curve_types.h).
pub const CU_NURB_CYCLIC: i16 = 1 << 0;
pub const CU_NURB_ENDPOINT: i16 = 1 << 1;
pub const CU_NURB_BEZIER: i16 = 1 << 2;

/// `KNOTSU()`: the number of knots of `pnts` control points of `order`.
pub fn knot_count(pnts: usize, order: usize, flag: i16) -> usize {
    pnts + order + if flag & CU_NURB_CYCLIC != 0 { order.saturating_sub(1) } else { 0 }
}

/// `calcknots()`. Needs a valid curve (`order` at least 2 and at most `pnts`).
pub fn calc_knots(pnts: usize, order: usize, flag: i16) -> Vec<f32> {
    let is_cyclic = flag & CU_NURB_CYCLIC != 0;
    let is_bezier = flag & CU_NURB_BEZIER != 0;
    let is_end_point = flag & CU_NURB_ENDPOINT != 0;
    /* Inner knots are always repeated once except on Bezier case. */
    let repeat_inner = if is_bezier { order - 1 } else { 1 };
    /* How many times to repeat 0.0 at the beginning of knot. */
    let head = if is_end_point {
        order - usize::from(is_cyclic)
    } else if is_bezier {
        repeat_inner.min(2)
    } else {
        1
    };
    /* Knots replicating the widths of the starting knots (cyclic and endpoint cases). */
    let tail = if is_cyclic {
        2 * order - 1
    } else if is_end_point {
        order
    } else {
        0
    };
    let count = knot_count(pnts, order, flag);
    let mut knots = vec![0.0f32; count];
    let mut r = head;
    let mut current = 0.0f32;

    let offset = usize::from(is_end_point && is_cyclic);
    if offset == 1 {
        current += 1.0;
    }
    for k in &mut knots[offset..count - tail] {
        *k = current;
        r -= 1;
        if r == 0 {
            current += 1.0;
            r = repeat_inner;
        }
    }
    let tail_index = count - tail;
    for i in 0..tail {
        knots[tail_index + i] = current + (knots[i] - knots[0]);
    }
    knots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (pnts, order, flag, knots) as printed by the C `calcknots()` compiled on its own.
    #[test]
    fn matches_the_c_function_for_each_flag_combination() {
        let cases: &[(usize, usize, i16, &[f32])] = &[
            (5, 4, 0, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            (5, 4, 2, &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]),
            (7, 4, 4, &[0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0]),
            (4, 3, 1, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            (4, 3, 3, &[0.0, 1.0, 1.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0]),
            (6, 3, 4, &[0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0]),
            (4, 4, 5, &[0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0]),
            (5, 4, 6, &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            (6, 2, 4, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
        ];
        for &(pnts, order, flag, want) in cases {
            assert_eq!(calc_knots(pnts, order, flag), want, "{pnts} {order} {flag}");
        }
    }
}
