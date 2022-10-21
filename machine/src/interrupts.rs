//! Abstractions for working with interrupts


use core::marker::PhantomData;
use core::mem;
use core::arch::asm;
use core::fmt;
use core::ops::{Index, IndexMut};
use crate::memory::Addr;
use crate::DescriptorTablePointer;
use num::Integer;

/// The number of none exception entries in the IDT
const NO_OF_INTERRUPTS: usize = 224;

#[repr(u8)]
pub enum CPUException {
    DivideByZero                = 0x0,
    Debug                       = 0x1,
    NonMaskableInterrupt        = 0x2,
    Breakpoint                  = 0x3,
    Overflow                    = 0x4,
    BoundRangeExceeded          = 0x5,
    InvalidOpcode               = 0x6,
    DeviceNotAvailable          = 0x7,
    DoubleFault                 = 0x8,
    InvalidTss                  = 0xa,
    SegmentNotPresent           = 0xb,
    StackSegmentFault           = 0xc,
    GeneralProtectionFault      = 0xd,
    PageFault                   = 0xe,
    X87FloatingPoint            = 0x10,
    AlignmentCheck              = 0x11,
    MachineCheck                = 0x12,
    SIMDFloatingPoint           = 0x13,
    Virtualization              = 0x14,
    ControlProtection           = 0x15,
    HypervisorInjection         = 0x1c,
    VMMCommunication            = 0x1d,
    Security                    = 0x1e
}

/// Representation of an entry in the IDT
#[derive(Clone, Copy)]
#[repr(C)]
pub struct IDTEntry<F> {
    /// The lower 16 bits of the handler's address
    handler_ptr_low: u16,
    /// The selector into the GDT
    pub gdt_selector: u16,
    pub options: IDTEntryOptions,
    /// The next 16 bits of the handler's address
    handler_ptr_middle: u16,
    /// The upper 32 bits of the handler's address
    handler_ptr_high: u32,
    /// Empty space
    reserved: u32,
    phantom_data: PhantomData<F>
}

impl<F> IDTEntry<F> {
    pub fn empty() -> IDTEntry<F> {
        IDTEntry {
            handler_ptr_low: 0,
            gdt_selector: 0,
            options: IDTEntryOptions::new(),
            handler_ptr_middle: 0,
            handler_ptr_high: 0,
            reserved: 0,
            phantom_data: PhantomData
        }
    }

    pub fn set_handler_addr(&mut self, handler: Addr){
        let handler = handler.as_u64();
        self.handler_ptr_low = handler as u16;
        self.handler_ptr_middle = (handler >> 16) as u16;
        self.handler_ptr_high = (handler >> 32) as u32;
        self.gdt_selector = segment::CS::get_reg().0;
        self.options.set_present(true);
    }

    pub fn set_ist_stack_index(&mut self, index: u16) {
        self.options.set_ist_stack_index(index)
    }

}

impl<F> fmt::Debug for IDTEntry<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("IDTEntry")
            .field("handler_ptr_low", &self.handler_ptr_low)
            .field("handler_ptr_middle", &self.handler_ptr_middle)
            .field("handler_ptr_high", &self.handler_ptr_high)
            .field("options", &self.options)
            .finish()
    }
}

/// A representation of the options in an IDT entry
///
/// # References
///
/// * <https://os.phil-opp.com/cpu-exceptions/>
#[derive(Clone, Copy)]
#[repr(C)]
pub struct IDTEntryOptions(u16);

impl IDTEntryOptions {
    pub fn new() -> IDTEntryOptions {
        IDTEntryOptions(0b1110_0000_0000)
    }

    fn set_present(&mut self, present: bool){
        if present {
            self.0.set_bit(15);
        } else {
            self.0.unset_bit(15);
        }
    }

    /// Sets an IST stack to the handler
    pub fn set_ist_stack_index(&mut self, index: u16) {
        // Hardware IST index is 1-based, that is, starts at 1
        self.0.set_bits(0..3, index + 1);
    }
}

