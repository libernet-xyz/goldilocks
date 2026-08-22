use crate::base;
use crate::gl4;
use crate::helpers::{MODULUS, QUADRATIC_NON_RESIDUE, gl_add, gl_mul, gl_mul2, gl_sub};
use anyhow::anyhow;
use primitive_types::{H256, U256, U512};
use rand_core::{CryptoRng, TryCryptoRng};
use starkom_ff::{Field, Field128};
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

/// Goldilocks^2 extension field.
///
/// This is the degree-2 extension `GF(p)[X] / (X^2 - QUADRATIC_NON_RESIDUE)` of the Goldilocks
/// field, where `p` is [`MODULUS`]. A scalar `Scalar(a, b)` represents the polynomial `a * X + b`.
///
/// For all purposes other than field arithmetic (ordering, formatting, parsing, exponentiation,
/// etc.) a scalar is instead treated as the numeric value `a * MODULUS + b`. This gives every
/// scalar a canonical representative in `0..(MODULUS * MODULUS)` for those purposes.
///
/// NOTE: The `u64` words are stored from most significant to least significant: `Scalar::0` is the
/// most significant and `Scalar::1` is the least significant. This way Rust's automatic comparison
/// trait implementations work as intended.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scalar(pub(crate) u64, pub(crate) u64);

impl Scalar {
    /// Constructs a Goldilocks^2 scalar from a raw 64-bit value.
    #[inline]
    pub const fn from_const(value: u64) -> Self {
        Self(value / MODULUS, value % MODULUS)
    }
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        (((self.0 == other.0) && (self.1 == other.1)) as u8).into()
    }
}

impl ConstantTimeGreater for Scalar {
    fn ct_gt(&self, other: &Self) -> Choice {
        (((self.0, self.1) > (other.0, other.1)) as u8).into()
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
        Self(gl_add(self.0, rhs.0), gl_add(self.1, rhs.1))
    }
}

impl<'a> Add<&'a Self> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a Self) -> Self::Output {
        Self(gl_add(self.0, rhs.0), gl_add(self.1, rhs.1))
    }
}

impl AddAssign<Self> for Scalar {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = gl_add(self.0, rhs.0);
        self.1 = gl_add(self.1, rhs.1);
    }
}

impl<'a> AddAssign<&'a Self> for Scalar {
    fn add_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_add(self.0, rhs.0);
        self.1 = gl_add(self.1, rhs.1);
    }
}

impl Add<base::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: base::Scalar) -> Self::Output {
        Self(self.0, gl_add(self.1, rhs.0))
    }
}

impl<'a> Add<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(self.0, gl_add(self.1, rhs.0))
    }
}

impl AddAssign<base::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: base::Scalar) {
        self.1 = gl_add(self.1, rhs.0);
    }
}

impl<'a> AddAssign<&'a base::Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &'a base::Scalar) {
        self.1 = gl_add(self.1, rhs.0);
    }
}

impl Add<gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn add(self, rhs: gl4::Scalar) -> Self::Output {
        gl4::Scalar(rhs.0, rhs.1, gl_add(self.0, rhs.2), gl_add(self.1, rhs.3))
    }
}

impl<'a> Add<&'a gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn add(self, rhs: &'a gl4::Scalar) -> Self::Output {
        gl4::Scalar(rhs.0, rhs.1, gl_add(self.0, rhs.2), gl_add(self.1, rhs.3))
    }
}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Self::Output {
        Self(gl_sub(0, self.0), gl_sub(0, self.1))
    }
}

impl Sub<Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(gl_sub(self.0, rhs.0), gl_sub(self.1, rhs.1))
    }
}

impl<'a> Sub<&'a Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a Self) -> Self::Output {
        Self(gl_sub(self.0, rhs.0), gl_sub(self.1, rhs.1))
    }
}

impl SubAssign<Self> for Scalar {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = gl_sub(self.0, rhs.0);
        self.1 = gl_sub(self.1, rhs.1);
    }
}

impl<'a> SubAssign<&'a Self> for Scalar {
    fn sub_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_sub(self.0, rhs.0);
        self.1 = gl_sub(self.1, rhs.1);
    }
}

impl Sub<base::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: base::Scalar) -> Self::Output {
        Self(self.0, gl_sub(self.1, rhs.0))
    }
}

impl<'a> Sub<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(self.0, gl_sub(self.1, rhs.0))
    }
}

impl SubAssign<base::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: base::Scalar) {
        self.1 = gl_sub(self.1, rhs.0);
    }
}

impl<'a> SubAssign<&'a base::Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: &'a base::Scalar) {
        self.1 = gl_sub(self.1, rhs.0);
    }
}

