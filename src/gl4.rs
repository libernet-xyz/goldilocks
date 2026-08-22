use crate::base;
use crate::gl2;
use crate::helpers::{MODULUS, QUADRATIC_NON_RESIDUE, gl_add, gl_mul, gl_mul2, gl_sub};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use subtle::{
    Choice, ConditionallySelectable, ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess,
};

/// Upper-case characters used in textual representations.
static CHARACTERS_UPPER_CASE: &'static [u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Lower-case characters used in textual representations.
static CHARACTERS_LOWER_CASE: &'static [u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Goldilocks^4 extension field.
///
/// This is the degree-2 extension `GL2[Y] / (Y^2 - X)` of [`gl2::Scalar`], where `X` is the GL2
/// generator (`X^2 = 7` in the base field). Equivalently, this is the degree-4 extension
/// `GF(p)[Y] / (Y^4 - 7)` of the Goldilocks field, where `p` is [`MODULUS`]. A scalar
/// `Scalar(w, x, y, z)` represents `GL2(w, x) * Y + GL2(y, z)`.
///
/// For all purposes other than field arithmetic (ordering, formatting, parsing, exponentiation,
/// etc.) a scalar is instead treated as the numeric value `w * MODULUS^3 + x * MODULUS^2 + y *
/// MODULUS + z`. This gives every scalar a canonical representative in `0..(MODULUS^4)` for those
/// purposes.
///
/// NOTE: The `u64` words are stored from most significant to least significant: `Scalar::0` is the
/// most significant and `Scalar::3` is the least significant. This way Rust's automatic comparison
/// trait implementations work as intended.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scalar(
    pub(crate) u64,
    pub(crate) u64,
    pub(crate) u64,
    pub(crate) u64,
);

impl Scalar {
    /// Constructs a Goldilocks^4 scalar from a raw 64-bit value.
    #[inline]
    pub const fn from_const(value: u64) -> Self {
        Self(0, 0, value / MODULUS, value % MODULUS)
    }

    /// Multiplies two GL4 scalars.
    ///
    /// `self` represents `A*Y + B` and `rhs` represents `C*Y + D`, where `A = (self.0, self.1)`,
    /// `B = (self.2, self.3)`, `C = (rhs.0, rhs.1)` and `D = (rhs.2, rhs.3)` are GL2 elements,
    /// using the `a*X + b` convention of [`gl2::Scalar`].
    ///
    /// `(A*Y+B) * (C*Y+D) = A*C*Y^2 + (A*D+B*C)*Y + B*D`, and since `Y^2 = X` (the GL2 generator),
    /// this reduces to `(A*D+B*C)*Y + (B*D + A*C*X)`. Multiplying a GL2 element `(p, q)` (i.e.
    /// `p*X+q`) by `X` gives `p*X^2 + q*X = q*X + QUADRATIC_NON_RESIDUE*p`, that is
    /// `(q, QUADRATIC_NON_RESIDUE*p)`, which is why `A*C*X` below is just a swap-and-scale of `A*C`
    /// rather than a full GL2 multiplication.
    ///
    /// Rather than computing `A*D`, `B*C`, `B*D` and `A*C` as 4 separate GL2 multiplications, this
    /// uses the same Karatsuba identity as [`gl_mul2`] one tower level up: `A*D + B*C =
    /// (A+B)*(C+D) - A*C - B*D`. Since `A*C` and `B*D` are needed on their own anyway (for `A*C*X`
    /// and for the constant term, respectively), this only requires computing `A*C`, `B*D` and
    /// `(A+B)*(C+D)` as GL2 multiplications, i.e. 3 instead of 4.
    fn mul_impl(self, rhs: Self) -> Self {
        let (a0, a1) = (self.0, self.1);
        let (b0, b1) = (self.2, self.3);
        let (c0, c1) = (rhs.0, rhs.1);
        let (d0, d1) = (rhs.2, rhs.3);

        // A*C
        let (ac0, ac1) = gl_mul2(a0, a1, c0, c1);

        // B*D
        let (bd0, bd1) = gl_mul2(b0, b1, d0, d1);

        // (A+B)*(C+D)
        let (s0, s1) = gl_mul2(
            gl_add(a0, b0),
            gl_add(a1, b1),
            gl_add(c0, d0),
            gl_add(c1, d1),
        );

        // A*D + B*C = (A+B)*(C+D) - A*C - B*D
        let ad_bc0 = gl_sub(gl_sub(s0, ac0), bd0);
        let ad_bc1 = gl_sub(gl_sub(s1, ac1), bd1);

        // A*C*X
        let acx0 = ac1;
        let acx1 = gl_mul(QUADRATIC_NON_RESIDUE, ac0);

        Self(ad_bc0, ad_bc1, gl_add(bd0, acx0), gl_add(bd1, acx1))
    }
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        (((self.0 == other.0) && (self.1 == other.1) && (self.2 == other.2) && (self.3 == other.3))
            as u8)
            .into()
    }
}