impl fmt::Debug for IDTEntryOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IDTEntryOptions({:#x})", self.0)
    }
}

/// A representation of the Interrupt Descriptor Table
///
/// The CPU uses this to find interrupt service routines and handle exceptions
///
/// # References
///
/// * <https://wiki.osdev.org/Interrupt_Descriptor_Table>
/// * <https://wiki.osdev.org/Exceptions>
#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    pub div_by_zero: IDTEntry<Handler>,
    pub debug: IDTEntry<Handler>,
    pub non_maskable_interrupt: IDTEntry<Handler>,
    pub brkpoint: IDTEntry<Handler>,
    pub overflow: IDTEntry<Handler>,
    pub bound_range_exceeded: IDTEntry<Handler>,
    pub invalid_opcode: IDTEntry<Handler>,
    pub device_not_available: IDTEntry<Handler>,
    pub double_fault: IDTEntry<HandlerOfNoReturn>,
    pub coprocessor_segement_overrun: IDTEntry<Handler>,
    pub invalid_tss: IDTEntry<HandlerWithErrCode>,
    pub segment_not_present: IDTEntry<HandlerWithErrCode>,
    pub stack_segment_fault: IDTEntry<HandlerWithErrCode>,
    pub general_protection_fault: IDTEntry<HandlerWithErrCode>,
    pub page_fault: IDTEntry<HandlerWithErrCode>,
    reserved1: IDTEntry<Handler>,
    pub x87_floating_point_exception: IDTEntry<Handler>,
    pub alignment_check: IDTEntry<HandlerWithErrCode>,
    pub machine_check: IDTEntry<HandlerOfNoReturn>,
    pub simd_floating_point_exception: IDTEntry<Handler>,
    pub virtualization_exception: IDTEntry<Handler>,
    reserved2: [IDTEntry<Handler>; 8],
    pub vmm_communication_exception: IDTEntry<HandlerWithErrCode>,
    pub security_exception: IDTEntry<HandlerWithErrCode>,
    reserved3: IDTEntry<Handler>,
    pub interrupts: [IDTEntry<Handler>; NO_OF_INTERRUPTS]
}

impl InterruptDescriptorTable {
    pub fn new() -> InterruptDescriptorTable {
        InterruptDescriptorTable {
            div_by_zero: IDTEntry::empty(),
            debug: IDTEntry::empty(),
            non_maskable_interrupt: IDTEntry::empty(),
            brkpoint: IDTEntry::empty(),
            overflow: IDTEntry::empty(),
            bound_range_exceeded: IDTEntry::empty(),
            invalid_opcode: IDTEntry::empty(),
            device_not_available: IDTEntry::empty(),
            double_fault: IDTEntry::empty(),
            coprocessor_segement_overrun: IDTEntry::empty(),
            invalid_tss: IDTEntry::empty(),
            segment_not_present: IDTEntry::empty(),
            stack_segment_fault: IDTEntry::empty(),
            general_protection_fault: IDTEntry::empty(),
            page_fault: IDTEntry::empty(),
            reserved1: IDTEntry::empty(),
            x87_floating_point_exception: IDTEntry::empty(),
            alignment_check: IDTEntry::empty(),
            machine_check: IDTEntry::empty(),
            simd_floating_point_exception: IDTEntry::empty(),
            virtualization_exception: IDTEntry::empty(),
            reserved2: [IDTEntry::empty(); 8],
            vmm_communication_exception: IDTEntry::empty(),
            security_exception: IDTEntry::empty(),
            reserved3: IDTEntry::empty(),
            interrupts: [IDTEntry::empty(); NO_OF_INTERRUPTS]
        }
    }

    /// Load the IDT with the lidt instruction
    /// 'static to prevent a situation where the IDT is dropped while it is still in use
    pub fn load(&'static self){
        unsafe {
            asm!("
                lidt [{}]",
                in(reg) &self.as_pointer(), options(readonly, nostack, preserves_flags)
            );
        }
    }