impl Sub<gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn sub(self, rhs: gl4::Scalar) -> Self::Output {
        gl4::Scalar(
            gl_sub(0, rhs.0),
            gl_sub(0, rhs.1),
            gl_sub(self.0, rhs.2),
            gl_sub(self.1, rhs.3),
        )
    }
}

impl<'a> Sub<&'a gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn sub(self, rhs: &'a gl4::Scalar) -> Self::Output {
        gl4::Scalar(
            gl_sub(0, rhs.0),
            gl_sub(0, rhs.1),
            gl_sub(self.0, rhs.2),
            gl_sub(self.1, rhs.3),
        )
    }
}

impl Mul<Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: Self) -> Self::Output {
        let (hi, lo) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        Self(hi, lo)
    }
}

impl<'a> Mul<&'a Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a Self) -> Self::Output {
        let (hi, lo) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        Self(hi, lo)
    }
}

impl MulAssign<Self> for Scalar {
    fn mul_assign(&mut self, rhs: Self) {
        let (hi, lo) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        self.0 = hi;
        self.1 = lo;
    }
}

impl<'a> MulAssign<&'a Self> for Scalar {
    fn mul_assign(&mut self, rhs: &'a Self) {
        let (hi, lo) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        self.0 = hi;
        self.1 = lo;
    }
}

impl Mul<base::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: base::Scalar) -> Self::Output {
        Self(gl_mul(self.0, rhs.0), gl_mul(self.1, rhs.0))
    }
}

impl<'a> Mul<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a base::Scalar) -> Self::Output {
        Self(gl_mul(self.0, rhs.0), gl_mul(self.1, rhs.0))
    }
}

impl MulAssign<base::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: base::Scalar) {
        self.0 = gl_mul(self.0, rhs.0);
        self.1 = gl_mul(self.1, rhs.0);
    }
}

impl<'a> MulAssign<&'a base::Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &'a base::Scalar) {
        self.0 = gl_mul(self.0, rhs.0);
        self.1 = gl_mul(self.1, rhs.0);
    }
}

impl Mul<gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn mul(self, rhs: gl4::Scalar) -> Self::Output {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.0, self.1, rhs.2, rhs.3);
        gl4::Scalar(y0, y1, c0, c1)
    }
}

impl<'a> Mul<&'a gl4::Scalar> for Scalar {
    type Output = gl4::Scalar;

    fn mul(self, rhs: &'a gl4::Scalar) -> Self::Output {
        let (y0, y1) = gl_mul2(self.0, self.1, rhs.0, rhs.1);
        let (c0, c1) = gl_mul2(self.0, self.1, rhs.2, rhs.3);
        gl4::Scalar(y0, y1, c0, c1)
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
        let inverse = rhs.invert_unwrap();
        Self(gl_mul(self.0, inverse.0), gl_mul(self.1, inverse.0))
    }
}

impl<'a> Div<&'a base::Scalar> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: &'a base::Scalar) -> Self::Output {
        let inverse = rhs.invert_unwrap();
        Self(gl_mul(self.0, inverse.0), gl_mul(self.1, inverse.0))
    }
}

impl DivAssign<base::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: base::Scalar) {
        let inverse = rhs.invert_unwrap();
        self.0 = gl_mul(self.0, inverse.0);
        self.1 = gl_mul(self.1, inverse.0);
    }
}

impl<'a> DivAssign<&'a base::Scalar> for Scalar {
    fn div_assign(&mut self, rhs: &'a base::Scalar) {
        let inverse = rhs.invert_unwrap();
        self.0 = gl_mul(self.0, inverse.0);
        self.1 = gl_mul(self.1, inverse.0);
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
        write!(f, "Scalar({:#034x})", self)
    }
}

impl Display for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#034x}", self)
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
        Self(0, value as u64)
    }
}

impl From<u16> for Scalar {
    fn from(value: u16) -> Self {
        Self(0, value as u64)
    }
}

impl From<u32> for Scalar {
    fn from(value: u32) -> Self {
        Self(0, value as u64)
    }
}

impl From<u64> for Scalar {
    fn from(value: u64) -> Self {
        Self(value / MODULUS, value % MODULUS)
    }
}

impl From<base::Scalar> for Scalar {
    fn from(value: base::Scalar) -> Self {
        Self(0, value.0)
    }
}

impl TryFrom<usize> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self::from_const(value as u64))
    }
}

impl TryFrom<u128> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        let modulus = MODULUS as u128;
        let hi = value / modulus;
        let lo = value % modulus;
        if hi < modulus {
            Ok(Self(hi as u64, lo as u64))
        } else {
            Err(anyhow!("{:#x} exceeds the Goldilocks^2 range", value))
        }
    }
}

impl Field for Scalar {
    const LEN: usize = 16;

