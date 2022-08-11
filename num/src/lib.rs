//! General numeric and bitwise operations on numbers

#![cfg_attr(not(test), no_std)]
#![feature(custom_test_frameworks)]
#![cfg_attr(test, test_runner(tester::test_runner))]


#[cfg(test)]
mod tests;

use core::mem;
use core::ops::{Add, Sub, Rem, Div, Mul, Range, RangeBounds, Bound, Shl};



/// A trait to provide generic number and bitwise operations
pub trait Num: NumOps + PartialEq + PartialOrd + Sized {
    /// Number of bits
    ///
    /// ```rust
    /// use num::Num;
    ///
    /// assert_eq!(u32::BIT_LENGTH, 32);
    /// assert_eq!(i128::BIT_LENGTH, 128);
    /// ```
    const BIT_LENGTH: usize;

    /// Sets the bit in index i, where the least significant bit has index 0
    /// and the highest significant bit has index BIT_LENGTH - 1
    ///
    /// ```rust
    /// use num::Num;
    ///
    /// let mut n = 0u32;
    /// n.set_bit(1);
    /// assert_eq!(n, 2u32);
    /// ```
    ///
    /// ## Panics
    ///
    /// Will panic if the index i is out of bounds of the bit length
    fn set_bit(&mut self, i: usize);

    /// Unsets the bit in index i, where the least significant bit has index 0
    /// and the highest significant bit has index BIT_LENGTH - 1
    ///
    /// ```rust
    /// use num::Num;
    ///
    /// let mut n = 0b1010_1011_1u128;
    /// n.unset_bit(4);
    /// assert_eq!(n, 0b1010_0011_1u128);
    /// ```
    ///
    /// ## Panics
    ///
    /// Will panic if the index i is out of bounds of the bit length
    fn unset_bit(&mut self, i: usize);

    /// Returns the bit state of the bit at the ith index
    ///
    /// ```rust
    /// use num::{Num, BitState};
    ///
    /// let mut n = 0b1101101u32;
    /// assert_eq!(BitState::Set, n.get_bit(3));
    /// ```
    ///
    /// ## Panics
    ///
    /// Will panic if index i is out of bounds of the bit length
    fn get_bit(&self, i: usize) -> BitState;

    /// Sets range of bits to the supplied value
    ///
    /// ```rust
    /// use num::Num;
    ///
    /// let mut n = 0u64;
    /// n.set_bits(2..5, 0b111);
    /// assert_eq!(n, 0b11100);
    /// ```
    ///
    /// ## Panics
    ///
    /// Will panic if range is out of range of the bit length
    fn set_bits<R: RangeBounds<usize>>(&mut self, range: R, value: Self);

    /// Gets the bits in the range specified
    ///
    /// ```rust
    /// use num::Num;
    ///
    /// let n = 0b1011011i128;
    /// assert_eq!(0b1011, n.get_bits(3..=6));
    /// ```
    ///
    /// ## Panics
    ///
    /// Will panic if the range is out of range of the bit length
    fn get_bits<R: RangeBounds<usize>>(&self, range: R) -> Self;

    fn to_u8(&self) -> u8;

    fn to_u64(&self) -> u64;
}

pub trait NumOps<Rhs=Self, Output=Self>:
    Add<Rhs, Output=Output>
    + Sub<Rhs, Output=Output>
    + Div<Rhs, Output=Output>
    + Mul<Rhs, Output=Output>
    + Rem<Rhs, Output=Output>
    {}

impl <T, Rhs, Output> NumOps<Rhs, Output> for T where
    T: Add<Rhs, Output=Output>
        + Sub<Rhs, Output=Output>
        + Div<Rhs, Output=Output>
        + Mul<Rhs, Output=Output>
        + Rem<Rhs, Output=Output>
    {}

macro_rules! impl_num {
    ($($t:ty)+) => {$(
        impl Num for $t {
            const BIT_LENGTH: usize = mem::size_of::<$t>() * 8;
            
            fn set_bit(&mut self, i: usize){
                assert!(i < Self::BIT_LENGTH);
                *self |= 1 << i;
            }

            fn unset_bit(&mut self, i: usize) {
                self.set_bits(i..i+1, 0);
            }

            fn get_bit(&self, i: usize) -> BitState {
                assert!(i < Self::BIT_LENGTH);
                match (*self >> i) & 1 {
                    0 => BitState::Unset,
                    1 => BitState::Set,
                    _ => unreachable!()
                }
            }

            fn set_bits<R: RangeBounds<usize>>(&mut self, range: R, value: Self){
                let range = to_range(range, Self::BIT_LENGTH);
                assert!(range.start < Self::BIT_LENGTH);
                assert!(range.end <= Self::BIT_LENGTH);
                assert!(range.start < range.end);
                assert!(
                    value << (Self::BIT_LENGTH - (range.end - range.start))
                          >> (Self::BIT_LENGTH - (range.end - range.start))
                          == value,
                    "The given value does not fit in the given range"
                );
                let mask = !(
                    !0 << range.start
                    & (!0 >> (Self::BIT_LENGTH - range.end))
                );
                *self = *self & mask | (value << range.start);
            }

            fn get_bits<R: RangeBounds<usize>>(&self, range: R) -> Self {
                let range = to_range(range, Self::BIT_LENGTH);
                assert!(range.start < Self::BIT_LENGTH);
                assert!(range.end <= Self::BIT_LENGTH);
                assert!(range.start < range.end);
                *self >> range.start & (!0 >> (Self::BIT_LENGTH - range.end))
            }

            fn to_u8(&self) -> u8 {
                *self as u8
            }

            fn to_u64(&self) -> u64 {
                *self as u64
            }
        }
    )+}
}

impl_num! { u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 }

/// Represents whether or not a bit has been set
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BitState {
    Set,
    Unset
}

fn to_range<R: RangeBounds<usize>>(range: R, max_length: usize) -> Range<usize> {
    let start = match range.start_bound(){
        Bound::Included(&i) => i,
        Bound::Excluded(&i) => i + 1,
        Bound::Unbounded    => 0
    };
    let end = match range.end_bound(){
        Bound::Included(&i) => i + 1,
        Bound::Excluded(&i) => i,
        Bound::Unbounded    => max_length - 1
    };
    start..end
}