impl ConstantTimeGreater for Scalar {
    fn ct_gt(&self, other: &Self) -> Choice {
        (((self.0, self.1, self.2, self.3) > (other.0, other.1, other.2, other.3)) as u8).into()
    }
}

impl ConstantTimeLess for Scalar {}

impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        if choice.into() { *b } else { *a }
    }
}

impl Add<Self> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: Self) -> Self::Output {
        Self(
            gl_add(self.0, rhs.0),
            gl_add(self.1, rhs.1),
            gl_add(self.2, rhs.2),
            gl_add(self.3, rhs.3),
        )
    }
}

impl<'a> Add<&'a Self> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a Self) -> Self::Output {
        Self(
            gl_add(self.0, rhs.0),
            gl_add(self.1, rhs.1),
            gl_add(self.2, rhs.2),
            gl_add(self.3, rhs.3),
        )
    }
}

impl AddAssign<Self> for Scalar {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = gl_add(self.0, rhs.0);
        self.1 = gl_add(self.1, rhs.1);
        self.2 = gl_add(self.2, rhs.2);
        self.3 = gl_add(self.3, rhs.3);
    }
}

impl<'a> AddAssign<&'a Self> for Scalar {
    fn add_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_add(self.0, rhs.0);
        self.1 = gl_add(self.1, rhs.1);
        self.2 = gl_add(self.2, rhs.2);
        self.3 = gl_add(self.3, rhs.3);
    }
}

impl Add<base::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: base::Scalar) -> Self::Output {
        Self(self.0, self.1, self.2, gl_add(self.3, rhs.0))
    }
}

impl<'a> Add<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(self.0, self.1, self.2, gl_add(self.3, rhs.0))
    }
}

impl AddAssign<base::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: base::Scalar) {
        self.3 = gl_add(self.3, rhs.0);
    }
}

impl<'a> AddAssign<&'a base::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &'a base::Scalar) {
        self.3 = gl_add(self.3, rhs.0);
    }
}

impl Add<gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: gl2::Scalar) -> Self::Output {
        Self(self.0, self.1, gl_add(self.2, rhs.0), gl_add(self.3, rhs.1))
    }
}

impl<'a> Add<&'a gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a gl2::Scalar) -> Self::Output {
        Self(self.0, self.1, gl_add(self.2, rhs.0), gl_add(self.3, rhs.1))
    }
}

impl AddAssign<gl2::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: gl2::Scalar) {
        self.2 = gl_add(self.2, rhs.0);
        self.3 = gl_add(self.3, rhs.1);
    }
}

impl<'a> AddAssign<&'a gl2::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &'a gl2::Scalar) {
        self.2 = gl_add(self.2, rhs.0);
        self.3 = gl_add(self.3, rhs.1);
    }
}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Self::Output {
        Self(
            gl_sub(0, self.0),
            gl_sub(0, self.1),
            gl_sub(0, self.2),
            gl_sub(0, self.3),
        )
    }
}

impl Sub<Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(
            gl_sub(self.0, rhs.0),
            gl_sub(self.1, rhs.1),
            gl_sub(self.2, rhs.2),
            gl_sub(self.3, rhs.3),
        )
    }
}

impl<'a> Sub<&'a Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a Self) -> Self::Output {
        Self(
            gl_sub(self.0, rhs.0),
            gl_sub(self.1, rhs.1),
            gl_sub(self.2, rhs.2),
            gl_sub(self.3, rhs.3),
        )
    }
}

impl SubAssign<Self> for Scalar {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = gl_sub(self.0, rhs.0);
        self.1 = gl_sub(self.1, rhs.1);
        self.2 = gl_sub(self.2, rhs.2);
        self.3 = gl_sub(self.3, rhs.3);
    }
}

impl<'a> SubAssign<&'a Self> for Scalar {
    fn sub_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_sub(self.0, rhs.0);
        self.1 = gl_sub(self.1, rhs.1);
        self.2 = gl_sub(self.2, rhs.2);
        self.3 = gl_sub(self.3, rhs.3);
    }
}

impl Sub<base::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: base::Scalar) -> Self::Output {
        Self(self.0, self.1, self.2, gl_sub(self.3, rhs.0))
    }
}