    const ZERO: Self = Self(0, 0);

    const ONE: Self = Self(0, 1);

    const MAX: Self = Self(MODULUS - 1, MODULUS - 1);

    fn is_odd(&self) -> Choice {
        (((self.0 ^ self.1) & 1) as u8).into()
    }

    fn try_random<R: TryCryptoRng>(rng: &mut R) -> Result<Self, R::Error> {
        Ok(Self(
            base::Scalar::try_random(rng)?.0,
            base::Scalar::try_random(rng)?.0,
        ))
    }

    fn random<R: CryptoRng>(rng: &mut R) -> Self {
        Self(base::Scalar::random(rng).0, base::Scalar::random(rng).0)
    }

    fn random_default() -> Self {
        Self(
            base::Scalar::random_default().0,
            base::Scalar::random_default().0,
        )
    }

    fn invert(&self) -> CtOption<Self> {
        let a = base::Scalar(self.0);
        let b = base::Scalar(self.1);
        let norm = b * b - a * a * base::Scalar(QUADRATIC_NON_RESIDUE);
        let conjugate = Self(gl_sub(0, self.0), self.1);
        norm.invert().map(|inverse_norm| conjugate * inverse_norm)
    }

    fn invert_vartime(&self) -> Option<Self> {
        let a = base::Scalar(self.0);
        let b = base::Scalar(self.1);
        let norm = b * b - a * a * base::Scalar(QUADRATIC_NON_RESIDUE);
        let conjugate = Self(gl_sub(0, self.0), self.1);
        norm.invert_vartime()
            .map(|inverse_norm| conjugate * inverse_norm)
    }

    fn pow(mut self, exp: Self) -> Self {
        let mut exponent = exp.to_u128();
        let mut result = Self::ONE;
        for _ in 0..Self::NUM_BITS {
            let product = result * self;
            result = Scalar::conditional_select(&result, &product, ((exponent & 1) as u8).into());
            exponent >>= 1;
            self = self.square();
        }
        result
    }

    fn pow_vartime(mut self, exp: Self) -> Self {
        let mut exponent = exp.to_u128();
        let mut result = Self::ONE;
        while exponent != 0 {
            if (exponent & 1) != 0 {
                result *= self;
            }
            exponent >>= 1;
            self = self.square();
        }
        result
    }

    fn div_int(&self, rhs: &Self) -> (Self, Self) {
        let modulus = MODULUS as u128;
        let lhs_value = self.to_u128();
        let rhs_value = rhs.to_u128();
        let quotient = lhs_value / rhs_value;
        let remainder = lhs_value % rhs_value;
        (
            Self((quotient / modulus) as u64, (quotient % modulus) as u64),
            Self((remainder / modulus) as u64, (remainder % modulus) as u64),
        )
    }

    fn try_from_le_bytes(bytes: &[u8]) -> CtOption<Self> {
        let mut fixed_bytes = [0u8; 16];
        fixed_bytes.copy_from_slice(bytes);
        let value = u128::from_le_bytes(fixed_bytes);
        let modulus = MODULUS as u128;
        let hi = value / modulus;
        let lo = value % modulus;
        CtOption::new(Self(hi as u64, lo as u64), ((hi < modulus) as u8).into())
    }

    fn try_from_be_bytes(bytes: &[u8]) -> CtOption<Self> {
        let mut fixed_bytes = [0u8; 16];
        fixed_bytes.copy_from_slice(bytes);
        let value = u128::from_be_bytes(fixed_bytes);
        let modulus = MODULUS as u128;
        let hi = value / modulus;
        let lo = value % modulus;
        CtOption::new(Self(hi as u64, lo as u64), ((hi < modulus) as u8).into())
    }

    fn from_str_radix(s: &str, radix: usize) -> Result<Self, std::fmt::Error> {
        assert!(radix >= 2 && radix <= 36);
        if s.is_empty() {
            return Err(std::fmt::Error);
        }
        let mut value: u128 = 0;
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
                .checked_mul(radix as u128)
                .ok_or(std::fmt::Error)?
                .checked_add(digit as u128)
                .ok_or(std::fmt::Error)?;
        }
        let modulus = MODULUS as u128;
        let hi = value / modulus;
        let lo = value % modulus;
        if hi >= modulus {
            return Err(std::fmt::Error);
        }
        Ok(Self(hi as u64, lo as u64))
    }

    fn to_str_radix(&self, radix: usize, pad_to: usize, upper_case: bool) -> String {
        assert!(radix >= 2 && radix <= 36);
        let characters = if upper_case {
            CHARACTERS_UPPER_CASE
        } else {
            CHARACTERS_LOWER_CASE
        };
        let mut value = self.to_u128();
        let mut s = String::default();
        let radix = radix as u128;
        while value != 0 {
            let digit = value % radix;
            s.push(characters[digit as usize] as char);
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
        if self.0 != 0 || self.1 > u8::MAX as u64 {
            None
        } else {
            Some(self.1 as u8)
        }
    }

    fn try_to_u16(&self) -> Option<u16> {
        if self.0 != 0 || self.1 > u16::MAX as u64 {
            None
        } else {
            Some(self.1 as u16)
        }
    }
}

