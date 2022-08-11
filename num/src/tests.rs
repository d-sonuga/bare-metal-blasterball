use crate::{Num, BitState};

#[test_case]
fn test_bit_lengths(){
    assert_eq!(u8::BIT_LENGTH, 8);
    assert_eq!(u16::BIT_LENGTH, 16);
    assert_eq!(u32::BIT_LENGTH, 32);
    assert_eq!(u64::BIT_LENGTH, 64);
    assert_eq!(u128::BIT_LENGTH, 128);

    assert_eq!(i8::BIT_LENGTH, 8);
    assert_eq!(i16::BIT_LENGTH, 16);
    assert_eq!(i32::BIT_LENGTH, 32);
    assert_eq!(i64::BIT_LENGTH, 64);
    assert_eq!(i128::BIT_LENGTH, 128);
}

#[test_case]
fn test_get_set_unset_bit(){
    for i in 0..8 {
        let mut n = 0u8;
        n.set_bit(i);
        assert_eq!(n, 1 << i);
        assert_eq!(BitState::Set, n.get_bit(i));
        n.unset_bit(i);
        assert_eq!(BitState::Unset, n.get_bit(i));
    }
}

#[test_case]
fn test_get_set_bits(){
    let mut n = 0u16;
    n.set_bits(0..3, 0b101);
    assert_eq!(n, 0b101);
    assert_eq!(0b101, n.get_bits(0..3));

    let mut n = 0b11011u32;
    assert_eq!(0b110, n.get_bits(2..5));
}