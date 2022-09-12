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
    pub const fn new(primary_offset: u8, secondary_offset: u8) -> Pics {
        let primary = Pic {
            offset: primary_offset,
            command: Port::<u8>::new(PRIMARY_PIC_COMMAND_PORT),
            data: Port::<u8>::new(PRIMARY_PIC_DATA_PORT)
        };
        let secondary = Pic {
            offset: secondary_offset,
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
        // Saving original interrupt masks
        let original_masks = self.read_masks();
         
        
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

        // Restore the masks
        self.write_masks(original_masks.0, original_masks.1);
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