impl Field128 for Scalar {
    fn to_le_bytes(&self) -> [u8; 16] {
        self.to_u128().to_le_bytes()
    }

    fn to_be_bytes(&self) -> [u8; 16] {
        self.to_u128().to_be_bytes()
    }

    fn from_u256_mod_n(u256: U256) -> Self {
        let modulus = U256::from(MODULUS);
        let value = u256 % (modulus * modulus);
        let hi = value / modulus;
        let lo = value % modulus;
        Self(hi.as_u64(), lo.as_u64())
    }

    fn from_h256(h256: H256) -> Self {
        Self::from_u256_mod_n(U256::from_little_endian(h256.as_bytes()))
    }

    fn try_to_u32(&self) -> CtOption<u32> {
        let hi_is_zero = Choice::from((self.0 == 0) as u8);
        let lo_fits = Choice::from((self.1 <= u32::MAX as u64) as u8);
        CtOption::new(self.1 as u32, hi_is_zero & lo_fits)
    }

    fn try_to_u64(&self) -> CtOption<u64> {
        CtOption::new(self.1, ((self.0 == 0) as u8).into())
    }

    fn to_u128(&self) -> u128 {
        (self.0 as u128) * (MODULUS as u128) + (self.1 as u128)
    }

    fn to_u256(&self) -> U256 {
        U256::from(self.to_u128())
    }

    fn to_u512(&self) -> U512 {
        U512::from(self.to_u128())
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
        assert_eq!(from_const(MODULUS), Scalar(1, 0));
        assert_eq!(from_const(MODULUS + 1), Scalar(1, 1));
    }

    #[test]
    fn test_zero() {
        assert_eq!(Scalar::ZERO, Scalar::zero());
        assert_eq!(Scalar::ZERO, from_const(0));
        assert_eq!(Scalar::ZERO + from_const(1), from_const(1));
        assert_eq!(Scalar::ZERO * from_const(42), Scalar::ZERO);
    }

    #[test]
    fn test_one() {
        assert_eq!(Scalar::ONE, Scalar::one());
        assert_eq!(Scalar::ONE, from_const(1));
        assert_eq!(Scalar::ONE * from_const(42), from_const(42));
    }

    #[test]
    fn test_max() {
        assert_eq!(Scalar::MAX, Scalar(MODULUS - 1, MODULUS - 1));
    }

    #[test]
    fn test_equality() {
        assert!(from_const(0) == from_const(0));
        assert!(from_const(0) != from_const(1));
        assert!(Scalar::MAX != Scalar::MAX - Scalar::ONE);
        assert!(Scalar::MAX == Scalar::MAX);
    }

    #[test]
    fn test_total_order() {
        let v0 = Scalar::ZERO;
        let v1 = Scalar::ONE;
        let v2 = Scalar(0, 42);
        let v3 = Scalar(1, 0);
        let v4 = Scalar::MAX;

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
        let a = Scalar(1, 42);
        let b = Scalar(1, 42);
        let c = Scalar(1, 43);
        let d = Scalar(2, 42);
        assert_eq!(bool::from(a.ct_eq(&b)), true);
        assert_eq!(bool::from(a.ct_eq(&c)), false);
        assert_eq!(bool::from(a.ct_eq(&d)), false);
    }

    #[test]
    fn test_ct_gt() {
        let v0 = Scalar::ZERO;
        let v1 = Scalar(0, 42);
        let v2 = Scalar(1, 0);
        assert_eq!(bool::from(v0.ct_gt(&v0)), false);
        assert_eq!(bool::from(v1.ct_gt(&v0)), true);
        assert_eq!(bool::from(v2.ct_gt(&v1)), true);
        assert_eq!(bool::from(v0.ct_gt(&v2)), false);
    }

    #[test]
    fn test_ct_lt() {
        let v0 = Scalar::ZERO;
        let v1 = Scalar(0, 42);
        let v2 = Scalar(1, 0);
        assert_eq!(bool::from(v0.ct_lt(&v1)), true);
        assert_eq!(bool::from(v1.ct_lt(&v2)), true);
        assert_eq!(bool::from(v2.ct_lt(&v0)), false);
    }

