use artist::{println, WriteTarget};
use machine::port::{Port, PortReadWrite};
use machine::interrupts::IRQ;
use crate::wav::WavFile;

#[link_section = ".sound"]
static MUSIC: [u8; 7287938] = *include_bytes!("./assets/canon-in-d-major.wav");
#[link_section = ".sound"]
static BOUNCE: [u8; 16140] = *include_bytes!("./assets/bounce.wav");
#[link_section = ".sound"]
static CLINK: [u8; 217536] = *include_bytes!("./assets/clink.wav");
#[link_section = ".sound"]
static DRUM: [u8; 734028] = *include_bytes!("./assets/drum.wav");

pub unsafe fn figure_out_how_to_make_sounds() {
    println!("So the rust compiler won't remove it, here's a byte {:x}", MUSIC[0]);
    let hda_bus_and_device_number_opt = find_hda_bus_and_device_number();
    if hda_bus_and_device_number_opt.is_none() {
        panic!("Didn't find the HDA");
    }
    let mut sound_device = hda_bus_and_device_number_opt.unwrap();
    sound_device.pci_config.set_interrupt_line(IRQ::Sound);
    let cap = sound_device.global_capabilities();
    println!("Num of output streams: {}", cap.num_of_output_streams());
    loop {}
}

struct HDAController;

impl HDAController {
    fn reset(&self) {

    }
}

/// Searches all buses on the PCI until it finds the HDA
///
/// # References
///
/// * https://wiki.osdev.org/PCI
/// * https://wiki.osdev.org/Intel_High_Definition_Audio#Identifying_HDA_on_a_machine
fn find_hda_bus_and_device_number() -> Option<SoundDevice> {
    for bus in 0..=255 {
        for device in 0..32 {
            for func in 0..8 {
                let pci_device = PCIDevice { bus, device, func };
                if pci_device.is_valid() {
                    // No vendor id is ever equal to 0xffff.
                    // According to the OSDev wiki, the best way to identify HDA is to look for
                    // the class code (0x4) and subclass (0x3)
                    if pci_device.classcode() == 0x4 && pci_device.subclass() == 0x3 {
                        return Some(SoundDevice::from(pci_device));
                    }
                }
            }
        }
    }
    None
}

/// A device on the PCI bus
///
/// It is assumed that the device has a PCI configuration header of type 0x0
///
/// # References
///
/// * The OSDev wiki <https://wiki.osdev.org/PCI>
struct PCIDevice {
    bus: u32,
    device: u32,
    func: u32
}

impl PCIDevice {
    // Register offsets for values in the PCI configuration header
    const DEVICE_AND_VENDOR_ID_OFFSET: u32 = 0x0;
    const CLASSCODE_AND_SUBCLASS_OFFSET: u32 = 0x8;
    const HEADER_TYPE_OFFSET: u32 = 0xc;
    const BAR0_OFFSET: u32 = 0x10;
    const INTERRUPT_PIN_LINE_OFFSET: u32 = 0x3c;

    /// This port is written to specify which configuration header of a PCI device
    /// should be read from the `DATA_PORT`
    const ADDR_PORT: u16 = 0xcf8;
    /// The data this port outputs is the data from the configuration header previously
    /// specified by writing to the `ADDR_PORT`
    const DATA_PORT: u16 = 0xcfc;

    /// Checks if the device specified by the bus, device and func numbers
    /// is a valid device on the PCI
    ///
    /// According to the OSDev wiki <https://wiki.osdev.org/PCI>, no valid device
    /// can have a vendor id of 0xfff
    fn is_valid(&self) -> bool {
        self.vendor_id() != 0xfff
    }

    /// Returns the device's vendor id from the PCI configuration header
    fn vendor_id(&self) -> u16 {
        let device_vendor_id_addr: u32 = self.reg_addr(Self::DEVICE_AND_VENDOR_ID_OFFSET);
        let mut address_port: Port<u32> = Port::new(Self::ADDR_PORT);
        address_port.write(device_vendor_id_addr);
        let data_port: Port<u32> = Port::new(Self::DATA_PORT);
        let val = data_port.read();
        val as u16
    }

    fn read_classcode_subclass_reg(&self) -> (u8, u8) {
        let mut address_port: Port<u32> = Port::new(Self::ADDR_PORT);
        let data_port: Port<u32> = Port::new(Self::DATA_PORT);
        let address: u32 = self.reg_addr(Self::CLASSCODE_AND_SUBCLASS_OFFSET);
        address_port.write(address);
        let val = data_port.read();
        let classcode = (val >> 24) as u8;
        let subclass = ((val >> 16) & 0xff) as u8;
        (classcode, subclass)
    }

    fn header_type(&self) -> PCIHeaderType {
        let (mut address_port, data_port) = self.ports();
        let address: u32 = self.reg_addr(Self::HEADER_TYPE_OFFSET);
        address_port.write(address);
        let val = data_port.read();
        let val = PCIHeaderTypeReg(((val >> 16) & 0xff) as u8);
        val.header_type().unwrap()
    }

