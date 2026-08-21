use crate::base;
use crate::gl2;
use crate::helpers::{MODULUS, gl_add};
use std::ops::{Add, AddAssign};
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

// TODO

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
}