impl<'a> Sub<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(self.0, self.1, self.2, gl_sub(self.3, rhs.0))
    }
}

impl SubAssign<base::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: base::Scalar) {
        self.3 = gl_sub(self.3, rhs.0);
    }
}

impl<'a> SubAssign<&'a base::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: &'a base::Scalar) {
        self.3 = gl_sub(self.3, rhs.0);
    }
}

impl Sub<gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: gl2::Scalar) -> Self::Output {
        Self(self.0, self.1, gl_sub(self.2, rhs.0), gl_sub(self.3, rhs.1))
    }
}

impl<'a> Sub<&'a gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a gl2::Scalar) -> Self::Output {
        Self(self.0, self.1, gl_sub(self.2, rhs.0), gl_sub(self.3, rhs.1))
    }
}

impl SubAssign<gl2::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: gl2::Scalar) {
        self.2 = gl_sub(self.2, rhs.0);
        self.3 = gl_sub(self.3, rhs.1);
    }
}

impl<'a> SubAssign<&'a gl2::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: &'a gl2::Scalar) {
        self.2 = gl_sub(self.2, rhs.0);
        self.3 = gl_sub(self.3, rhs.1);
    }
}

impl Mul<Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_impl(rhs)
    }
}

impl<'a> Mul<&'a Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a Self) -> Self::Output {
        self.mul_impl(*rhs)
    }
}

impl MulAssign<Self> for Scalar {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul_impl(rhs);
    }
}

impl<'a> MulAssign<&'a Self> for Scalar {
    fn mul_assign(&mut self, rhs: &'a Self) {
        *self = self.mul_impl(*rhs);
    }
}

impl Mul<base::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: base::Scalar) -> Self::Output {
        Self(
            gl_mul(self.0, rhs.0),
            gl_mul(self.1, rhs.0),
            gl_mul(self.2, rhs.0),
            gl_mul(self.3, rhs.0),
        )
    }
}

impl<'a> Mul<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(
            gl_mul(self.0, rhs.0),
            gl_mul(self.1, rhs.0),
            gl_mul(self.2, rhs.0),
            gl_mul(self.3, rhs.0),
        )
    }
}

impl MulAssign<base::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: base::Scalar) {
        self.0 = gl_mul(self.0, rhs.0);
        self.1 = gl_mul(self.1, rhs.0);
        self.2 = gl_mul(self.2, rhs.0);
        self.3 = gl_mul(self.3, rhs.0);
    }
}

impl<'a> MulAssign<&'a base::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &'a base::Scalar) {
        self.0 = gl_mul(self.0, rhs.0);
        self.1 = gl_mul(self.1, rhs.0);
        self.2 = gl_mul(self.2, rhs.0);
        self.3 = gl_mul(self.3, rhs.0);
    }
}

impl Mul<gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: gl2::Scalar) -> Self::Output {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.2, self.3, rhs.0, rhs.1);
        Self(y0, y1, c0, c1)
    }
}

impl<'a> Mul<&'a gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a gl2::Scalar) -> Self::Output {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.2, self.3, rhs.0, rhs.1);
        Self(y0, y1, c0, c1)
    }
}

impl MulAssign<gl2::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: gl2::Scalar) {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.2, self.3, rhs.0, rhs.1);
        self.0 = y0;
        self.1 = y1;
        self.2 = c0;
        self.3 = c1;
    }
}

