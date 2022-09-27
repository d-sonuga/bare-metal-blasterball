use core::arch::asm;
use num::Integer;
use crate::acpi::MADT;
use crate::printer::Printer;
use core::fmt::Write;

pub unsafe fn setup_apic(madt: &MADT) {
    /*let mut x: u32;
    unsafe {
        asm!("
            mov ecx, 0x1b
            rdmsr
            mov edi, eax",
            out("edi") x
        );
    }
    x.set_bit(11);
    unsafe {
        asm!("
            mov edx, 0
            mov eax, edi
            mov ecx, 0x1b
            wrmsr
        ", in("edi") x);
    }*/
    let sivr_val = read_reg(madt, 0xf0);
    writeln!(Printer, "{:x}", sivr_val);
    loop {}
    write_reg(madt, 0xf0, sivr_val | 0x100 | 0xff);

}

unsafe fn write_reg(madt: &MADT, reg_no: u8, val: u32) {
    let base_addr = madt.local_interrupt_controller_addr();
    let reg_addr = base_addr + reg_no as u32;
    let ptr = reg_addr as *mut u32;
    ptr.write(val);
}

unsafe fn read_reg(madt: &MADT, reg_no: u8) -> u32 {
    let base_addr = madt.local_interrupt_controller_addr();
    let reg_addr = base_addr + reg_no as u32;
    let ptr = reg_addr as *mut u32;
    ptr.read()
}