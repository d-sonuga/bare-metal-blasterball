//! Collections for managing heap allocated values

#![cfg_attr(not(test), no_std)]
#![feature(lang_items, box_syntax, rustc_attrs)]

pub mod vec;
pub mod allocator;
pub mod boxed;
pub use allocator::Allocator;