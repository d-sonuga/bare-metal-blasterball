//! Collections for managing heap allocated values

#![cfg_attr(not(test), no_std)]
#![feature(lang_items, box_syntax, rustc_attrs, core_intrinsics)]

pub mod vec;
pub mod allocator;
pub mod boxed;
pub mod queue;
pub use allocator::Allocator;