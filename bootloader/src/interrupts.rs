use machine::interrupts::{InterruptDescriptorTable, InterruptStackFrame, IRQ};
use machine::pic8259::{Pics, PIC_1_OFFSET};
use machine::instructions::interrupts::{enable as enable_interrupts, disable as disable_interrupts};
use machine::keyboard::Keyboard;
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use event_hook::Event;
use event_hook;
use crate::gdt::DOUBLE_FAULT_IST_INDEX;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.double_fault.set_handler(double_fault_handler)
            .set_ist_stack_index(DOUBLE_FAULT_IST_INDEX);
        idt.page_fault.set_handler(page_fault_handler);
        idt.general_protection_fault.set_handler(general_protection_fault_handler);
        idt.brkpoint.set_handler(brkpoint_interrupt_handler);
        idt[IRQ::Timer].set_handler(timer_interrupt_handler);
        idt[IRQ::Keyboard].set_handler(keyboard_interrupt_handler);
        idt[IRQ::Sound].set_handler(sound_interrupt_handler);
        idt
    };
}

pub static PICS: Mutex<Pics> = Mutex::new(Pics::new());

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard> = Mutex::new(Keyboard::new());
}

pub fn init(){
    disable_interrupts();
    IDT.load();
    PICS.lock().init();
    event_hook::init();
    enable_interrupts();
}

extern "x86-interrupt" fn brkpoint_interrupt_handler(_sf: InterruptStackFrame) {
    panic!("In the breakpoint");
}

extern "x86-interrupt" fn page_fault_handler(sf: InterruptStackFrame, err_code: u64) {
    panic!("Page Fault\nErr Code: {}\n{:?}", err_code, sf);
}

extern "x86-interrupt" fn double_fault_handler(sf: InterruptStackFrame, err_code: u64) -> ! {
    panic!("Double Fault\nErr Code: {}\n{:?}", err_code, sf);
}

extern "x86-interrupt" fn timer_interrupt_handler(_sf: InterruptStackFrame) {
    event_hook::send_event(Event::Timer);
    PICS.lock().end_of_interrupt(IRQ::Timer.as_u8() + PIC_1_OFFSET)
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_sf: InterruptStackFrame) {
    use machine::port::{Port, PortReadWrite};
    let port: Port<u8> = Port::new(0x60);
    let scancode: u8 = port.read();
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(event)) = keyboard.process_byte(scancode) {
        event_hook::send_event(Event::Keyboard(event.keycode, event.direction, event.key_modifiers));
    }
    PICS.lock().end_of_interrupt(IRQ::Keyboard.as_u8() + PIC_1_OFFSET)
}

extern "x86-interrupt" fn sound_interrupt_handler(_sf: InterruptStackFrame) {
    event_hook::send_event(Event::Sound);
    PICS.lock().end_of_interrupt(IRQ::Sound.as_u8() + PIC_1_OFFSET)
}

extern "x86-interrupt" fn general_protection_fault_handler(sf: InterruptStackFrame, err_code: u64) {
    panic!("General Protection Fault\nErr Code: {}\n{:?}", err_code, sf);
}