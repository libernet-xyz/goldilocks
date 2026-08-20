use crate::helpers::{MODULUS, gl_add};
use starkom_ff::Field;
use std::ops::{Add, AddAssign};
use subtle::{
    Choice, ConditionallySelectable, ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess,
};

/// Goldilocks^2 extension field.
///
/// NOTE: The `u64` words are stored from most significant to least significant: `Scalar::0` is the
/// most significant and `Scalar::1` is the least significant. This way Rust's automatic comparison
/// trait implementations work as intended.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scalar(u64, u64);

impl Scalar {
    /// Constructs a Goldilocks^2 scalar from its raw 64-bit value.
    ///
    /// Panics if the specified `value` exceeds [`MODULUS`].
    #[inline]
    pub const fn from_const(value: u64) -> Self {
        Self(value / MODULUS, value % MODULUS)
    }
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        todo!()
    }
}

impl ConstantTimeGreater for Scalar {
    fn ct_gt(&self, other: &Self) -> Choice {
        todo!()
    }
}

impl ConstantTimeLess for Scalar {}

impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        todo!()
    }
}

impl Add for Scalar {
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

impl Field for Scalar {
    const LEN: usize = 16;

    const ZERO: Self = Self(0, 0);

    const ONE: Self = Self(0, 1);

    const MAX: Self = Self(MODULUS - 1, 0);

    fn is_odd(&self) -> subtle::Choice {
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
        todo!()
    }

    fn invert_vartime(&self) -> Option<Self> {
        todo!()
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

    fn try_from_le_bytes(bytes: &[u8]) -> subtle::CtOption<Self> {
        todo!()
    }

    fn try_from_be_bytes(bytes: &[u8]) -> subtle::CtOption<Self> {
        todo!()
    }

    fn from_str_radix(s: &str, radix: usize) -> Result<Self, std::fmt::Error> {
        todo!()
    }

    fn to_str_radix(&self, radix: usize, pad_to: usize, upper_case: bool) -> String {
        todo!()
    }

    fn try_to_u8(&self) -> Option<u8> {
        todo!()
    }

    fn try_to_u16(&self) -> Option<u16> {
        todo!()
    }
}
