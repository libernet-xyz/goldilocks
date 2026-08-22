use crate::base;
use crate::gl2;
use crate::helpers::{MODULUS, QUADRATIC_NON_RESIDUE, gl_add, gl_mul, gl_mul2, gl_sub};
use anyhow::anyhow;
use primitive_types::{H512, U256, U512};
use starkom_ff::{Field, Field256};
use std::fmt::{Binary, Debug, Display, Formatter, LowerHex, Octal, UpperHex};
use std::iter::{Product, Sum};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;
use subtle::{
    Choice, ConditionallySelectable, ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess,
    CtOption,
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
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

impl Div<Self> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: Self) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl<'a> Div<&'a Self> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: &'a Self) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl DivAssign<Self> for Scalar {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl<'a> DivAssign<&'a Self> for Scalar {
    fn div_assign(&mut self, rhs: &'a Self) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl Div<base::Scalar> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: base::Scalar) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl<'a> Div<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: &'a base::Scalar) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl DivAssign<base::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: base::Scalar) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl<'a> DivAssign<&'a base::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: &'a base::Scalar) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl Div<gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: gl2::Scalar) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl<'a> Div<&'a gl2::Scalar> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: &'a gl2::Scalar) -> Self::Output {
        self * rhs.invert_unwrap()
    }
}

impl DivAssign<gl2::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: gl2::Scalar) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl<'a> DivAssign<&'a gl2::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: &'a gl2::Scalar) {
        *self = *self * rhs.invert_unwrap();
    }
}

impl Sum<Scalar> for Scalar {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |a, b| a + b)
    }
}

impl<'a> Sum<&'a Scalar> for Scalar {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |a, b| a + b)
    }
}

impl Product<Scalar> for Scalar {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, |a, b| a * b)
    }
}

impl<'a> Product<&'a Scalar> for Scalar {
    fn product<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, |a, b| a * b)
    }
}

impl Debug for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scalar({:#066x})", self)
    }
}

impl Display for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#066x}", self)
    }
}

impl Binary for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if f.alternate() { "0b" } else { "" };
        f.pad_integral(true, prefix, &self.to_str_radix(2, 0, false))
    }
}

impl Octal for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if f.alternate() { "0o" } else { "" };
        f.pad_integral(true, prefix, &self.to_str_radix(8, 0, false))
    }
}

impl LowerHex for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        f.pad_integral(true, prefix, &self.to_str_radix(16, 0, false))
    }
}

impl UpperHex for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        f.pad_integral(true, prefix, &self.to_str_radix(16, 0, true))
    }
}

impl FromStr for Scalar {
    type Err = std::fmt::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("0x") || s.starts_with("0X") {
            Self::from_str_radix(&s[2..], 16)
        } else if s.starts_with("0b") || s.starts_with("0B") {
            Self::from_str_radix(&s[2..], 2)
        } else if s.starts_with("0o") || s.starts_with("0O") {
            Self::from_str_radix(&s[2..], 8)
        } else if s.starts_with("0") {
            Self::from_str_radix(s, 8)
        } else {
            Self::from_str_radix(s, 10)
        }
    }
}

impl From<u8> for Scalar {
    fn from(value: u8) -> Self {
        Self(0, 0, 0, value as u64)
    }
}

impl From<u16> for Scalar {
    fn from(value: u16) -> Self {
        Self(0, 0, 0, value as u64)
    }
}

impl From<u32> for Scalar {
    fn from(value: u32) -> Self {
        Self(0, 0, 0, value as u64)
    }
}

impl From<u64> for Scalar {
    fn from(value: u64) -> Self {
        Self(0, 0, value / MODULUS, value % MODULUS)
    }
}

impl From<u128> for Scalar {
    fn from(value: u128) -> Self {
        const MODULUS_U128: u128 = MODULUS as u128;
        let d0 = value % MODULUS_U128;
        let value = value / MODULUS_U128;
        let d1 = value % MODULUS_U128;
        let value = value / MODULUS_U128;
        let d2 = value % MODULUS_U128;
        Self(0, d2 as u64, d1 as u64, d0 as u64)
    }
}

impl From<base::Scalar> for Scalar {
    fn from(value: base::Scalar) -> Self {
        Self(0, 0, 0, value.0)
    }
}

