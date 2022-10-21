//! Collections for managing heap allocated values

#![cfg_attr(not(test), no_std)]
#![allow(dead_code)]

pub mod vec;
pub mod allocator;
pub mod boxed;
pub mod queue;
pub use allocator::Allocator;