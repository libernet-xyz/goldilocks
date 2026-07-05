use anyhow::Context;
use primitive_types::{U256, U512};
use rand_core::{CryptoRng, TryCryptoRng};
use starkom_ff::{Field, Field64, PrimeField, PrimeField64};
use std::fmt::{Binary, Debug, Display, Formatter, LowerHex, Octal, UpperHex};
use std::iter::{Product, Sum};
use std::num::ParseIntError;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;
use subtle::{
    Choice, ConditionallySelectable, ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess,
    CtOption,
};

pub const MODULUS: u64 = 0xFFFFFFFF00000001u64;

#[inline(always)]
const fn gl_add(lhs: u64, rhs: u64) -> u64 {
    let (mut value, overflow) = lhs.overflowing_add(rhs);
    if overflow {
        value -= MODULUS;
    }
    value
}

#[inline(always)]
const fn gl_sub(lhs: u64, rhs: u64) -> u64 {
    if rhs > lhs {
        MODULUS - rhs + lhs
    } else {
        lhs - rhs
    }
}

#[inline(always)]
const fn gl_mul(lhs: u64, rhs: u64) -> u64 {
    let wide_value = (lhs as u128) * (rhs as u128);
    (wide_value % (MODULUS as u128)) as u64
}

/// A Goldilocks scalar.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scalar(u64);

impl Scalar {
    #[inline]
    pub const fn from_const(value: u64) -> Self {
        assert!(value < MODULUS, "invalid Goldilocks value");
        Self(value)
    }
}

/// Alias for [`Scalar::from_const`].
#[inline(always)]
pub const fn from_const(value: u64) -> Scalar {
    Scalar::from_const(value)
}

/// Parses a scalar from a string using the [`FromStr`] trait and unwrapping the result.
///
/// REQUIRES: the input string must be a static one known to have a valid scalar.
#[inline]
pub fn parse_scalar(s: &'static str) -> Scalar {
    s.parse().unwrap()
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        ((self.0 == other.0) as u8).into()
    }
}

impl ConstantTimeGreater for Scalar {
    fn ct_gt(&self, other: &Self) -> Choice {
        ((self.0 > other.0) as u8).into()
    }
}

impl ConstantTimeLess for Scalar {}

impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        if choice.into() { *a } else { *b }
    }
}

impl Add<Self> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: Self) -> Self::Output {
        Self(gl_add(self.0, rhs.0))
    }
}

impl<'a> Add<&'a Self> for Scalar {
    type Output = Scalar;

    fn add(self, rhs: &'a Self) -> Self::Output {
        self.add(*rhs)
    }
}

impl AddAssign<Self> for Scalar {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = gl_add(self.0, rhs.0);
    }
}

impl<'a> AddAssign<&'a Self> for Scalar {
    fn add_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_add(self.0, rhs.0);
    }
}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Self::Output {
        Self(MODULUS - self.0)
    }
}

impl Sub<Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(gl_sub(self.0, rhs.0))
    }
}

impl<'a> Sub<&'a Self> for Scalar {
    type Output = Scalar;

    fn sub(self, rhs: &'a Self) -> Self::Output {
        Self(gl_sub(self.0, rhs.0))
    }
}

impl SubAssign<Self> for Scalar {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = gl_sub(self.0, rhs.0);
    }
}

impl<'a> SubAssign<&'a Self> for Scalar {
    fn sub_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_sub(self.0, rhs.0);
    }
}

impl Mul<Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(gl_mul(self.0, rhs.0))
    }
}

impl<'a> Mul<&'a Self> for Scalar {
    type Output = Scalar;

    fn mul(self, rhs: &'a Self) -> Self::Output {
        Self(gl_mul(self.0, rhs.0))
    }
}

impl MulAssign<Self> for Scalar {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 = gl_mul(self.0, rhs.0);
    }
}

impl<'a> MulAssign<&'a Self> for Scalar {
    fn mul_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_mul(self.0, rhs.0);
    }
}

impl Div<Self> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: Self) -> Self::Output {
        Self(gl_mul(self.0, rhs.invert_unwrap().0))
    }
}

impl<'a> Div<&'a Self> for Scalar {
    type Output = Scalar;

    fn div(self, rhs: &'a Self) -> Self::Output {
        Self(gl_mul(self.0, rhs.invert_unwrap().0))
    }
}

impl DivAssign<Self> for Scalar {
    fn div_assign(&mut self, rhs: Self) {
        self.0 = gl_mul(self.0, rhs.invert_unwrap().0);
    }
}

impl<'a> DivAssign<&'a Self> for Scalar {
    fn div_assign(&mut self, rhs: &'a Self) {
        self.0 = gl_mul(self.0, rhs.invert_unwrap().0);
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
        write!(f, "Scalar({:#018x})", self)
    }
}