impl From<gl2::Scalar> for Scalar {
    fn from(value: gl2::Scalar) -> Self {
        Self(0, 0, value.0, value.1)
    }
}

impl TryFrom<usize> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let value = value as u64;
        Ok(Self(0, 0, value / MODULUS, value % MODULUS))
    }
}

impl TryFrom<U256> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        let modulus = U256::from(MODULUS);
        let mut remaining = value;
        let d0 = remaining % modulus;
        remaining /= modulus;
        let d1 = remaining % modulus;
        remaining /= modulus;
        let d2 = remaining % modulus;
        remaining /= modulus;
        let d3 = remaining % modulus;
        remaining /= modulus;
        if remaining != U256::zero() {
            return Err(anyhow!("{:#x} exceeds the Goldilocks^4 range", value));
        }
        Ok(Self(d3.as_u64(), d2.as_u64(), d1.as_u64(), d0.as_u64()))
    }
}

impl Field for Scalar {
    const LEN: usize = 32;

    const ZERO: Self = Self(0, 0, 0, 0);

    const ONE: Self = Self(0, 0, 0, 1);

    const MAX: Self = Self(MODULUS - 1, MODULUS - 1, MODULUS - 1, MODULUS - 1);

    fn is_odd(&self) -> Choice {
        todo!()
    }

    fn try_random<R: rand_core::TryCryptoRng>(rng: &mut R) -> Result<Self, R::Error> {
        todo!()
    }

    fn random<R: rand_core::CryptoRng>(rng: &mut R) -> Self {
        todo!()
    }

    fn random_default() -> Self {
        todo!()
    }

    fn invert(&self) -> subtle::CtOption<Self> {
        let a = gl2::Scalar(self.0, self.1);
        let b = gl2::Scalar(self.2, self.3);
        let a_squared = a * a;
        // `a_squared * X`, using the same swap-and-scale trick as `A*C*X` in `mul_impl`.
        let a_squared_x = gl2::Scalar(a_squared.1, gl_mul(QUADRATIC_NON_RESIDUE, a_squared.0));
        let norm = b * b - a_squared_x;
        let conjugate = Self(gl_sub(0, self.0), gl_sub(0, self.1), self.2, self.3);
        norm.invert().map(|inverse_norm| conjugate * inverse_norm)
    }

    fn invert_vartime(&self) -> Option<Self> {
        let a = gl2::Scalar(self.0, self.1);
        let b = gl2::Scalar(self.2, self.3);
        let a_squared = a * a;
        let a_squared_x = gl2::Scalar(a_squared.1, gl_mul(QUADRATIC_NON_RESIDUE, a_squared.0));
        let norm = b * b - a_squared_x;
        let conjugate = Self(gl_sub(0, self.0), gl_sub(0, self.1), self.2, self.3);
        norm.invert_vartime()
            .map(|inverse_norm| conjugate * inverse_norm)
    }

    fn pow(self, exp: Self) -> Self {
        todo!()
    }

    fn pow_vartime(self, exp: Self) -> Self {
        todo!()
    }

    fn div_int(&self, rhs: &Self) -> (Self, Self) {
        todo!()
    }

    fn try_from_le_bytes(bytes: &[u8]) -> CtOption<Self> {
        todo!()
    }

    fn try_from_be_bytes(bytes: &[u8]) -> CtOption<Self> {
        todo!()
    }

    fn from_str_radix(s: &str, radix: usize) -> Result<Self, std::fmt::Error> {
        assert!(radix >= 2 && radix <= 36);
        if s.is_empty() {
            return Err(std::fmt::Error);
        }
        let mut value = U256::zero();
        let radix_u256 = U256::from(radix);
        for byte in s.bytes() {
            let digit = CHARACTERS_UPPER_CASE[..radix]
                .iter()
                .position(|&c| c == byte)
                .or_else(|| {
                    CHARACTERS_LOWER_CASE[..radix]
                        .iter()
                        .position(|&c| c == byte)
                })
                .ok_or(std::fmt::Error)?;
            value = value
                .checked_mul(radix_u256)
                .ok_or(std::fmt::Error)?
                .checked_add(U256::from(digit))
                .ok_or(std::fmt::Error)?;
        }
        Self::try_from(value).map_err(|_| std::fmt::Error)
    }

