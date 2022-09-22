use crate::port::{Port, PortReadWrite};
use crate::port::consts::WAIT_PORT_NO;

/// Gets the current time from CMOS registers
///
/// Register  Contents            Range
/// 0x00      Seconds             0–59
/// 0x02      Minutes             0–59
/// 0x04      Hours               0–23 in 24-hour mode, 
///                               1–12 in 12-hour mode, highest bit set if pm
/// 0x06      Weekday             1–7, Sunday = 1
/// 0x07      Day of Month        1–31
/// 0x08      Month               1–12
/// 0x09      Year                0–99
/// 0x32      Century (maybe)     19–20?
/// 0x0A      Status Register A
/// 0x0B      Status Register B
///
/// Reference: https://wiki.osdev.org/CMOS
pub fn get_current_time() -> RTCTime {
    let seconds = read_register(0x00);
    let minutes = read_register(0x02);
    let hours = read_register(0x04);
    let weekday = read_register(0x06);
    let day_of_month = read_register(0x07);
    let month = read_register(0x08);
    let year = read_register(0x09);
    RTCTime { year, month, day_of_month, weekday, hours, minutes, seconds }
}

/// Reads a CMOS register
///
/// Reference: https://wiki.osdev.org/CMOS#Accessing_CMOS_Registers
fn read_register(register_no: u8) -> usize {
    // A CMOS register is selected by writing the register number to port 0x70
    // The most significant bit of whichever register_no is written to port 0x70
    // controls the Non Maskable Interrupts (NMI)
    const NMI_ENABLED: u8 = 0x80;
    let mut port: Port<u8> = Port::new(0x70);
    // Selecting the register
    port.write(NMI_ENABLED | register_no);
    // wait
    let mut wait_port: Port<u8> = Port::new(WAIT_PORT_NO);
    wait_port.write(0);
    // Reading the value of the selected register
    let port: Port<u8> = Port::new(0x71);
    let val = port.read();
    val as usize
}

/// The time that is retrieved from the CMOS
#[derive(Debug)]
pub struct RTCTime {
    pub year: usize,
    pub month: usize,
    pub day_of_month: usize,
    pub weekday: usize,
    pub hours: usize,
    pub minutes: usize,
    pub seconds: usize
}

impl RTCTime {
    /// The sum of all the time fields in the struct
    ///
    /// Used for generating sufficiently "random" numbers in the game
    pub fn sum_of_fields(&self) -> usize {
        self.year + self.month + self.day_of_month + self.weekday + self.hours
        + self.minutes + self.seconds
    }
}