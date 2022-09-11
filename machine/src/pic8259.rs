//! Abstractions for working with the 8259 Intel Programmable Interrupt Controllers

use crate::port::{Port, PortReadWrite};

/// Command issued at the end of an interrupt routine
const END_OF_INTERRUPT: u8 = 0x20;

/// Command to initialise a PIC
const CMD_INIT: u8 = 0x11;

/// The master and slave PICs port numbers
const MASTER_PIC_COMMAND_PORT: u16 = 0x20;
const MASTER_PIC_DATA_PORT: u16 = 0x21;
const SLAVE_PIC_COMMAND_PORT: u16 = 0xa0;
const SLAVE_PIC_DATA_PORT: u16 = 0xa1;

/// A port used to put garbage for waiting
const WAIT_PORT: u16 = 0x80;

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

/// The master and slave PICs
pub struct Pics {
    master: Pic,
    slave: Pic
}

impl Pics {

    /// Creates a new instance of the master and slave PICs
    pub const fn new(master_offset: u8, slave_offset: u8) -> Pics {
        let master = Pic {
            offset: master_offset,
            command: Port::<u8>::new(MASTER_PIC_COMMAND_PORT),
            data: Port::<u8>::new(MASTER_PIC_DATA_PORT)
        };
        let slave = Pic {
            offset: slave_offset,
            command: Port::<u8>::new(SLAVE_PIC_COMMAND_PORT),
            data: Port::<u8>::new(SLAVE_PIC_DATA_PORT)
        };
        Pics {
            master,
            slave
        }
    }

    /// Signifies the end of an interrupt routine to the PIC chips
    pub fn end_of_interrupt(&mut self, irq: u8) {
        if handles_interrupt(irq, self.master) {
            self.master.command.write(END_OF_INTERRUPT);
        } else if handles_interrupt(irq, self.slave) {
            self.slave.command.write(END_OF_INTERRUPT);
            self.master.command.write(END_OF_INTERRUPT);
        }
    }
    
    /// Handles the remapping of the PICs to the offsets
    pub fn init(&mut self) {
        // Saving original interrupt masks
        let original_masks = self.read_masks();
         
        
        let mut wait_port: Port<u8> = Port::new(WAIT_PORT);
        let mut wait = || wait_port.write(0);

        // Start the initialization sequence by sending
        self.master.command.write(CMD_INIT);
        wait();
        self.slave.command.write(CMD_INIT);
        wait();
        
        // Setup base offsets
        self.master.data.write(self.master.offset);
        wait();
        self.slave.data.write(self.slave.offset);
        wait();

        // Tell master that there is a slave PIC at IRQ 2
        self.master.data.write(4);
        wait();

        // Tell the slave PIC it's cascade identity
        self.slave.data.write(2);
        wait();

        // Set the mode
        self.master.data.write(MODE_8086);
        wait();
        self.slave.data.write(MODE_8086);
        wait();

        // Restore the masks
        self.write_masks(original_masks.0, original_masks.1);
    }

    /// Reads the interrupt masks of the PICs
    pub fn read_masks(&self) -> (u8, u8) {
        (self.master.data.read(), self.slave.data.read())
    }
    
    /// Writes the PICs' interrupt masks
    pub fn write_masks(&mut self, master_mask: u8, slave_mask: u8) {
        self.master.data.write(master_mask);
        self.slave.data.write(slave_mask);
    }
}

/// The PIC pic handles the IRQ irq only if the irq is within range of the PIC's numbers
fn handles_interrupt(irq: u8, pic: Pic) -> bool {
    pic.offset <= irq && pic.offset + 8 > irq
}