    fn to_str_radix(&self, radix: usize, pad_to: usize, upper_case: bool) -> String {
        assert!(radix >= 2 && radix <= 36);
        let characters = if upper_case {
            CHARACTERS_UPPER_CASE
        } else {
            CHARACTERS_LOWER_CASE
        };
        let mut value = self.to_u256();
        let mut s = String::default();
        let radix = U256::from(radix);
        while value != U256::zero() {
            let digit = value % radix;
            s.push(characters[digit.as_u64() as usize] as char);
            value /= radix;
        }
        if s.is_empty() {
            s.push('0');
        }
        while s.len() < pad_to {
            s.push('0');
        }
        s.chars().rev().collect()
    }

    fn try_to_u8(&self) -> Option<u8> {
        todo!()
    }

    fn try_to_u16(&self) -> Option<u16> {
        todo!()
    }
}

impl Field256 for Scalar {
    fn to_le_bytes(&self) -> [u8; 32] {
        todo!()
    }

    fn to_be_bytes(&self) -> [u8; 32] {
        todo!()
    }

    fn from_u512_mod_n(u512: U512) -> Self {
        todo!()
    }

    fn from_h512(h512: H512) -> Self {
        todo!()
    }

    fn try_to_u32(&self) -> CtOption<u32> {
        todo!()
    }

    fn try_to_u64(&self) -> CtOption<u64> {
        todo!()
    }

    fn try_to_u128(&self) -> CtOption<u128> {
        todo!()
    }

    fn to_u256(&self) -> U256 {
        let modulus = U256::from(MODULUS);
        U256::from(self.0) * modulus * modulus * modulus
            + U256::from(self.1) * modulus * modulus
            + U256::from(self.2) * modulus
            + U256::from(self.3)
    }

