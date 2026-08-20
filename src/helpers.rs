/// The order of the Goldilocks field, `0xffffffff00000001`.
pub(crate) const MODULUS: u64 = 0xffffffff00000001u64;

const EPSILON: u64 = (1 << 32) - 1;

#[inline]
pub(crate) const fn gl_add(lhs: u64, rhs: u64) -> u64 {
    let (sum, overflow) = lhs.overflowing_add(rhs);
    let sum = if overflow { sum + EPSILON } else { sum };
    if sum < MODULUS { sum } else { sum - MODULUS }
}

#[inline]
pub(crate) const fn gl_sub(lhs: u64, rhs: u64) -> u64 {
    if rhs > lhs {
        MODULUS - rhs + lhs
    } else {
        lhs - rhs
    }
}

#[inline]
pub(crate) const fn gl_mul(lhs: u64, rhs: u64) -> u64 {
    let wide = (lhs as u128) * (rhs as u128);
    let lo = wide as u64;
    let hi = (wide >> 64) as u64;
    let hi_hi = hi >> 32;
    let hi_lo = hi & EPSILON;

    let (t0, borrow) = lo.overflowing_sub(hi_hi);
    let t0 = if borrow { t0.wrapping_sub(EPSILON) } else { t0 };

    let t1 = hi_lo * EPSILON;

    let (t2, overflow) = t0.overflowing_add(t1);
    let t2 = if overflow { t2 + EPSILON } else { t2 };
    if t2 < MODULUS { t2 } else { t2 - MODULUS }
}
