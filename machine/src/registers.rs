//! Abstractions for dealing with special registers

use core::arch::asm;

/// The flags register
pub struct RFlags(u64);

impl RFlags {
    /// For enabling interrupts
    pub const INTERRUPT_FLAG: u64 = 1 << 9;

    /// Creates a new RFlags instance containing the current value of the flags register
    pub fn read() -> RFlags {
        let value: u64;
        unsafe {
            asm!("pushfq; pop {}", out(reg) value, options(nomem, preserves_flags))
        }
        RFlags(value)
    }
    
    /// Checks if the bit set in flag is also set in the RFlags register
    pub fn contains(&self, flag: u64) -> bool {
        self.0 & flag != 0
    }
}