    fn bar0(&self) -> BaseAddrReg {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let mut addr_port: Port<u32> = Port::new(Self::ADDR_PORT);
        let data_port: Port<u32> = Port::new(Self::DATA_PORT);
        let addr: u32 = self.reg_addr(Self::BAR0_OFFSET);
        addr_port.write(addr);
        let val = data_port.read();
        BaseAddrReg::try_from(val).unwrap()
    }

    fn size_of_addr_space_needed(&self) -> u32 {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let mut addr_port: Port<u32> = Port::new(Self::ADDR_PORT);
        let mut data_port: Port<u32> = Port::new(Self::DATA_PORT);
        let addr: u32 = self.reg_addr(Self::BAR0_OFFSET);
        addr_port.write(addr);
        let baddr_reg_val = data_port.read();
        data_port.write(u32::MAX);
        let new_val = data_port.read();
        let amount_of_mem_needed = (!new_val) + 1;
        data_port.write(baddr_reg_val);
        amount_of_mem_needed
    }

    fn interrupt_line(&self) -> u8 {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let (mut addr_port, data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::INTERRUPT_PIN_LINE_OFFSET);
        addr_port.write(reg_addr);
        data_port.read() as u8
    }

    fn set_interrupt_line(&mut self, line: IRQ) {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let (mut addr_port, mut data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::INTERRUPT_PIN_LINE_OFFSET);
        addr_port.write(reg_addr);
        data_port.write(line.as_u8() as u32);
    }

    /// Returns the device's class code read from the PCI configuration header
    fn classcode(&self) -> u8 {
        self.read_classcode_subclass_reg().0
    }

    fn subclass(&self) -> u8 {
        self.read_classcode_subclass_reg().1
    }

    /// Returns the address to be written into the `ADDR_PORT` to access
    /// the data in the configuration header at offset `reg_offset`
    fn reg_addr(&self, reg_offset: u32) -> u32 {
        self.bus << 16 
            | self.device << 11 | self.func << 8
            | (reg_offset & 0xfc) | 0x80000000u32
    }

    fn ports(&self) -> (Port<u32>, Port<u32>) {
        let addr_port: Port<u32> = Port::new(Self::ADDR_PORT);
        let data_port: Port<u32> = Port::new(Self::DATA_PORT);
        (addr_port, data_port)
    }
}

/// A HDA sound device on the PCI bus
struct SoundDevice {
    pci_config: PCIDevice
}

impl SoundDevice {
    // Register offsets
    const GLOBAL_CAPABILTIES_OFFSET: isize = 0x00;
    const CORBWP_OFFSET: isize = 0x48;

    /// Returns the pointer to the location of the device's
    /// memory mapped registers
    fn base_ptr(&self) -> *mut u8 {
        self.pci_config.bar0().addr() as *mut u8
    }

    /// Returns the pointer to the location of the register
    /// mapped to the memory location at an offset of `offset` from
    /// the base address
    fn reg_ptr(&self, offset: isize) -> *mut u8 {
        unsafe { self.base_ptr().offset(offset) }
    }
    /// Sets the device's CORBWP pointer to val, which points to
    /// the index of the last valid command in the CORB
    fn set_corbwp(&self, val: usize) {
        
    }

    fn global_capabilities(&self) -> HDAGlobalCapabilitiesReg {
        let ptr = self.reg_ptr(Self::GLOBAL_CAPABILTIES_OFFSET).cast::<u16>();
        unsafe { HDAGlobalCapabilitiesReg::from(ptr.read()) }
    }
}

impl From<PCIDevice> for SoundDevice {
    fn from(pci_device: PCIDevice) -> SoundDevice {
        SoundDevice { pci_config: pci_device }
    }
}

/// Indicates the capabilities of the HDA controller
#[derive(Clone, Copy)]
#[repr(transparent)]
struct HDAGlobalCapabilitiesReg(u16);

impl HDAGlobalCapabilitiesReg {
    /// A value of 0 indicates that no output streams
    /// are supported. The max value is 15
    fn num_of_output_streams(&self) -> u8 {
        (self.0 >> 12) as u8
    }
    /// A value of 0 indicates that no input streams are supported.
    /// The max value is 15
    fn num_of_input_streams(&self) -> u8 {
        ((self.0 >> 8) & 0b1111) as u8
    }
    /// A value of 0 indicates that no bi-directional streams
    /// are supported. The max value is 30
    fn num_of_bidirectional_streams(&self) -> u8 {
        ((self.0 >> 3) & 0b11111) as u8
    }
    /// A value of 0 indicates that 1 SDO is supported.
    /// A 1 indicates 2 are supported, 0b10 indicates 4 are supported
    /// and 0b11 is reserved
    fn num_of_serial_data_out_signals(&self) -> u8 {
        ((self.0 >> 1) & 0b11) as u8
    }
    /// Indicates whether or not 64 bit addressing is supported by the
    /// controller
    fn addr_64bit_supported(&self) -> bool {
        (self.0 & 0b1) == 1
    }
}

