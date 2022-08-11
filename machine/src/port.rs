//! Abstractions for dealing with I/O ports

use core::arch::asm;

/// An I/O port
#[derive(Clone, Copy)]
pub struct Port(u16);

impl Port {

    /// Creates a new I/O port with the given port number
    pub const fn new(port: u16) -> Port {
        Port(port)
    }

    /// Reads the value from the I/O port
    pub fn read(&self) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") self.0, options(nomem, nostack, preserves_flags));
        }
        value
    }

    /// Writes a value to a port
    pub fn write(&mut self, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") self.0, in("al") value, options(nomem, nostack, preserves_flags));
        }
    }
}