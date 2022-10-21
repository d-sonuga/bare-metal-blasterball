//! Abstractions for dealing with the Global Descriptor Table structure

use core::{fmt, mem};
use core::arch::asm;
use num::Integer;
use crate::{DescriptorTablePointer, tss::TaskStateSegment, Addr};

/// A self-imposed limit on the number of entries that should be in the GDT
const GDT_MAX_ENTRY_SIZE: usize = 8;

/// A structure containing entries of memory segments
///
/// # References
///
/// * <https://wiki.osdev.org/Global_Descriptor_Table>
/// * <https://en.wikipedia.org/wiki/Global_Descriptor_Table>
#[repr(C)]
pub struct GlobalDescriptorTable {
    entries: [u64; GDT_MAX_ENTRY_SIZE],
    next_index: usize
}

impl GlobalDescriptorTable {

    /// Creates a new GDT, initializing all entries to the null descriptor
    pub fn new() -> Self {
        Self {
            entries: [0u64; GDT_MAX_ENTRY_SIZE],
            next_index: 1
        }
    }

    pub fn add_entry(&mut self, descriptor: Descriptor) -> SegmentSelector {
        match descriptor {
            Descriptor::SystemSegment(lower, higher) => {
                if self.entries.len() - self.next_index < 2 {
                    panic!("Need 2 entries for another system segment");
                }
                self.entries[self.next_index] = lower;
                self.entries[self.next_index + 1] = higher;
                self.next_index += 2;
                SegmentSelector::new(self.next_index as u16 - 2)
            },
            Descriptor::NonSystemSegment(value) => {
                if self.next_index >= self.entries.len() {
                    panic!("Too many entries in the GDT");
                }
                self.entries[self.next_index] = value;
                self.next_index += 1;
                SegmentSelector::new(self.next_index as u16 - 1)
            }
        }
    }

    pub fn load(&'static self) {
        unsafe {
            asm!("lgdt [{}]", in(reg) &self.as_pointer(), options(readonly, nostack, preserves_flags));
        }
    }

    fn as_pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            base: Addr::new(self as *const _ as u64),
            limit: (mem::size_of::<Self>() - 1) as u16
        }
    }
}

/// Representation of a segment descriptor
pub enum Descriptor {
    /// A system segment descriptor like a TSS descriptor
    SystemSegment(u64, u64),
    /// A code or data segment descriptor
    NonSystemSegment(u64)
}

impl Descriptor {

    pub fn code_segment() -> Descriptor {
        Descriptor::NonSystemSegment(DescriptorFlags::CODE_SEGMENT)
    }

    pub fn data_segment() -> Descriptor {
        Descriptor::NonSystemSegment(DescriptorFlags::DATA_SEGMENT)
    }

    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        let tss_ptr = tss as *const _ as u64;
        let mut high = 0;
        high.set_bits(0..32, tss_ptr.get_bits(32..64));

        let mut low = DescriptorFlags::PRESENT;
        low.set_bits(16..40, tss_ptr.get_bits(0..24));
        low.set_bits(56..64, tss_ptr.get_bits(24..32));
        low.set_bits(0..16, (mem::size_of::<TaskStateSegment>() - 1) as u64);
        low.set_bits(40..44, 0b1001);
        Descriptor::SystemSegment(low, high)
    }
}

struct DescriptorFlags;

impl DescriptorFlags {

    const PRESENT: u64 = 1 << 47;
    const WRITABLE: u64 = 1 << 41;
    const ACCESSED: u64 = 1 << 40;
    const NON_SYSTEM_SEGMENT: u64 = 1 << 44;
    const LIMIT_0_15: u64 = 0xffff;
    const LIMIT_16_19: u64 = 0xffff;
    const BASE_0_23: u64 = 0xffffff << 16;
    const BASE_24_31: u64 = 0xff << 56;
    const GRANULARITY: u64 = 1 << 55;
    const DEFAULT_OP_SIZE: u64 = 1 << 54;
    const LONG_MODE: u64 = 1 << 53;
    const EXECUTABLE: u64 = 1 << 43;

    /// Bit flags that are set by all segments
    const USED_BY_ALL: u64 = Self::NON_SYSTEM_SEGMENT
        | Self::GRANULARITY
        | Self::LIMIT_0_15
        | Self::LIMIT_16_19
        | Self::ACCESSED
        | Self::PRESENT
        | Self::WRITABLE;
    
    const DATA_SEGMENT: u64 = Self::USED_BY_ALL | Self::DEFAULT_OP_SIZE;
    const CODE_SEGMENT: u64 = Self::USED_BY_ALL | Self::EXECUTABLE | Self::LONG_MODE;
    
}

/// Representation of a segment's offset into the GDT table
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct SegmentSelector(pub u16);

impl SegmentSelector {

    /// Creates a new SegmentSelector
    ///
    /// # Arguments
    ///  * `index`: index in the table entries array, not the offset
    fn new(index: u16) -> Self {
        Self(index * 8)
    }
}

impl fmt::Debug for SegmentSelector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SegmentSelector({:#x})", self.0)
    }
}

pub trait SegmentRegister {
    unsafe fn set(&self, selector: SegmentSelector);
}

/// Representation of the Code Segment register
pub struct CS;

impl SegmentRegister for CS {
    unsafe fn set(&self, selector: SegmentSelector) {
        asm!(
            "push {sel:r}",
            "lea {tmp}, [1f + rip]",
            "push {tmp}",
            "retfq",
            "1:",
            sel = in(reg) selector.0,
            tmp = lateout(reg) _,
            options(preserves_flags)
        );
    }
}

pub struct DS;

impl SegmentRegister for DS {
    unsafe fn set(&self, selector: SegmentSelector) {
        asm!("mov ds, ax", in("ax") selector.0);
    }
}

pub struct SS;

impl SegmentRegister for SS {
    unsafe fn set(&self, selector: SegmentSelector) {
        asm!("mov ss, ax", in("ax") selector.0);
    }
}