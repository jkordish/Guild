#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Pure deterministic effect protocol kernel. This crate performs no I/O.

pub mod protocol;
pub mod scalar;
