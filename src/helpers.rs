/// The order of the Goldilocks field, `0xffffffff00000001`.
pub(crate) const MODULUS: u64 = 0xffffffff00000001u64;

const EPSILON: u64 = (1 << 32) - 1;

/// The quadratic non-residue used to build the extension: `X^2 = QUADRATIC_NON_RESIDUE` in the base
/// field.
pub(crate) const QUADRATIC_NON_RESIDUE: u64 = 7;

/// Goldilocks addition.
#[inline]
pub(crate) const fn gl_add(lhs: u64, rhs: u64) -> u64 {
    let (sum, overflow) = lhs.overflowing_add(rhs);
    let sum = if overflow { sum + EPSILON } else { sum };
    if sum < MODULUS { sum } else { sum - MODULUS }
}

/// Goldilocks subtraction.
#[inline]
pub(crate) const fn gl_sub(lhs: u64, rhs: u64) -> u64 {
    if rhs > lhs {
        MODULUS - rhs + lhs
    } else {
        lhs - rhs
    }
}

/// Goldilocks multiplication.
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

/// Multiplies `a * X + b` by `c * X + d` in the quadratic extension
/// `GF(p)[X] / (X^2 - QUADRATIC_NON_RESIDUE)` of the Goldilocks field, returning the `(hi, lo)`
/// coefficients of the product.
#[inline]
pub(crate) const fn gl_mul2(a: u64, b: u64, c: u64, d: u64) -> (u64, u64) {
    let hi = gl_add(gl_mul(a, d), gl_mul(b, c));
    let lo = gl_add(gl_mul(b, d), gl_mul(QUADRATIC_NON_RESIDUE, gl_mul(a, c)));
    (hi, lo)
}