impl From<u16> for HDAGlobalCapabilitiesReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct PCIHeaderTypeReg(u8);

impl PCIHeaderTypeReg {
    fn has_multiple_funcs(&self) -> bool {
        self.0 >> 7 == 1
    }
    fn header_type(&self) -> Result<PCIHeaderType, &'static str> {
        match self.0 & 0b01111111 {
            0x0 => Ok(PCIHeaderType::Standard),
            0x1 => Ok(PCIHeaderType::PCIToPCIBridge),
            0x2 => Ok(PCIHeaderType::CardBusBridge),
            _ => Err("This header type register value has an unexpected header type number")
        }
    }
}

#[derive(Debug, PartialEq)]
#[repr(u8)]
enum PCIHeaderType {
    Standard = 0x0,
    PCIToPCIBridge = 0x1,
    CardBusBridge = 0x2
}

/// The memory/port address used by a PCI device for mapping
enum BaseAddrReg {
    Memory(MemBAR),
    IO(IOBAR)
}

impl BaseAddrReg {
    fn addr(&self) -> u32 {
        match self {
            Self::Memory(mbar) => mbar.addr(),
            Self::IO(iobar) => iobar.addr()
        }
    }
}

impl TryFrom<u32> for BaseAddrReg {
    type Error = &'static str;
    fn try_from(val: u32) -> Result<BaseAddrReg, Self::Error> {
        match val & 0x1 {
            0 => Ok(Self::Memory(MemBAR(val))),
            1 => Ok(Self::IO(IOBAR(val))),
            _ => Err("Expected either a 0 or 1 in bit 0")
        }
    }
}

struct MemBAR(u32);

impl MemBAR {
    /// Returns the 16 byte aligned base address
    fn addr(&self) -> u32 {
        self.0 >> 4 << 4
    }
}

struct IOBAR(u32);

impl IOBAR {
    /// Returns the 4 byte aligned base address
    fn addr(&self) -> u32 {
        self.0 >> 2 << 2
    }
}

enum BaseAddrRegisterKind {
    Mem,
    IO
}

#[repr(transparent)]
struct HDANodeCommand(u32);

#[repr(transparent)]
struct HDANodeResponse(u64);

/// The Command Outbound Ring buffer as specified in
/// section 4.4.1 of the HDA spec, revision 1.0a
///
/// The CORB is a circular buffer (circular means when the buffer is read through,
/// reading starts from the beginning again), in memory used to pass commands to the
/// codecs connected to the HDA link
///
/// According to the spec, the entry number is programmable to 2, 16 or 256,
/// but this representation is hardcoded to 256, because it's good enough for its
/// purposes in this project
#[repr(C, align(128))]
struct CORB {
    /// The commands to be fetched by the HDA controller
    commands: [HDANodeCommand; 256],
    /// Indicates to the hardware the last valid command in the `commands` array
    write_pointer: usize
}

impl CORB {
  /*  fn new() -> Self {
        Self {
            commands: [?; 256],
            write_pointer: 0
        }
    }
*/
    fn add_command(&mut self, command: HDANodeCommand, sound_device: &PCIDevice) {
        self.write_pointer += 1;
        //sound_device.set_corbwp(self.write_pointer);
        self.commands[self.write_pointer] = command;
    }

    fn init(&self) {
        // Assert that the CORBRUN bit in the CORBCTL register is 0
        // Set the CORBSIZE register
        // CORBBASE should be set to the base of the CORB memory
        // CORBRPRST bit is used to reset the read pointer to 0
        // 0 must be written to the write pointer to clear the write pointer
        // CORBRUN bit should be set to 1 to enable operation
    }
}

/// The Response Inbound Ring Buffer as specified by in
/// section 4.4.2 of the HDA spec, revision 1.0a
///
/// This is a circular buffer used to store responses from the
/// codecs connected to the link
///
/// According to the spec, the entry number is programmable to 2, 16 or 256,
/// but this representation is hardcoded to 256, because it's good enough for its
/// purposes in this project
#[repr(C, align(128))]
struct RIRB {
    /// The responses from the codecs
    responses: [HDANodeResponse; 256],
    /// The index of the last response read
    read_pointer: usize
}
/*
impl RIRB {
    fn new() -> {
        Self {
            responses: [?; 256],
            read_pointer: 0
        }
    }

    fn init(&self) {
        // A bunch of register stuff
    }
}
*/
/// A 16-bit sample container as specified in the HDA spec
#[repr(transparent)]
struct Sample(u16);
/*
/// A buffer of samples
///
/// A set of instances of this structure is what makes up
/// the virtual cyclic buffer. The buffer descriptor list contains
/// the descriptions of these buffers
#[repr(C, align(128))]
struct SampleBuffer {
    samples: [Sample; ]
}
*/