impl Display for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#018x}", self)
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
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl From<u8> for Scalar {
    fn from(value: u8) -> Self {
        Self(value as u64)
    }
}

impl From<u16> for Scalar {
    fn from(value: u16) -> Self {
        Self(value as u64)
    }
}

impl From<u32> for Scalar {
    fn from(value: u32) -> Self {
        Self(value as u64)
    }
}

impl TryFrom<usize> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::try_from_le_bytes(&value.to_le_bytes())
            .into_option()
            .context("overflow")
    }
}

impl TryFrom<u64> for Scalar {
    type Error = anyhow::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::try_from_le_bytes(&value.to_le_bytes())
            .into_option()
            .context("overflow")
    }
}

impl Field for Scalar {
    const LEN: usize = 8;

    const ZERO: Self = Self(0);

    const ONE: Self = Self(1);

    const MAX: Self = Self(MODULUS - 1);

    fn is_odd(&self) -> Choice {
        ((self.0 & 1) as u8).into()
    }

    fn try_random<R: rand_core::TryCryptoRng>(rng: &mut R) -> Result<Self, R::Error> {
        let mut bytes = [0u8; 64];
        rng.try_fill_bytes(&mut bytes)?;
        Ok(Self::from_u256_mod_n(U256::from_little_endian(&bytes)))
    }

    fn random<R: rand_core::CryptoRng>(rng: &mut R) -> Self {
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        Self::from_u256_mod_n(U256::from_little_endian(&bytes))
    }

    fn random_default() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        Self::from_u256_mod_n(U256::from_little_endian(&bytes))
    }

    fn invert(&self) -> CtOption<Self> {
        CtOption::new(self.pow(Scalar::MINUS_TWO), !self.is_zero())
    }

    fn invert_vartime(&self) -> Option<Self> {
        if self.is_zero().into() {
            None
        } else {
            Some(self.pow(Scalar::MINUS_TWO))
        }
    }

    fn pow(self, exp: Self) -> Self {
        todo!()
    }

    fn pow_vartime(self, exp: Self) -> Self {
        todo!()
    }

    fn div_int(&self, rhs: &Self) -> (Self, Self) {
        (Self(self.0 / rhs.0), Self(self.0 % rhs.0))
    }

    fn try_from_le_bytes(bytes: &[u8]) -> CtOption<Self> {
        let mut fixed_bytes = [0u8; 8];
        fixed_bytes.copy_from_slice(bytes);
        let value = u64::from_le_bytes(fixed_bytes);
        CtOption::new(Self(value), ((value < MODULUS) as u8).into())
    }

    fn try_from_be_bytes(bytes: &[u8]) -> CtOption<Self> {
        let mut fixed_bytes = [0u8; 8];
        fixed_bytes.copy_from_slice(bytes);
        let value = u64::from_be_bytes(fixed_bytes);
        CtOption::new(Self(value), ((value < MODULUS) as u8).into())
    }

    fn from_str_radix(s: &str, radix: usize) -> Result<Self, std::fmt::Error> {
        todo!()
    }

    fn to_str_radix(&self, radix: usize, pad_to: usize, upper_case: bool) -> String {
        todo!()
    }

    fn try_to_u8(&self) -> Option<u8> {
        if self.0 > u8::MAX as u64 {
            None
        } else {
            Some(self.0 as u8)
        }
    }

    fn try_to_u16(&self) -> Option<u16> {
        if self.0 > u16::MAX as u64 {
            None
        } else {
            Some(self.0 as u16)
        }
    }
}

impl Field64 for Scalar {
    fn to_le_bytes(&self) -> [u8; 8] {
        todo!()
    }

    fn to_be_bytes(&self) -> [u8; 8] {
        todo!()
    }

    fn from_u128_mod_n(u128: u128) -> Self {
        todo!()
    }

    fn from_u256_mod_n(u256: U256) -> Self {
        todo!()
    }

    fn try_to_u32(&self) -> CtOption<u32> {
        todo!()
    }

    fn to_u64(&self) -> u64 {
        todo!()
    }

    fn to_u128(&self) -> u128 {
        todo!()
    }

    fn to_u256(&self) -> U256 {
        todo!()
    }

    fn to_u512(&self) -> U512 {
        todo!()
    }
}

impl PrimeField for Scalar {
    const MODULUS: &'static str = "0xFFFFFFFF00000001";

    const S: usize = 32;

    const MULTIPLICATIVE_GENERATOR: Self;

    const MINUS_TWO: Self = Self(MODULUS - 2);

    const TWO_INV: Self;

    const ROOT_OF_UNITY: Self;

    const ROOT_OF_UNITY_INV: Self;

    const DELTA: Self;
}

impl PrimeField64 for Scalar {}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO
}
