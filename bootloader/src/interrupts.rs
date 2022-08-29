use core::fmt::Write;
use machine::interrupts::{InterruptDescriptorTable, InterruptStackFrame};
use machine::pic8259::Pics;
use machine::instructions::interrupts::enable as enable_interrupts;
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use drivers::keyboard::Keyboard;
use event_hook::{EventHooker, Event};
use event_hook;
use printer::{println, print};
use crate::gdt::DOUBLE_FAULT_IST_INDEX;

/// The base IDT index number of the first PIC's IRQs
pub const PIC_1_OFFSET: u8 = 32;

/// The base IDT index number of the second PIC's IRQs
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.brkpoint.set_handler(brkpoint_handler);
        idt.double_fault.set_handler(double_fault_handler)
            .set_ist_stack_index(DOUBLE_FAULT_IST_INDEX);
        idt.general_protection_fault.set_handler(general_protection_fault_handler);
        idt[InterruptIndex::Timer.as_usize()].set_handler(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler(keyboard_interrupt_handler);
        idt
    };
}

pub static PICS: Mutex<Pics> = Mutex::new(Pics::new(PIC_1_OFFSET, PIC_2_OFFSET));

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard> = Mutex::new(Keyboard::new());
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

pub fn init(){
    IDT.load();
    PICS.lock().init();
    enable_interrupts();
}

extern "x86-interrupt" fn brkpoint_handler(stack_frame: InterruptStackFrame) {
    println!("A breakpoint: {:?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(sf: InterruptStackFrame, err_code: u64) {
    println!("greetings from the page fault handler");
    loop {}
}

extern "x86-interrupt" fn double_fault_handler(sf: InterruptStackFrame, err_code: u64) -> ! {
    println!("Double Fault\nErr Code: {}\n{:?}", err_code, sf);
    loop {}
}

extern "x86-interrupt" fn timer_interrupt_handler(sf: InterruptStackFrame) {
    event_hook::send_event(Event::Timer);
    unsafe { PICS.lock().end_of_interrupt(InterruptIndex::Timer.as_u8()) }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(sf: InterruptStackFrame) {
    use machine::port::Port;
    let port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(event)) = keyboard.process_byte(scancode) {
        event_hook::send_event(Event::Keyboard(event.keycode, event.direction, event.key_modifiers));
        //print!("{:?} {:?} {:?}", event.keycode, event.direction, event.key_modifiers);
    }
    unsafe { PICS.lock().end_of_interrupt(InterruptIndex::Keyboard.as_u8()) }
}

extern "x86-interrupt" fn general_protection_fault_handler(sf: InterruptStackFrame, err_code: u64) {
    println!("greetings from the general protection fault handler");
    loop {}
}