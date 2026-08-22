# Goldilocks

[![CI](https://img.shields.io/github/actions/workflow/status/libernet-xyz/goldilocks/ci.yml?label=CI)](https://github.com/libernet-xyz/goldilocks/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/starkom-goldilocks)](https://crates.io/crates/starkom-goldilocks)
[![license](https://img.shields.io/crates/l/starkom-goldilocks)](https://github.com/libernet-xyz/goldilocks/blob/main/LICENSE)

## Overview

Starkom's implementation of the Goldilocks field.

The order of the field is the Goldilocks prime $p = 2^{64} - 2^{32} + 1$, or `0xFFFFFFFF00000001`.

This crate provides not only the base Goldilocks field but also the extension fields Goldilocks^2
and Goldilocks^4.
