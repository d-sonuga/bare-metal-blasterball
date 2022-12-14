//! Abstractions for dealing with the x86_64 machine

#![cfg_attr(not(test), no_std)]
#![feature(abi_x86_interrupt, array_windows, abi_efiapi)]
#![allow(dead_code)]

pub mod interrupts;
pub mod memory;
pub mod tss;
pub mod gdt;
pub mod port;
pub mod pic8259;
pub mod instructions;
pub mod registers;
pub mod power;
pub mod cmos;
pub mod uefi;
pub mod keyboard;
pub mod acpi;
mod printer;
mod font;

use memory::Addr;

use sync::once::Once;

/// The address of the display screen memory map
pub static FRAMEBUFFER: Once<Addr> = Once::new();

/// A structure that is used to load a new Descriptor Table
#[repr(C, packed(2))]
pub struct DescriptorTablePointer {
    /// Size of the IDT - 1
    limit: u16,
    /// The base address of the IDT
    base: Addr
}