impl<'a> MulAssign<&'a gl2::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &'a gl2::Scalar) {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.2, self.3, rhs.0, rhs.1);
        self.0 = y0;
        self.1 = y1;
        self.2 = c0;
        self.3 = c1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[inline]
    const fn from_const(value: u64) -> Scalar {
        Scalar::from_const(value)
    }

    #[test]
    fn test_from_const() {
        assert_eq!(from_const(0), Scalar(0, 0, 0, 0));
        assert_eq!(from_const(1), Scalar(0, 0, 0, 1));
        assert_eq!(from_const(MODULUS), Scalar(0, 0, 1, 0));
        assert_eq!(from_const(MODULUS + 1), Scalar(0, 0, 1, 1));
    }

    #[test]
    fn test_equality() {
        assert_eq!(Scalar(1, 2, 3, 4), Scalar(1, 2, 3, 4));
        assert_ne!(Scalar(1, 2, 3, 4), Scalar(1, 2, 3, 5));
    }

    #[test]
    fn test_total_order() {
        let v0 = Scalar(0, 0, 0, 0);
        let v1 = Scalar(0, 0, 0, 1);
        let v2 = Scalar(0, 0, 1, 0);
        let v3 = Scalar(0, 1, 0, 0);
        let v4 = Scalar(1, 0, 0, 0);

        assert_eq!(v0.cmp(&v0), Ordering::Equal);
        assert_eq!(v0.cmp(&v1), Ordering::Less);
        assert_eq!(v1.cmp(&v2), Ordering::Less);
        assert_eq!(v2.cmp(&v3), Ordering::Less);
        assert_eq!(v3.cmp(&v4), Ordering::Less);
        assert_eq!(v4.cmp(&v3), Ordering::Greater);
        assert_eq!(v4.cmp(&v4), Ordering::Equal);
    }

    #[test]
    fn test_ct_eq() {
        let a = Scalar(1, 2, 3, 4);
        let b = Scalar(1, 2, 3, 4);
        let c = Scalar(1, 2, 3, 5);
        let d = Scalar(9, 2, 3, 4);
        assert_eq!(bool::from(a.ct_eq(&b)), true);
        assert_eq!(bool::from(a.ct_eq(&c)), false);
        assert_eq!(bool::from(a.ct_eq(&d)), false);
    }

    #[test]
    fn test_ct_gt() {
        let v0 = Scalar(0, 0, 0, 0);
        let v1 = Scalar(0, 0, 0, 42);
        let v2 = Scalar(0, 0, 1, 0);
        assert_eq!(bool::from(v0.ct_gt(&v0)), false);
        assert_eq!(bool::from(v1.ct_gt(&v0)), true);
        assert_eq!(bool::from(v2.ct_gt(&v1)), true);
        assert_eq!(bool::from(v0.ct_gt(&v2)), false);
    }

    #[test]
    fn test_ct_lt() {
        let v0 = Scalar(0, 0, 0, 0);
        let v1 = Scalar(0, 0, 0, 42);
        let v2 = Scalar(0, 0, 1, 0);
        assert_eq!(bool::from(v0.ct_lt(&v1)), true);
        assert_eq!(bool::from(v1.ct_lt(&v2)), true);
        assert_eq!(bool::from(v2.ct_lt(&v0)), false);
    }

    #[test]
    fn test_conditional_select() {
        let a = Scalar(1, 2, 3, 4);
        let b = Scalar(5, 6, 7, 8);
        assert_eq!(Scalar::conditional_select(&a, &b, Choice::from(0)), a);
        assert_eq!(Scalar::conditional_select(&a, &b, Choice::from(1)), b);
    }

    #[test]
    fn test_add() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = Scalar(5, 6, 7, 8);
        assert_eq!(lhs + rhs, Scalar(6, 8, 10, 12));
        assert_eq!(lhs + &rhs, Scalar(6, 8, 10, 12));
    }

    #[test]
    fn test_add_wraparound() {
        let lhs = Scalar(MODULUS - 1, MODULUS - 2, MODULUS - 3, MODULUS - 4);
        let rhs = Scalar(2, 3, 4, 5);
        assert_eq!(lhs + rhs, Scalar(1, 1, 1, 1));
    }

    #[test]
    fn test_add_assign() {
        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(6, 8, 10, 12));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += &Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(6, 8, 10, 12));
    }

    #[test]
    fn test_add_base_scalar() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = base::Scalar(5);
        assert_eq!(lhs + rhs, Scalar(1, 2, 3, 9));
        assert_eq!(lhs + &rhs, Scalar(1, 2, 3, 9));
    }

    #[test]
    fn test_add_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 9));

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 9));
    }

    #[test]
    fn test_add_gl2() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = gl2::Scalar(5, 6);
        assert_eq!(lhs + rhs, Scalar(1, 2, 8, 10));
        assert_eq!(lhs + &rhs, Scalar(1, 2, 8, 10));
    }

    #[test]
    fn test_add_assign_gl2() {
        let rhs = gl2::Scalar(5, 6);

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += rhs;
        assert_eq!(lhs, Scalar(1, 2, 8, 10));

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs += &rhs;
        assert_eq!(lhs, Scalar(1, 2, 8, 10));
    }

    #[test]
    fn test_neg() {
        assert_eq!(-Scalar(0, 0, 0, 0), Scalar(0, 0, 0, 0));
        assert_eq!(
            -Scalar(1, 2, 3, 4),
            Scalar(MODULUS - 1, MODULUS - 2, MODULUS - 3, MODULUS - 4)
        );
        assert_eq!(Scalar(1, 2, 3, 4) + -Scalar(1, 2, 3, 4), Scalar(0, 0, 0, 0));
    }

    #[test]
    fn test_sub() {
        let lhs = Scalar(6, 8, 10, 12);
        let rhs = Scalar(5, 6, 7, 8);
        assert_eq!(lhs - rhs, Scalar(1, 2, 3, 4));
        assert_eq!(lhs - &rhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_wraparound() {
        let lhs = Scalar(1, 1, 1, 1);
        let rhs = Scalar(2, 3, 4, 5);
        assert_eq!(
            lhs - rhs,
            Scalar(MODULUS - 1, MODULUS - 2, MODULUS - 3, MODULUS - 4)
        );
    }

    #[test]
    fn test_sub_assign() {
        let mut lhs = Scalar(6, 8, 10, 12);
        lhs -= Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_assign_ref() {
        let mut lhs = Scalar(6, 8, 10, 12);
        lhs -= &Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_base_scalar() {
        let lhs = Scalar(1, 2, 3, 9);
        let rhs = base::Scalar(5);
        assert_eq!(lhs - rhs, Scalar(1, 2, 3, 4));
        assert_eq!(lhs - &rhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(1, 2, 3, 9);
        lhs -= rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));

        let mut lhs = Scalar(1, 2, 3, 9);
        lhs -= &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_gl2() {
        let lhs = Scalar(1, 2, 8, 10);
        let rhs = gl2::Scalar(5, 6);
        assert_eq!(lhs - rhs, Scalar(1, 2, 3, 4));
        assert_eq!(lhs - &rhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_sub_assign_gl2() {
        let rhs = gl2::Scalar(5, 6);

        let mut lhs = Scalar(1, 2, 8, 10);
        lhs -= rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));

        let mut lhs = Scalar(1, 2, 8, 10);
        lhs -= &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_extension_root() {
        let y = Scalar(0, 1, 0, 0);
        let y_squared = y * y;
        // Y^2 = X, i.e. the GL2 generator embedded with a zero Y-coefficient.
        assert_eq!(y_squared, Scalar(0, 0, 1, 0));
        assert_eq!(y * &y, y_squared);
        // Y^4 = QUADRATIC_NON_RESIDUE.
        assert_eq!(y_squared * y_squared, from_const(QUADRATIC_NON_RESIDUE));
    }

    #[test]
    fn test_mul_by_zero() {
        assert_eq!(Scalar(0, 0, 0, 0) * Scalar(1, 2, 3, 4), Scalar(0, 0, 0, 0));
        assert_eq!(Scalar(1, 2, 3, 4) * Scalar(0, 0, 0, 0), Scalar(0, 0, 0, 0));
    }

    #[test]
    fn test_mul_by_one() {
        assert_eq!(Scalar(0, 0, 0, 1) * Scalar(2, 3, 4, 5), Scalar(2, 3, 4, 5));
        assert_eq!(Scalar(2, 3, 4, 5) * Scalar(0, 0, 0, 1), Scalar(2, 3, 4, 5));
    }

    #[test]
    fn test_mul() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = Scalar(5, 6, 7, 8);
        let expected = Scalar(60, 194, 99, 291);
        assert_eq!(lhs * rhs, expected);
        assert_eq!(lhs * &rhs, expected);
        assert_eq!(rhs * lhs, expected);
    }

    #[test]
    fn test_mul_assign() {
        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(60, 194, 99, 291));
    }

    #[test]
    fn test_mul_assign_ref() {
        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= &Scalar(5, 6, 7, 8);
        assert_eq!(lhs, Scalar(60, 194, 99, 291));
    }

    #[test]
    fn test_mul_base_scalar() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = base::Scalar(5);
        assert_eq!(lhs * rhs, Scalar(5, 10, 15, 20));
        assert_eq!(lhs * &rhs, Scalar(5, 10, 15, 20));
    }

    #[test]
    fn test_mul_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= rhs;
        assert_eq!(lhs, Scalar(5, 10, 15, 20));

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= &rhs;
        assert_eq!(lhs, Scalar(5, 10, 15, 20));
    }

    #[test]
    fn test_mul_gl2() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = gl2::Scalar(5, 6);
        assert_eq!(lhs * rhs, Scalar(16, 47, 38, 129));
        assert_eq!(lhs * &rhs, Scalar(16, 47, 38, 129));
    }

    #[test]
    fn test_mul_assign_gl2() {
        let rhs = gl2::Scalar(5, 6);

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= rhs;
        assert_eq!(lhs, Scalar(16, 47, 38, 129));

        let mut lhs = Scalar(1, 2, 3, 4);
        lhs *= &rhs;
        assert_eq!(lhs, Scalar(16, 47, 38, 129));
    }
}
