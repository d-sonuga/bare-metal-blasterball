//! Abstractions for using x86 instructions

use core::arch::asm;

/// Interrupt related instructions
pub mod interrupts {
    use super::*;

    /// Enable interrupts
    #[inline]
    pub fn enable() {
        unsafe {
            asm!("sti", options(nomem, nostack));
        }
    }

    /// Disable interrupts
    #[inline]
    pub fn disable() {
        unsafe {
            asm!("cli", options(nomem, nostack));
        }
    }

    /// Checks if interrupts are enabled
    pub fn is_enabled() -> bool {
        use crate::registers::RFlags;

        RFlags::read().contains(RFlags::INTERRUPT_FLAG)
    }

    /// Executes a closure with interrupts disabled
    #[inline]
    pub fn without_interrupts<F, R>(func: F) -> R
        where F: FnOnce() -> R 
    {
        let interrupts_originally_enabled = is_enabled();
        if interrupts_originally_enabled {
            disable();
        }
        let result = func();
        if interrupts_originally_enabled {
            enable();
        }
        result
    }

}