    fn as_pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            limit: (mem::size_of::<Self>() - 1) as u16,
            base: Addr::new(self as *const _ as u64)
        }
    }
}

impl Index<usize> for InterruptDescriptorTable {
    type Output = IDTEntry<Handler>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        match index {
            i @ 32..=255 => &self.interrupts[i - 32],
            i => panic!("index for entry {} has not been implemented", i)
        }
    }
}

impl IndexMut<usize> for InterruptDescriptorTable {

    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            i @ 32..=255 => &mut self.interrupts[i - 32],
            i => panic!("index mut for entry {} has not been implemented", i)
        }
    }
}

impl Index<IRQ> for InterruptDescriptorTable {
    type Output = IDTEntry<Handler>;

    fn index(&self, idx: IRQ) -> &Self::Output {
        &self.interrupts[idx.as_u8().as_usize()]
    }
}

impl IndexMut<IRQ> for InterruptDescriptorTable {
    fn index_mut(&mut self, idx: IRQ) -> &mut Self::Output {
        &mut self.interrupts[idx.as_u8().as_usize()]
    }
}

macro_rules! impl_set_handler {
    ($handler_type:ty) => {
        impl IDTEntry<$handler_type> {
            pub fn set_handler(&mut self, handler: $handler_type) -> &mut Self {
                let handler_addr = Addr::new(handler as u64);
                self.set_handler_addr(handler_addr);
                self
            }
        }
    }
}

impl_set_handler!(Handler);
impl_set_handler!(HandlerWithErrCode);
impl_set_handler!(HandlerOfNoReturn);

pub type Handler = extern "x86-interrupt" fn(InterruptStackFrame);

pub type HandlerWithErrCode = extern "x86-interrupt" fn(InterruptStackFrame, u64);

pub type HandlerOfNoReturn = extern "x86-interrupt" fn(InterruptStackFrame, u64) -> !;


/// The values pushed on the stack by the CPU during an interrupt or exception
///
/// # References
///
/// * <https://wiki.osdev.org/Interrupt_Service_Routines>
#[repr(C)]
pub struct InterruptStackFrame {
    /// The instruction following the last executed instruction before the exception | interrupt
    pub original_instr_ptr: Addr,
    /// The CS selector padded to become 4 bytes
    pub code_segment: u64,
    /// The flags register at the moment of the interrupt | exception
    pub flags: u64,
    /// The stack pointer at the moment of the interrupt | exception
    pub original_stack_ptr: Addr,
    /// The SS selector at the moment of the interrupt | exception
    pub stack_segment: u64,
}

impl fmt::Debug for InterruptStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InterruptStackFrame")
            .field("original_instr_ptr", &self.original_instr_ptr)
            .field("code_segment", &self.code_segment)
            .field("flags", &self.flags)
            .field("original_stack_ptr", &self.original_stack_ptr)
            .field("stack_segment", &self.stack_segment)
            .finish()
    }
}

/// An index into the IDT specifically for regular interrupts and not exceptions
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum IRQ {
    /// This is a hardcoded value for the PIC
    Timer = 0,
    /// This is a hardcoded value for the PIC
    Keyboard = 1,
    /// According to the info gotten from <https://os.phil-opp.com/hardware-interrupts/>,
    /// interrupt line 11 is generally available, so it is used for sound in this
    /// project
    Sound = 11
}

impl IRQ {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

mod segment {
    use core::arch::asm;

    pub struct CS;

    impl CS {
        pub fn get_reg() -> SegmentSelector {
            let segment: u16;
            unsafe { asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack, preserves_flags)) };
            SegmentSelector(segment)
        }
    }

    /// A index into a GDT or LDT table
    #[repr(transparent)]
    pub struct SegmentSelector(pub u16);

}