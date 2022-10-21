//! Abstractions for working with the 8259 Intel Programmable Interrupt Controllers

use crate::port::{Port, PortReadWrite};
use crate::port::consts::WAIT_PORT_NO;

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
        self.write_masks(0b11111_0_0_0, 0b1111_0111);
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

