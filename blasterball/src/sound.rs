use artist::{println, WriteTarget};
use machine::port::{Port, PortReadWrite};

pub unsafe fn figure_out_how_to_make_sounds() {
    let hda_bus_and_device_number_opt = find_hda_bus_and_device_number();
    if hda_bus_and_device_number_opt.is_none() {
        panic!("Didn't find the HDA");
    }
    let (bus, device) = hda_bus_and_device_number_opt.unwrap();
    println!("The HDA bus: {:x} and device: {:x}", bus, device);
    loop {}
}

/// Searches all buses on the PCI until it finds the bus and device number
/// of the HDA
///
/// # References
///
/// * https://wiki.osdev.org/PCI
/// * https://wiki.osdev.org/Intel_High_Definition_Audio#Identifying_HDA_on_a_machine
unsafe fn find_hda_bus_and_device_number() -> Option<(u8, u8)> {
    for bus in 0..=255 {
        for device in 0..32 {
            if let PCIInfo { vendor_id, classcode, subclass } = pci_config_info(bus, device) {
                // No vendor id is ever equal to 0xffff.
                // According to the OSDev wiki, the best way to identify HDA is to look for
                // the class code (0x4) and subclass (0x3)
                if vendor_id != 0xffff && classcode == 0x4 && subclass == 0x3 {
                    return Some((classcode, subclass));
                }
            }
        }
    }
    None
}

/// Gets info about the Vendor ID, class code and subclass from the PCI
/// configuration space
///
/// # References
///
/// * https://wiki.osdev.org/PCI
unsafe fn pci_config_info(bus: u8, slot: u8) -> PCIInfo {
    let bus: u32 = bus as u32;
    let slot: u32 = slot as u32;
    let func: u32 = 0;
    let device_vendor_id_offset = 0;
    let device_vendor_id_addr: u32 = bus << 16 | slot << 11 | func << 8 | (device_vendor_id_offset as u32 & 0xfc) | 0x80000000 as u32;
    let mut address_port: Port<u32> = Port::new(0xcf8);
    address_port.write(device_vendor_id_addr);
    let data_port: Port<u32> = Port::new(0xcfc);
    let val = data_port.read();
    let vendor_id = val as u16;
    let device_id = (val >> 16) as u16;
    let classcode_subclass_offset = 0x8;
    let address: u32 = bus << 16 | slot << 11 | func << 8 | (classcode_subclass_offset as u32 & 0xfc) | 0x80000000 as u32;
    address_port.write(address);
    let val = data_port.read();
    let classcode = (val >> 24) as u8;
    let subclass = ((val >> 16) & 0xff) as u8;
    PCIInfo {
        vendor_id,
        classcode,
        subclass
    }
}

struct PCIInfo {
    vendor_id: u16,
    classcode: u8,
    subclass: u8
}