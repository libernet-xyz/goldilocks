// Copyright 2026 The Libernet Team
// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

mod helpers;

pub mod base;
pub mod gl2;
pub mod gl4;

pub use base::Scalar as GL;
pub use gl2::Scalar as GL2;
pub use gl4::Scalar as GL4;

/// The order of the Goldilocks field, `0xffffffff00000001`.
pub const MODULUS: u64 = helpers::MODULUS;

/// The quadratic non-residue used to build the extension: `X^2 = QUADRATIC_NON_RESIDUE` in the base
/// field.
pub const QUADRATIC_NON_RESIDUE: u64 = helpers::QUADRATIC_NON_RESIDUE;

/// Alias for [`GL::from_const`].
#[inline(always)]
pub const fn from_const(value: u64) -> GL {
    GL::from_const(value)
}
