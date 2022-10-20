//! Abstractions for working with the 8259 Intel Programmable Interrupt Controllers

use crate::port::{Port, PortReadWrite};
use crate::port::consts::WAIT_PORT_NO;
use num::Integer;
use core::arch::asm;

/// Command issued at the end of an interrupt routine
const END_OF_INTERRUPT: u8 = 0x20;

/// Command to initialise a PIC
const CMD_INIT: u8 = 0x11;

/// The primary and secondary PICs port numbers
const PRIMARY_PIC_COMMAND_PORT: u16 = 0x20;
const PRIMARY_PIC_DATA_PORT: u16 = 0x21;
const SECONDARY_PIC_COMMAND_PORT: u16 = 0xa0;
const SECONDARY_PIC_DATA_PORT: u16 = 0xa1;

/// The mode the PICs will run in
const MODE_8086: u8 = 0x01;

/// The base IDT index number of the first PIC's IRQs
///
/// This number was specifically chosen so that the interrupts will
/// start immediately after the exceptions in the IDT
pub const PIC_1_OFFSET: u8 = 32;

/// The base IDT index number of the second PIC's IRQs
///
/// This number was specifically chosen so that the interrupts on the second
/// PIC will start immediately after the ones in the first PIC's interrupts in the IDT
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// A PIC
#[derive(Clone, Copy)]
struct Pic {
    /// The base index in the IDT that the PIC's interrupts are masked to
    offset: u8,
    /// PIC's command port
    command: Port<u8>,
    /// PIC's data port, for accessing the interrupt mask
    data: Port<u8>
}

/// The primary and secondary PICs
pub struct Pics {
    primary: Pic,
    secondary: Pic
}

impl Pics {

    /// Creates a new instance of the primary and secondary PICs
    pub const fn new() -> Pics {
        let primary = Pic {
            offset: PIC_1_OFFSET,
            command: Port::<u8>::new(PRIMARY_PIC_COMMAND_PORT),
            data: Port::<u8>::new(PRIMARY_PIC_DATA_PORT)
        };
        let secondary = Pic {
            offset: PIC_2_OFFSET,
            command: Port::<u8>::new(SECONDARY_PIC_COMMAND_PORT),
            data: Port::<u8>::new(SECONDARY_PIC_DATA_PORT)
        };
        Pics {
            primary,
            secondary
        }
    }

    /// Signifies the end of an interrupt routine to the PIC chips
    pub fn end_of_interrupt(&mut self, irq: u8) {
        if handles_interrupt(irq, self.primary) {
            self.primary.command.write(END_OF_INTERRUPT);
        } else if handles_interrupt(irq, self.secondary) {
            self.secondary.command.write(END_OF_INTERRUPT);
            self.primary.command.write(END_OF_INTERRUPT);
        }
    }
    
    /// Handles the remapping of the PICs to the offsets
    pub fn init(&mut self) {
        let mut wait_port: Port<u8> = Port::new(WAIT_PORT_NO);
        let mut wait = || wait_port.write(0);

        let mut x: u32;
    unsafe {
        asm!("
            mov ecx, 0x1b
            rdmsr
            mov edi, eax",
            out("edi") x
        );
    }
    x.unset_bit(11);
    unsafe {
        asm!("
            mov edx, 0
            mov eax, edi
            mov ecx, 0x1b
            wrmsr
        ", in("edi") x);
    }
        //writeln!(Printer, "{:x} {:x}", original_masks.0, original_masks.1);
        //loop {}

        // Start the initialization sequence by sending
        self.primary.command.write(CMD_INIT);
        wait();
        self.secondary.command.write(CMD_INIT);
        wait();
        
        // Setup base offsets
        self.primary.data.write(self.primary.offset);
        wait();
        self.secondary.data.write(self.secondary.offset);
        wait();

        // Tell primary that there is a secondary PIC at IRQ 2
        self.primary.data.write(4);
        wait();

        // Tell the secondary PIC it's cascade identity
        self.secondary.data.write(2);
        wait();

        // Set the mode
        self.primary.data.write(MODE_8086);
        wait();
        self.secondary.data.write(MODE_8086);
        wait();

        // Receive interrupts from only the keyboard
        // and interrupt line 11, which is being used for sound in this project
        self.write_masks(0b11111_0_0_1, 0b1111_0111);
    }

    /// Reads the interrupt masks of the PICs
    pub fn read_masks(&self) -> (u8, u8) {
        (self.primary.data.read(), self.secondary.data.read())
    }
    
    /// Writes the PICs' interrupt masks
    pub fn write_masks(&mut self, primary_mask: u8, secondary_mask: u8) {
        self.primary.data.write(primary_mask);
        self.secondary.data.write(secondary_mask);
    }
}

/// The PIC pic handles the IRQ irq only if the irq is within range of the PIC's numbers
fn handles_interrupt(irq: u8, pic: Pic) -> bool {
    pic.offset <= irq && pic.offset + 8 > irq
}





/*
const FONT_WIDTH: usize = 8;
const FONT_HEIGHT: usize = 8;
const SCREEN_WIDTH: usize = 640;
const SCREEN_HEIGHT: usize = 480;
use crate::font;
const X_SCALE: usize = SCREEN_WIDTH / 320;
const Y_SCALE: usize = SCREEN_HEIGHT / 200;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt;
pub struct Printer;
impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            Printer.print_char(c);
        }
        Ok(())
    }
}

static X_POS: AtomicUsize = AtomicUsize::new(0);
static Y_POS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy)]
#[repr(C)]
struct Color {
    blue: u8,
    green: u8,
    red: u8,
    reserved: u8
}
/*
#[derive(Clone, Copy)]
#[repr(transparent)]
struct Color(u8);
*/
use core::fmt::Write;
impl Printer {
    pub fn print_char(&mut self, c: u8) {
        /*let mut vga = 0xa0000 as *mut Color;
        let back = Color(0);
        let fore = Color(0xe);*/
        let mut vga = 0x80000000 as *mut Color;
        let back = Color {
            blue: 0,
            green: 0,
            red: 0,
            reserved: 0
        };
        let fore = Color {
            blue: 0,
            green: 255,
            red: 255,
            reserved: 0
        };
        let curr_x = X_POS.load(Ordering::Relaxed);
        let curr_y = Y_POS.load(Ordering::Relaxed);
        if c == b'\n' {
            X_POS.store(0, Ordering::Relaxed);
            let old_y = Y_POS.load(Ordering::Relaxed);
            Y_POS.store(old_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                let i = y + 1;
                for yp in y * Y_SCALE..i*Y_SCALE {
                    for x in 0..FONT_WIDTH {
                        let j = x + 1;
                        for xp in x * X_SCALE..j * X_SCALE {
                            unsafe {
                                if byte & (1 << (FONT_WIDTH - x - 1)) == 0 {
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = back;
                                } else {
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = fore;
                                }
                            }
                        }
                    }
                }
            }
            X_POS.store(curr_x + FONT_WIDTH * X_SCALE, Ordering::Relaxed);
            if X_POS.load(Ordering::Relaxed) >= SCREEN_WIDTH {
                X_POS.store(0, Ordering::Relaxed);
                Y_POS.store(curr_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
            }
        } else {
            self.print_char(b'?');
        }
    }
}
*/
pub fn is_printable_ascii(c: u8) -> bool {
    match c {
        b' '..=b'~' => true,
        _ => false
    }
}