    #[test]
    fn test_conditional_select() {
        let a = from_const(12);
        let b = from_const(34);
        assert_eq!(Scalar::conditional_select(&a, &b, Choice::from(0)), a);
        assert_eq!(Scalar::conditional_select(&a, &b, Choice::from(1)), b);
    }

    #[test]
    fn test_add() {
        let lhs = Scalar(10, 20);
        let rhs = Scalar(5, 7);
        assert_eq!(lhs + rhs, Scalar(15, 27));
        assert_eq!(lhs + &rhs, Scalar(15, 27));
    }

    #[test]
    fn test_add_wraparound() {
        let lhs = Scalar(MODULUS - 5, MODULUS - 3);
        let rhs = Scalar(10, 10);
        assert_eq!(lhs + rhs, Scalar(5, 7));
    }

    #[test]
    fn test_add_assign() {
        let mut lhs = Scalar(10, 20);
        lhs += Scalar(5, 7);
        assert_eq!(lhs, Scalar(15, 27));
    }

    #[test]
    fn test_add_assign_ref() {
        let mut lhs = Scalar(10, 20);
        lhs += &Scalar(5, 7);
        assert_eq!(lhs, Scalar(15, 27));
    }

    #[test]
    fn test_add_base_scalar() {
        let lhs = Scalar(1, 2);
        let rhs = base::Scalar(5);
        assert_eq!(lhs + rhs, Scalar(1, 7));
        assert_eq!(lhs + &rhs, Scalar(1, 7));
    }

    #[test]
    fn test_add_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(1, 2);
        lhs += rhs;
        assert_eq!(lhs, Scalar(1, 7));

