//! Abstractions for dealing with I/O ports

use core::arch::asm;
use core::marker::PhantomData;

/// An I/O port
#[derive(Clone, Copy)]
pub struct Port<T>(u16, PhantomData<T>);

impl<T> Port<T> {

    /// Creates a new I/O port with the given port number
    pub const fn new(port: u16) -> Port<T> {
        Port(port, PhantomData)
    }
}

pub trait PortReadWrite {
    type T;
    /// Reads the value from the I/O port
    fn read(&self) -> Self::T;

    /// Writes a value to a port
    fn write(&mut self, value: Self::T);
}

impl PortReadWrite for Port<u8> {
    type T = u8;
    fn read(&self) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") self.0, options(nomem, nostack, preserves_flags));
        }
        value
    }

    fn write(&mut self, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") self.0, in("al") value, options(nomem, nostack, preserves_flags));
        }
    }
}

impl PortReadWrite for Port<u16> {
    type T = u16;
    fn read(&self) -> u16 {
        let value: u16;
        unsafe {
            asm!("in ax, dx", out("ax") value, in("dx") self.0, options(nomem, nostack, preserves_flags));
        }
        value
    }

    fn write(&mut self, value: u16) {
        unsafe {
            asm!("out dx, ax", in("dx") self.0, in("ax") value, options(nomem, nostack, preserves_flags));
        }
    }
}