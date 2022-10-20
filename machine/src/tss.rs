//! Abstractions for dealing with the Task State Segment structure
#![allow(unaligned_references)]

use core::{mem, fmt};
use core::arch::asm;
use crate::memory::Addr;
use crate::gdt::SegmentSelector;

/// A structure for storing tables used for switching stacks during exceptions
///
/// https://wiki.osdev.org/Task_State_Segment
///
/// https://en.wikipedia.org/wiki/Task_state_segment
#[repr(C, packed(4))]
pub struct TaskStateSegment {
    reserved1: u32,
    /// Stack pointers for different privilege levels
    pub privilege_stack_table: [Addr; 3],
    reserved2: u64,
    /// Stack pointers for switching stacks when an entry in the IDT has IST other than 0
    pub interrupt_stack_table: [Addr; 7],
    reserved3: u64,
    reserved4: u16,
    /// Offset from the base of the TSS to I/O Permission Bit Map
    pub io_map_base_addr: u16
}

impl TaskStateSegment {
    
    /// Creates a new TSS with IST and PST all init to 0 and an empty I/O Permission Bit Map
    #[inline]
    pub fn new() -> Self {
        Self {
            privilege_stack_table: [Addr::new(0); 3],
            interrupt_stack_table: [Addr::new(0); 7],
            io_map_base_addr: (mem::size_of::<TaskStateSegment>()) as u16,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            reserved4: 0
        }
    }
}

impl fmt::Debug for TaskStateSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskStateSegment")
            .field("privilege_stack_table", &self.privilege_stack_table)
            .field("interrupt_stack_table", &self.interrupt_stack_table)
            .field("io_map_base_addr", &self.io_map_base_addr)
            .finish()
    }
}

#[inline]
pub unsafe fn load_tss(sel: SegmentSelector) {
    asm!("ltr {0:x}", in(reg) sel.0, options(nostack, preserves_flags));
}