//! Collections for managing heap allocated values

#![cfg_attr(not(test), no_std)]

pub mod vec;
mod allocator;
pub use allocator::Allocator;