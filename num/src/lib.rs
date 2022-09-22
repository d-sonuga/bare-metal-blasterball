//! General numeric and bitwise operations on numbers

#![cfg_attr(not(test), no_std)]
#![feature(custom_test_frameworks, core_intrinsics)]
//#![cfg_attr(test, test_runner(tester::test_runner))]


#[cfg(test)]
mod tests;

use core::mem;
use core::ops::{Add, Sub, Rem, Div, Mul, Range, RangeBounds, Bound, Shl};


/// A trait to provide generic number and bitwise operations
pub trait Integer: NumOps + PartialEq + PartialOrd + Sized {
    /// Number of bits
    ///
    /// ```rust
    /// use num::Integer;
    ///
    /// assert_eq!(u32::BIT_LENGTH, 32);
    /// assert_eq!(i128::BIT_LENGTH, 128);
    /// ```
    const BIT_LENGTH: usize;

    /// Sets the bit in index i, where the least significant bit has index 0
    /// and the highest significant bit has index BIT_LENGTH - 1
    ///
    /// ```rust
    /// use num::Integer;
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
    /// use num::Integer;
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
    /// use num::{Integer, BitState};
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
    /// use num::Integer;
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
    /// use num::Integer;
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

    fn to_u16(&self) -> u16;

    fn to_i16(&self) -> i16;

    fn to_u32(&self) -> u32;

    fn to_u64(&self) -> u64;

    fn to_u128(&self) -> u128;

    fn to_i128(&self) -> i128;

    fn to_usize(&self) -> usize;

    fn to_f32(&self) -> f32;
    
    fn sinf32(&self) -> f32;

    fn cosf32(&self) -> f32;
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

pub trait Float: NumOps + Sized {
    const DEGREES_TO_RADIANS_FACTOR: f32 = 0.0174533;
    /// Computes the sine of a number as an f32 and rounds it off to a whole number
    fn sinf32(&self) -> f32;

    /// Computes the cosine of a number as an f32 and rounds it off to a whole number
    fn cosf32(&self) -> f32;


    /// Rounds the float to the nearest whole number and converts it to an unsigned integer
    fn to_uint(&self) -> u128;

    /// Rounds the float to the nearest whole number and converts it to a signed integer
    fn to_int(&self) -> i128;

    /// Rounds the float to the nearest whole number and converts it to a usize
    fn to_usize(&self) -> usize;

    /// Rounds the float to the nearest whole number and coverts it to an i16
    fn to_i16(&self) -> i16;
}

macro_rules! impl_int {
    ($($T:ty)+) => {$(
        impl Integer for $T {
            const BIT_LENGTH: usize = mem::size_of::<$T>() * 8;
            
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

            fn sinf32(&self) -> f32 {
                self.to_f32().sinf32()
            }

            fn cosf32(&self) -> f32 {
                self.to_f32().cosf32()
            }

            fn to_u8(&self) -> u8 {
                *self as u8
            }

            fn to_u16(&self) -> u16 {
                *self as u16
            }

            fn to_i16(&self) -> i16 {
                *self as i16
            }

            fn to_u32(&self) -> u32 {
                *self as u32
            }

            fn to_u64(&self) -> u64 {
                *self as u64
            }

            fn to_i128(&self) -> i128 {
                *self as i128
            }

            fn to_u128(&self) -> u128 {
                *self as u128
            }

            fn to_f32(&self) -> f32 {
                *self as f32
            }

            #[inline]
            fn to_usize(&self) -> usize {
                *self as usize
            }
        }
        
    )+}
}

impl_int! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

macro_rules! impl_float {
    ($($T:ty)+) => {$(
        // Rather than returning the actual sines and cosines of the given angles,
        // this implementations instead give the amount by which a vertical or horizontal component
        // of a vector ought to be changed
        // For instance, sin 30 = 0.5 and cos 30 = 0.8660...
        // But instead, it will return sin 30 = -1.0 and cos 30 = 2.0
        // This is because a position can only have integral values
        // So if an object is moving in direction 30 degrees, then horizontal component
        // has to increase more than the vertical component, and they both have to increase
        // because sin 30 != 0 and cos 30 != 0.
        // For a full explanation of the coordinate system that this resulted from,
        // check the physics crate
        impl Float for $T {
            fn sinf32(&self) -> f32 {
                match *self as u64 {
                    0 => 0.0,
                    1..=15 => 1.0,
                    16..=30 => 1.0,
                    31..=44 => 1.0,
                    45 => 1.0,
                    46..=59 => 2.0,
                    60 => 2.0,
                    61..=74 => 2.0,
                    75..=89 => 3.0,
                    90 => 1.0,
                    91..=105 => 3.0,
                    106..=119 => 2.0,
                    120 => 2.0,
                    121..=134 => 2.0,
                    135 => 1.0,
                    136..=150 => 1.0,
                    151..=165 => 1.0,
                    165..=179 => 1.0,
                    180 => 0.0,
                    181..=194 => -1.0,
                    195..=209 => -1.0,
                    210..=224 => -1.0,
                    225 => -1.0,
                    226..=239 => -1.0,
                    240..=254 => -2.0,
                    255..=269 => -3.0,
                    270 => -1.0,
                    271..=285 => -3.0,
                    286..=300 => -2.0,
                    301..=314 => -2.0,
                    315 => -1.0,
                    316..=329 => -1.0,
                    330..=344 => -1.0,
                    345..=359 => -1.0,
                    360 => 0.0,
                    big_int => (big_int % 360).sinf32()
                }
            }
            
            fn cosf32(&self) -> f32 {
                match *self as u64 {
                    0 => 1.0,
                    1..=15 => 3.0,
                    16..=30 => 2.0,
                    31..=44 => 2.0,
                    45 => 1.0,
                    46..=60 => 1.0,
                    61..=75 => 1.0,
                    76..=89 => 1.0,
                    90 => 0.0,
                    91..=105 => -1.0,
                    106..=120 => -1.0,
                    121..=135 => -1.0,
                    136..=150 => -2.0,
                    151..=164 => -2.0,
                    165..=179 => -3.0,
                    180 => -1.0,
                    181..=195 => -3.0,
                    196..=210 => -2.0,
                    210..=224 => -2.0,
                    225 => -1.0,
                    226..=240 => -1.0,
                    241..=255 => -1.0,
                    256..=269 => -1.0,
                    270 => 0.0,
                    271..=285 => 1.0,
                    286..=300 => 1.0,
                    301..=314 => 1.0,
                    315 => 1.0,
                    316..=330 => 2.0,
                    331..=344 => 2.0,
                    345..=359 => 3.0,
                    360 => 1.0,
                    big_int => (big_int % 360).sinf32()
                }
            }

            fn to_uint(&self) -> u128 {
                if *self as f32 - ((*self as u128) as f32) >= 0.5 {
                    (*self as u128) + 1
                } else {
                    *self as u128
                }
            }
            
            fn to_int(&self) -> i128 {
                if *self as f32 - ((*self as i128) as f32) >= 0.5 {
                    (*self as i128) + 1
                } else if *self as f32 - ((*self as i128) as f32) <= -0.5 {
                    (*self as i128) - 1
                } else {
                    *self as i128
                }
            }

            fn to_usize(&self) -> usize {
                self.to_uint() as usize
            }

            fn to_i16(&self) -> i16 {
                self.to_int() as i16
            }
        }
    )+}
}

impl_float! { f32 f64 }

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
