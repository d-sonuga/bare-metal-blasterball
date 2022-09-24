//! Spin based synchronization primitives

#![cfg_attr(not(test), no_std)]

pub mod mutex;
pub mod once;