        let mut lhs = Scalar(1, 2);
        lhs += &rhs;
        assert_eq!(lhs, Scalar(1, 7));
    }

    #[test]
    fn test_add_gl4() {
        let lhs = Scalar(1, 2);
        let rhs = gl4::Scalar(3, 4, 5, 6);
        let expected = gl4::Scalar(3, 4, 6, 8);
        assert_eq!(lhs + rhs, expected);
        assert_eq!(lhs + &rhs, expected);
    }

    #[test]
    fn test_neg() {
        assert_eq!(-Scalar::ZERO, Scalar::ZERO);
        assert_eq!(-Scalar(1, 2), Scalar(MODULUS - 1, MODULUS - 2));
        assert_eq!(Scalar(1, 2) + -Scalar(1, 2), Scalar::ZERO);
    }

    #[test]
    fn test_sub() {
        let lhs = Scalar(15, 27);
        let rhs = Scalar(5, 7);
        assert_eq!(lhs - rhs, Scalar(10, 20));
        assert_eq!(lhs - &rhs, Scalar(10, 20));
    }

    #[test]
    fn test_sub_wraparound() {
        let lhs = Scalar(5, 7);
        let rhs = Scalar(10, 10);
        assert_eq!(lhs - rhs, Scalar(MODULUS - 5, MODULUS - 3));
    }

    #[test]
    fn test_sub_assign() {
        let mut lhs = Scalar(15, 27);
        lhs -= Scalar(5, 7);
        assert_eq!(lhs, Scalar(10, 20));
    }

    #[test]
    fn test_sub_assign_ref() {
        let mut lhs = Scalar(15, 27);
        lhs -= &Scalar(5, 7);
        assert_eq!(lhs, Scalar(10, 20));
    }

    #[test]
    fn test_sub_base_scalar() {
        let lhs = Scalar(1, 7);
        let rhs = base::Scalar(5);
        assert_eq!(lhs - rhs, Scalar(1, 2));
        assert_eq!(lhs - &rhs, Scalar(1, 2));
    }

    #[test]
    fn test_sub_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(1, 7);
        lhs -= rhs;
        assert_eq!(lhs, Scalar(1, 2));

        let mut lhs = Scalar(1, 7);
        lhs -= &rhs;
        assert_eq!(lhs, Scalar(1, 2));
    }

    #[test]
    fn test_sub_gl4() {
        let lhs = Scalar(1, 2);
        let rhs = gl4::Scalar(3, 4, 5, 6);
        let expected = gl4::Scalar(MODULUS - 3, MODULUS - 4, MODULUS - 4, MODULUS - 4);
        assert_eq!(lhs - rhs, expected);
        assert_eq!(lhs - &rhs, expected);
    }

    #[test]
    fn test_extension_root() {
        let x = Scalar(1, 0);
        assert_eq!(x * x, from_const(QUADRATIC_NON_RESIDUE));
        assert_eq!(x * &x, from_const(QUADRATIC_NON_RESIDUE));
    }

    #[test]
    fn test_mul_by_zero() {
        assert_eq!(Scalar::ZERO * Scalar(2, 3), Scalar::ZERO);
        assert_eq!(Scalar(2, 3) * Scalar::ZERO, Scalar::ZERO);
    }

    #[test]
    fn test_mul_by_one() {
        assert_eq!(Scalar::ONE * Scalar(2, 3), Scalar(2, 3));
        assert_eq!(Scalar(2, 3) * Scalar::ONE, Scalar(2, 3));
    }

    #[test]
    fn test_mul() {
        let lhs = Scalar(2, 3);
        let rhs = Scalar(5, 7);
        let expected = Scalar(29, 21 + 10 * QUADRATIC_NON_RESIDUE);
        assert_eq!(lhs * rhs, expected);
        assert_eq!(lhs * &rhs, expected);
        assert_eq!(rhs * lhs, expected);
    }

    #[test]
    fn test_mul_assign() {
        let mut lhs = Scalar(2, 3);
        lhs *= Scalar(5, 7);
        assert_eq!(lhs, Scalar(29, 21 + 10 * QUADRATIC_NON_RESIDUE));
    }

    #[test]
    fn test_mul_base_scalar() {
        let lhs = Scalar(2, 3);
        let rhs = base::Scalar(5);
        assert_eq!(lhs * rhs, Scalar(10, 15));
        assert_eq!(lhs * &rhs, Scalar(10, 15));
    }

    #[test]
    fn test_mul_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(2, 3);
        lhs *= rhs;
        assert_eq!(lhs, Scalar(10, 15));

        let mut lhs = Scalar(2, 3);
        lhs *= &rhs;
        assert_eq!(lhs, Scalar(10, 15));
    }

    #[test]
    fn test_mul_gl4() {
        let lhs = Scalar(5, 6);
        let rhs = gl4::Scalar(1, 2, 3, 4);
        let expected = gl4::Scalar(16, 47, 38, 129);
        assert_eq!(lhs * rhs, expected);
        assert_eq!(lhs * &rhs, expected);
    }

    #[test]
    fn test_div_by_one() {
        assert_eq!(Scalar(2, 3) / Scalar::ONE, Scalar(2, 3));
        assert_eq!(Scalar(2, 3) / &Scalar::ONE, Scalar(2, 3));
    }

    #[test]
    fn test_div() {
        let lhs = Scalar(2, 3);
        let rhs = Scalar(5, 7);
        assert_eq!((lhs * rhs) / rhs, lhs);
        assert_eq!((lhs * rhs) / &rhs, lhs);
    }

    #[test]
    fn test_div_base_scalar() {
        let lhs = Scalar(10, 15);
        let rhs = base::Scalar(5);
        assert_eq!(lhs / rhs, Scalar(2, 3));
        assert_eq!(lhs / &rhs, Scalar(2, 3));
    }

    #[test]
    fn test_div_assign_base_scalar() {
        let rhs = base::Scalar(5);

        let mut lhs = Scalar(10, 15);
        lhs /= rhs;
        assert_eq!(lhs, Scalar(2, 3));

        let mut lhs = Scalar(10, 15);
        lhs /= &rhs;
        assert_eq!(lhs, Scalar(2, 3));
    }

    #[test]
    fn test_sum() {
        let values = vec![Scalar::ONE, Scalar(0, 2), Scalar(1, 0)];
        assert_eq!(values.iter().sum::<Scalar>(), Scalar(1, 3));
        assert_eq!(values.into_iter().sum::<Scalar>(), Scalar(1, 3));
    }

    #[test]
    fn test_product() {
        let values = vec![Scalar(0, 2), Scalar(0, 3), Scalar(0, 4)];
        assert_eq!(values.iter().product::<Scalar>(), Scalar(0, 24));
        assert_eq!(values.into_iter().product::<Scalar>(), Scalar(0, 24));
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
    }

    #[test]
    fn test_from_base_scalar() {
        assert_eq!(Scalar::from(base::Scalar(42)), Scalar(0, 42));
        assert_eq!(Scalar::from(base::Scalar::ZERO), Scalar::ZERO);
    }

    #[test]
    fn test_try_from_usize() {
        assert_eq!(Scalar::try_from(0usize).unwrap(), from_const(0));
        assert_eq!(Scalar::try_from(42usize).unwrap(), from_const(42));
    }

    #[test]
    fn test_try_from_u128() {
        assert_eq!(Scalar::try_from(0u128).unwrap(), from_const(0));
        assert_eq!(Scalar::try_from(42u128).unwrap(), from_const(42));
        assert_eq!(
            Scalar::try_from(Scalar::MAX.to_u128()).unwrap(),
            Scalar::MAX
        );
        let modulus_squared = (MODULUS as u128) * (MODULUS as u128);
        assert!(Scalar::try_from(modulus_squared).is_err());
        assert!(Scalar::try_from(u128::MAX).is_err());
    }

    #[test]
    fn test_is_even() {
        assert!(bool::from(Scalar(0, 0).is_even()));
        assert!(bool::from(Scalar(1, 1).is_even()));
        assert!(!bool::from(Scalar(0, 1).is_even()));
        assert!(!bool::from(Scalar(1, 0).is_even()));
    }

    #[test]
    fn test_is_odd() {
        assert!(!bool::from(Scalar(0, 0).is_odd()));
        assert!(!bool::from(Scalar(1, 1).is_odd()));
        assert!(bool::from(Scalar(0, 1).is_odd()));
        assert!(bool::from(Scalar(1, 0).is_odd()));
    }

    struct OsRng;

    impl rand_core::TryRng for OsRng {
        type Error = getrandom::Error;

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
            getrandom::fill(dest)
        }

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0u8; 4];
            getrandom::fill(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0u8; 8];
            getrandom::fill(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }
    }

    impl rand_core::TryCryptoRng for OsRng {}

    #[test]
    fn test_try_random() {
        let mut rng = OsRng;
        assert_ne!(
            Scalar::try_random(&mut rng).unwrap(),
            Scalar::try_random(&mut rng).unwrap()
        );
    }

    #[test]
    fn test_random() {
        let mut rng = rand_core::UnwrapErr(OsRng);
        assert_ne!(Scalar::random(&mut rng), Scalar::random(&mut rng));
    }

    #[test]
    fn test_random_default() {
        assert_ne!(Scalar::random_default(), Scalar::random_default());
    }

    #[test]
    fn test_double() {
        assert_eq!(Scalar(2, 3).double(), Scalar(4, 6));
    }

    #[test]
    fn test_square() {
        assert_eq!(Scalar(0, 5).square(), Scalar(0, 25));
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
        test_inversion_impl(Scalar(0, 42));
        test_inversion_impl(Scalar(1, 0));
        test_inversion_impl(Scalar(7, 11));
        test_inversion_impl(Scalar::MAX);
    }

    #[test]
    fn test_invert_batch() {
        let values = vec![Scalar(1, 2), Scalar(0, 42), Scalar::ONE, Scalar(3, 5)];
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
    fn test_power() {
        let value = Scalar(1, 2);
        assert_eq!(value.pow(Scalar::ZERO), Scalar::ONE);
        assert_eq!(value.pow(Scalar::ONE), value);
        assert_eq!(value.pow(from_const(2)), value * value);
        assert_eq!(value.pow(from_const(3)), value * value * value);
    }

    #[test]
    fn test_power_vartime() {
        let value = Scalar(1, 2);
        assert_eq!(value.pow_vartime(Scalar::ZERO), Scalar::ONE);
        assert_eq!(value.pow_vartime(from_const(3)), value * value * value);
        assert_eq!(value.pow_vartime(from_const(3)), value.pow(from_const(3)));
    }

    #[test]
    fn test_integer_division() {
        assert_eq!(
            from_const(13).div_int(&from_const(5)),
            (from_const(2), from_const(3))
        );
    }

    #[test]
    fn test_try_from_le_bytes() {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&42u64.to_le_bytes());
        assert_eq!(Scalar::try_from_le_bytes(&bytes).unwrap(), from_const(42));
        assert!(bool::from(
            Scalar::try_from_le_bytes(&[255u8; 16]).is_none()
        ));
    }

    #[test]
    fn test_try_from_be_bytes() {
        let mut bytes = [0u8; 16];
        bytes[8..16].copy_from_slice(&42u64.to_be_bytes());
        assert_eq!(Scalar::try_from_be_bytes(&bytes).unwrap(), from_const(42));
    }

    #[test]
    fn test_le_be_bytes_roundtrip() {
        let value = Scalar(1, 42);
        let modulus = MODULUS as u128;
        let n = (value.0 as u128) * modulus + (value.1 as u128);
        assert_eq!(Scalar::try_from_le_bytes(&n.to_le_bytes()).unwrap(), value);
        assert_eq!(Scalar::try_from_be_bytes(&n.to_be_bytes()).unwrap(), value);
    }

    #[test]
    fn test_fmt_display() {
        assert_eq!(
            format!("{}", from_const(0)),
            "0x00000000000000000000000000000000"
        );
        assert_eq!(
            format!("{}", from_const(0xdeadbeef)),
            "0x000000000000000000000000deadbeef"
        );
    }

    #[test]
    fn test_fmt_debug() {
        assert_eq!(
            format!("{:?}", from_const(0)),
            "Scalar(0x00000000000000000000000000000000)"
        );
    }

    #[test]
    fn test_fmt_lower_hex() {
        assert_eq!(format!("{:x}", from_const(0xdeadbeef)), "deadbeef");
        assert_eq!(format!("{:#x}", from_const(0xdeadbeef)), "0xdeadbeef");
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
    fn test_parse_scalar() {
        assert_eq!(parse_scalar("0x2a"), from_const(42));
    }

    #[test]
    fn test_try_to_u8() {
        assert_eq!(from_const(0).try_to_u8().unwrap(), 0);
        assert_eq!(from_const(u8::MAX as u64).try_to_u8().unwrap(), u8::MAX);
        assert!(from_const(u8::MAX as u64 + 1).try_to_u8().is_none());
        assert!(Scalar(1, 0).try_to_u8().is_none());
    }

    #[test]
    fn test_try_to_u16() {
        assert_eq!(from_const(0).try_to_u16().unwrap(), 0);
        assert_eq!(from_const(u16::MAX as u64).try_to_u16().unwrap(), u16::MAX);
        assert!(from_const(u16::MAX as u64 + 1).try_to_u16().is_none());
        assert!(Scalar(1, 0).try_to_u16().is_none());
    }

    #[test]
    fn test_field128_to_le_bytes() {
        let value = Scalar(1, 42);
        let n = (value.0 as u128) * (MODULUS as u128) + (value.1 as u128);
        assert_eq!(value.to_le_bytes(), n.to_le_bytes());
    }

    #[test]
    fn test_field128_to_be_bytes() {
        let value = Scalar(1, 42);
        let n = (value.0 as u128) * (MODULUS as u128) + (value.1 as u128);
        assert_eq!(value.to_be_bytes(), n.to_be_bytes());
    }

    #[test]
    fn test_field128_le_be_bytes_roundtrip() {
        let value = Scalar(7, 11);
        assert_eq!(
            Scalar::try_from_le_bytes(&value.to_le_bytes()).unwrap(),
            value
        );
        assert_eq!(
            Scalar::try_from_be_bytes(&value.to_be_bytes()).unwrap(),
            value
        );
    }

    #[test]
    fn test_from_u256_mod_n() {
        assert_eq!(Scalar::from_u256_mod_n(U256::from(0)), from_const(0));
        assert_eq!(Scalar::from_u256_mod_n(U256::from(42)), from_const(42));
        assert_eq!(Scalar::from_u256_mod_n(U256::from(MODULUS)), Scalar(1, 0));
        let modulus_squared = U256::from(MODULUS) * U256::from(MODULUS);
        assert_eq!(Scalar::from_u256_mod_n(modulus_squared), Scalar::ZERO);
        assert_eq!(
            Scalar::from_u256_mod_n(modulus_squared + U256::from(1)),
            Scalar::ONE
        );
    }

    #[test]
    fn test_from_h256() {
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&42u64.to_le_bytes());
        assert_eq!(Scalar::from_h256(H256::from_slice(&bytes)), from_const(42));
    }

    #[test]
    fn test_field128_try_to_u32() {
        assert_eq!(from_const(0).try_to_u32().unwrap(), 0);
        assert_eq!(from_const(u32::MAX as u64).try_to_u32().unwrap(), u32::MAX);
        assert!(bool::from(
            from_const(u32::MAX as u64 + 1).try_to_u32().is_none()
        ));
        assert!(bool::from(Scalar(1, 0).try_to_u32().is_none()));
    }

    #[test]
    fn test_try_to_u64() {
        assert_eq!(from_const(0).try_to_u64().unwrap(), 0);
        assert_eq!(from_const(42).try_to_u64().unwrap(), 42);
        assert_eq!(Scalar(0, MODULUS - 1).try_to_u64().unwrap(), MODULUS - 1);
        assert!(bool::from(Scalar(1, 0).try_to_u64().is_none()));
    }

    #[test]
    fn test_field128_to_u128() {
        assert_eq!(from_const(0).to_u128(), 0);
        assert_eq!(from_const(42).to_u128(), 42);
        assert_eq!(
            Scalar::MAX.to_u128(),
            (MODULUS as u128) * (MODULUS as u128) - 1
        );
    }

    #[test]
    fn test_field128_to_u256() {
        assert_eq!(from_const(0).to_u256(), U256::from(0));
        assert_eq!(from_const(42).to_u256(), U256::from(42));
        assert_eq!(Scalar::MAX.to_u256(), U256::from(Scalar::MAX.to_u128()));
    }

    #[test]
    fn test_field128_to_u512() {
        assert_eq!(from_const(0).to_u512(), U512::from(0));
        assert_eq!(from_const(42).to_u512(), U512::from(42));
        assert_eq!(Scalar::MAX.to_u512(), U512::from(Scalar::MAX.to_u128()));
    }
}