    fn to_u512(&self) -> U512 {
        self.to_u256().into()
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

    #[inline]
    fn parse_scalar(s: &'static str) -> Scalar {
        s.parse().unwrap()
    }

    #[test]
    fn test_from_const() {
        assert_eq!(from_const(0), Scalar::ZERO);
        assert_eq!(from_const(1), Scalar::ONE);
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
        assert_eq!(-Scalar::ZERO, Scalar::ZERO);
        assert_eq!(
            -Scalar(1, 2, 3, 4),
            Scalar(MODULUS - 1, MODULUS - 2, MODULUS - 3, MODULUS - 4)
        );
        assert_eq!(Scalar(1, 2, 3, 4) + -Scalar(1, 2, 3, 4), Scalar::ZERO);
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
        assert_eq!(Scalar::ZERO * Scalar(1, 2, 3, 4), Scalar::ZERO);
        assert_eq!(Scalar(1, 2, 3, 4) * Scalar::ZERO, Scalar::ZERO);
    }

    #[test]
    fn test_mul_by_one() {
        assert_eq!(Scalar::ONE * Scalar(2, 3, 4, 5), Scalar(2, 3, 4, 5));
        assert_eq!(Scalar(2, 3, 4, 5) * Scalar::ONE, Scalar(2, 3, 4, 5));
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

    #[test]
    fn test_div_by_one() {
        assert_eq!(Scalar(1, 2, 3, 4) / Scalar::ONE, Scalar(1, 2, 3, 4));
        assert_eq!(Scalar(1, 2, 3, 4) / &Scalar::ONE, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div() {
        let lhs = Scalar(1, 2, 3, 4);
        let rhs = Scalar(5, 6, 7, 8);
        assert_eq!((lhs * rhs) / rhs, lhs);
        assert_eq!((lhs * rhs) / &rhs, lhs);
    }

    #[test]
    fn test_div_assign() {
        let rhs = Scalar(5, 6, 7, 8);
        let mut lhs = Scalar(1, 2, 3, 4) * rhs;
        lhs /= rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div_assign_ref() {
        let rhs = Scalar(5, 6, 7, 8);
        let mut lhs = Scalar(1, 2, 3, 4) * rhs;
        lhs /= &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div_base_scalar() {
        let lhs = Scalar(5, 10, 15, 20);
        let rhs = base::Scalar(5);
        assert_eq!(lhs / rhs, Scalar(1, 2, 3, 4));
        assert_eq!(lhs / &rhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(5, 10, 15, 20);
        lhs /= rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));

        let mut lhs = Scalar(5, 10, 15, 20);
        lhs /= &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div_gl2() {
        let lhs = Scalar(16, 47, 38, 129);
        let rhs = gl2::Scalar(5, 6);
        assert_eq!(lhs / rhs, Scalar(1, 2, 3, 4));
        assert_eq!(lhs / &rhs, Scalar(1, 2, 3, 4));
    }

    #[test]
    fn test_div_assign_gl2() {
        let rhs = gl2::Scalar(5, 6);

        let mut lhs = Scalar(16, 47, 38, 129);
        lhs /= rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));

        let mut lhs = Scalar(16, 47, 38, 129);
        lhs /= &rhs;
        assert_eq!(lhs, Scalar(1, 2, 3, 4));
    }

    fn test_inversion_impl(value: Scalar) {
        assert_ne!(value, Scalar::ZERO);
        assert_eq!(value * value.invert().unwrap(), Scalar::ONE);
        assert_eq!(value * value.invert_unwrap(), Scalar::ONE);
        assert_eq!(value * value.invert_or_zero(), Scalar::ONE);
        assert_eq!(value * value.invert_vartime().unwrap(), Scalar::ONE);
    }

    #[test]
    fn test_inversion() {
        assert!(Scalar::ZERO.invert_vartime().is_none());
        assert_eq!(Scalar::ZERO.invert_or_zero(), Scalar::ZERO);
        assert!(bool::from(Scalar::ZERO.invert().is_none()));
        test_inversion_impl(Scalar::ONE);
        test_inversion_impl(Scalar(0, 0, 0, 42));
        test_inversion_impl(Scalar(1, 0, 0, 0));
        test_inversion_impl(Scalar(0, 0, 1, 0));
        test_inversion_impl(Scalar(7, 11, 13, 17));
        test_inversion_impl(Scalar::MAX);
    }

    #[test]
    fn test_invert_batch() {
        let values = vec![
            Scalar(1, 2, 3, 4),
            Scalar(0, 0, 0, 42),
            Scalar::ONE,
            Scalar(3, 5, 7, 9),
        ];
        let expected: Vec<Scalar> = values
            .iter()
            .map(|value| value.invert_vartime().unwrap())
            .collect();

        let mut batch = values.clone();
        Scalar::invert_batch(&mut batch);
        assert_eq!(batch, expected);

        let mut batch = values;
        Scalar::invert_batch_vartime(&mut batch);
        assert_eq!(batch, expected);
    }

    #[test]
    fn test_sum() {
        let values = vec![Scalar(1, 2, 3, 4), Scalar(5, 6, 7, 8), Scalar(1, 1, 1, 1)];
        assert_eq!(values.iter().sum::<Scalar>(), Scalar(7, 9, 11, 13));
        assert_eq!(values.into_iter().sum::<Scalar>(), Scalar(7, 9, 11, 13));
    }

    #[test]
    fn test_product() {
        let values = vec![Scalar(0, 0, 0, 2), Scalar(0, 0, 0, 3), Scalar(0, 0, 0, 4)];
        assert_eq!(values.iter().product::<Scalar>(), Scalar(0, 0, 0, 24));
        assert_eq!(values.into_iter().product::<Scalar>(), Scalar(0, 0, 0, 24));
    }

    #[test]
    fn test_fmt_display() {
        assert_eq!(
            format!("{}", from_const(0)),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            format!("{}", from_const(0xdeadbeef)),
            "0x00000000000000000000000000000000000000000000000000000000deadbeef"
        );
        assert_eq!(
            format!("{}", Scalar(1, 2, 3, 4)),
            "0x0000000000000000fffffffd00000007fffffff50000000efffffff60000000a"
        );
    }

    #[test]
    fn test_fmt_debug() {
        assert_eq!(
            format!("{:?}", from_const(0)),
            "Scalar(0x0000000000000000000000000000000000000000000000000000000000000000)"
        );
    }

    #[test]
    fn test_fmt_lower_hex() {
        assert_eq!(format!("{:x}", from_const(0xdeadbeef)), "deadbeef");
        assert_eq!(format!("{:#x}", from_const(0xdeadbeef)), "0xdeadbeef");
        assert_eq!(
            format!("{:x}", Scalar(1, 2, 3, 4)),
            "fffffffd00000007fffffff50000000efffffff60000000a"
        );
    }

    #[test]
    fn test_fmt_upper_hex() {
        assert_eq!(format!("{:X}", from_const(0xdeadbeef)), "DEADBEEF");
    }

    #[test]
    fn test_fmt_binary() {
        assert_eq!(format!("{:b}", from_const(0b1010)), "1010");
    }

    #[test]
    fn test_fmt_octal() {
        assert_eq!(format!("{:o}", from_const(0o755)), "755");
    }

    #[test]
    fn test_from_str() {
        assert_eq!("0".parse::<Scalar>().unwrap(), Scalar::ZERO);
        assert_eq!("42".parse::<Scalar>().unwrap(), from_const(42));
        assert_eq!("0x2a".parse::<Scalar>().unwrap(), from_const(42));
        assert_eq!("0b101010".parse::<Scalar>().unwrap(), from_const(42));
        assert_eq!("0o52".parse::<Scalar>().unwrap(), from_const(42));
    }

    #[test]
    fn test_from_str_invalid() {
        assert!("".parse::<Scalar>().is_err());
        assert!("not a number".parse::<Scalar>().is_err());
        // MODULUS^4, i.e. one past the largest representable value.
        assert!(
            "115792089129476408817739443160502628952720274482139873392618675794070921543681"
                .parse::<Scalar>()
                .is_err()
        );
    }

    #[test]
    fn test_parse_scalar() {
        assert_eq!(parse_scalar("0x2a"), from_const(42));
    }

    #[test]
    fn test_display_from_str_roundtrip() {
        let value = Scalar(1, 2, 3, 4);
        assert_eq!(format!("{}", value).parse::<Scalar>().unwrap(), value);
        assert_eq!(
            format!("{}", Scalar::MAX).parse::<Scalar>().unwrap(),
            Scalar::MAX
        );
    }

    #[test]
    fn test_from_u8() {
        assert_eq!(Scalar::from(0u8), from_const(0));
        assert_eq!(Scalar::from(u8::MAX), from_const(u8::MAX as u64));
    }

    #[test]
    fn test_from_u16() {
        assert_eq!(Scalar::from(0u16), from_const(0));
        assert_eq!(Scalar::from(u16::MAX), from_const(u16::MAX as u64));
    }

    #[test]
    fn test_from_u32() {
        assert_eq!(Scalar::from(0u32), from_const(0));
        assert_eq!(Scalar::from(u32::MAX), from_const(u32::MAX as u64));
    }

    #[test]
    fn test_from_u64() {
        assert_eq!(Scalar::from(0u64), from_const(0));
        assert_eq!(Scalar::from(u64::MAX), from_const(u64::MAX));
        assert_eq!(Scalar::from(MODULUS), Scalar(0, 0, 1, 0));
    }

    #[test]
    fn test_from_u128() {
        assert_eq!(Scalar::from(0u128), from_const(0));
        assert_eq!(Scalar::from(42u128), from_const(42));
        // MODULUS^2, i.e. the smallest value needing all three lower words.
        let modulus = MODULUS as u128;
        assert_eq!(Scalar::from(modulus * modulus), Scalar(0, 1, 0, 0));
        assert_eq!(
            Scalar::from(u128::MAX),
            Scalar(0, 1, 8589934590, 18446744065119617024)
        );
    }

    #[test]
    fn test_try_from_usize() {
        assert_eq!(Scalar::try_from(0usize).unwrap(), from_const(0));
        assert_eq!(Scalar::try_from(42usize).unwrap(), from_const(42));
    }

    #[test]
    fn test_try_from_u256() {
        assert_eq!(Scalar::try_from(U256::from(0)).unwrap(), from_const(0));
        assert_eq!(Scalar::try_from(U256::from(42)).unwrap(), from_const(42));

        let modulus = U256::from(MODULUS);
        let modulus_pow4 = modulus * modulus * modulus * modulus;
        assert_eq!(
            Scalar::try_from(modulus_pow4 - U256::from(1)).unwrap(),
            Scalar::MAX
        );
        assert!(Scalar::try_from(modulus_pow4).is_err());
        assert!(Scalar::try_from(U256::MAX).is_err());
    }
}
