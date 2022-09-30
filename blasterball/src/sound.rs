use artist::{println, WriteTarget};
use machine::port::{Port, PortReadWrite};

pub unsafe fn figure_out_how_to_make_sounds() {
    let hda_bus_and_device_number_opt = find_hda_bus_and_device_number();
    if hda_bus_and_device_number_opt.is_none() {
        panic!("Didn't find the HDA");
    }
    let sound_device = hda_bus_and_device_number_opt.unwrap();

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
fn find_hda_bus_and_device_number() -> Option<PCIDevice> {
    for bus in 0..=255 {
        for device in 0..32 {
            for func in 0..8 {
                let pci_device = PCIDevice { bus, device, func };
                if pci_device.is_valid() {
                    // No vendor id is ever equal to 0xffff.
                    // According to the OSDev wiki, the best way to identify HDA is to look for
                    // the class code (0x4) and subclass (0x3)
                    if pci_device.classcode() == 0x4 && pci_device.subclass() == 0x3 {
                        return Some(pci_device);
                    }
                }
            }
        }
    }
    None
}

/// A device on the PCI bus
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
    const BAR0_OFFSET: u32 = 0x10;

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
}
