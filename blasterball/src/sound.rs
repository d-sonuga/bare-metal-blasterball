use core::ops::Index;
use artist::{println, WriteTarget};
use machine::port::{Port, PortReadWrite};
use machine::interrupts::IRQ;
use machine::memory::Addr;
use num::{Integer, BitState};
use collections::vec;
use collections::vec::Vec;
use crate::wav::WavFile;

#[link_section = ".sound"]
pub static MUSIC: [u8; 7287938] = *include_bytes!("./assets/canon-in-d-major.wav");
#[link_section = ".sound"]
static BOUNCE: [u8; 16140] = *include_bytes!("./assets/bounce.wav");
#[link_section = ".sound"]
static CLINK: [u8; 217536] = *include_bytes!("./assets/clink.wav");
#[link_section = ".sound"]
static DRUM: [u8; 734028] = *include_bytes!("./assets/drum.wav");

static mut corb_: Option<CORB> = None;
static mut rirb_: Option<RIRB> = None;

pub unsafe fn figure_out_how_to_make_sounds() {
    println!("So the rust compiler won't remove it, here's a byte {:x}", MUSIC[0]);
    let hda_bus_and_device_number_opt = find_hda_bus_and_device_number();
    if hda_bus_and_device_number_opt.is_none() {
        panic!("Didn't find the HDA");
    }
    let mut sound_device = hda_bus_and_device_number_opt.unwrap();
    sound_device.start();
    //sound_device.pci_config.set_interrupt_line(IRQ::Sound);
/*
    // Initialization from redox
    sound_device.set_state_change_status(HDAStateChangeStatusReg(0xffff));
    let mut ctrl = sound_device.global_control();
    ctrl.set_controller_reset(false);
    sound_device.set_global_control(ctrl);
    while sound_device.global_control().controller_reset() {}
    let mut ctrl = sound_device.global_control();
    ctrl.set_controller_reset(true);
    sound_device.set_global_control(ctrl);
    while !sound_device.global_control().controller_reset() {}
    let mut timeout = 0;
    while timeout < 1_000_000 { timeout += 1; }

    let (corb_size, rirb_size) = get_corb_rirb_sizes(&sound_device);
    let mut corb = CORB::new(corb_size);
    let mut rirb = RIRB::new(rirb_size);

    while sound_device.corb_control().corb_dma_engine_enabled() {
        sound_device.set_corb_control(HDACORBControlReg(0));
    }
    sound_device.set_corb_addr(&corb as *const _ as u64);
    sound_device.set_corbwp(HDACORBWritePointerReg(0));
    let mut rp = sound_device.corbrp();
    rp.set_read_pointer_reset(true);
    sound_device.set_corbrp(rp);

    let mut ctrl = sound_device.rirb_control();
    ctrl.enable_rirb_dma_engine(false);
    sound_device.set_rirb_control(ctrl);
    sound_device.set_rirb_addr(&rirb as *const _ as u64);
    let mut wp = sound_device.rirbwp();
    wp.reset_write_pointer();
    sound_device.set_rirbwp(wp);
    sound_device.set_response_interrupt_count(HDAResponseInterruptCountReg(1));

    let mut ctrl = sound_device.corb_control();
    ctrl.enable_corb_dma_engine(true);
    sound_device.set_corb_control(ctrl);
    let mut ctrl = sound_device.rirb_control();
    ctrl.enable_rirb_dma_engine(true);
    ctrl.enable_interrupt(true);
    sound_device.set_rirb_control(ctrl);

    println!("Wait 1; wp: {:?}, rp: {:?}", sound_device.corbwp().write_pointer(), sound_device.corbrp().read_pointer());
    while sound_device.corbwp().write_pointer() != sound_device.corbrp().read_pointer() {}
    let get_node_count_command = HDANodeCommand::get_node_count(0, 0);
    println!("About to add command");
        corb.add_command(get_node_count_command, &mut sound_device);
        println!("Added get node count command");
    println!("Current PCI status: {:b}", sound_device.pci_config.status());
    println!("Wait 2; rp: {:?}, wp: {:?}", rirb.read_pointer, sound_device.rirbwp().write_pointer());
    while rirb.read_pointer == sound_device.rirbwp().write_pointer().as_usize() {}
    let node_count_resp = rirb
        .read_next_response(&mut sound_device);
    println!("The response: {:?}", node_count_resp);
    // End Redox initialization
*/
    
    // After starting the device the addresses of the codecs
    // are the set bit positions in the state change status register
    let sdin_state_change_stat = sound_device.state_change_status().sdin_state_change_status();
    let mut codec_addrs = vec!(item_type => u8, capacity => 15);
    (0..16u8)
        .for_each(|i| if sdin_state_change_stat.get_bit(i.into()) == BitState::Set {
            codec_addrs.push(i);
        });
    println!("codec_addrs: {:?}", codec_addrs);
    
    /*
        let get_node_count_command = HDANodeCommand::get_node_count(codec_addrs[0], 0);
        assert!(!sound_device.immediate_response_received());
        sound_device.immediate_command_output(get_node_count_command);
        println!("Waiting for the response to indicate received");
        loop {
            if sound_device.immediate_response_received() {
                break;
            }
        }
        let resp = sound_device.immediate_response_input();
        println!("Response received: {:?}", resp);
    */
    
    

    
    let (corb_size, rirb_size) = get_corb_rirb_sizes(&sound_device);
    //let mut corb = CORB::new(corb_size);
    //let mut rirb = RIRB::new(rirb_size);
    corb_ = Some(CORB::new(corb_size));
    rirb_ = Some(RIRB::new(rirb_size));
    println!("here1");
    sound_device.init_corb(corb_.as_ref().unwrap());
    sound_device.init_rirb(rirb_.as_ref().unwrap());
    println!("About to enable CORB DMA engine");
    sound_device.enable_corb_dma_engine(true);
    println!("Enabled CORB DMA Engine");
    sound_device.enable_rirb_dma_engine(true);
    

    let get_node_count_command = HDANodeCommand::get_node_count(codec_addrs[0], 0);
    println!("About to add command");
        corb_.as_mut().unwrap().add_command(get_node_count_command, &mut sound_device);
        println!("Added get node count command");
    let node_count_resp = rirb_
        .as_mut().unwrap()
        .read_next_response(&mut sound_device);
    println!("Gotten response to add node count command: {:?}", node_count_resp);
    
    //println!("Initialized the CORB and RIRB");
    /*for &codec_addr in codec_addrs.iter() {
        // Gettings function group info from the root node
        let get_node_count_command = HDANodeCommand::get_node_count(codec_addr, 0);
        corb.add_command(get_node_count_command, &mut sound_device);
        println!("Added get node count command");
        let node_count_resp = rirb
            .read_next_response(&mut sound_device)
            .node_count_resp()
            .unwrap();
        println!("Gotten response to add node count command");
        let first_func_group_id = node_count_resp.start_node_number();
        for func_group_id in first_func_group_id..first_func_group_id + node_count_resp.number_of_nodes() {
            // Checking if the function group is an AFG
            let func_group_type_command = HDANodeCommand::function_group_type(codec_addr, func_group_id);
            corb.add_command(func_group_type_command, &mut sound_device);
            let func_group_type_resp = rirb
                .read_next_response(&mut sound_device)
                .func_group_type_resp()
                .unwrap();
            println!("Function group type: {:?}", func_group_type_resp.node_type());
            if func_group_type_resp.node_type() == HDANodeFunctionGroupType::AFG {
                // Getting the number of widgets in the AFG
                let get_node_count_command = HDANodeCommand::get_node_count(codec_addr, func_group_id);
                corb.add_command(get_node_count_command, &mut sound_device);
                let node_count_resp = rirb
                    .read_next_response(&mut sound_device)
                    .node_count_resp()
                    .unwrap();
                let start_widget_id = node_count_resp.start_node_number();
                for widget_id in start_widget_id..start_widget_id + node_count_resp.number_of_nodes() {
                    let afg_widget_cap_command = HDANodeCommand::afg_widget_capabilities(codec_addr, widget_id);
                    corb.add_command(afg_widget_cap_command, &mut sound_device);
                    let widget_cap_resp = rirb
                        .read_next_response(&mut sound_device)
                        .afg_widget_capabilities_resp()
                        .unwrap();
                    println!("Widget Type: {:?}", widget_cap_resp.widget_type());
                    // Looking for the pin widget connected to a DAC
                    if widget_cap_resp.widget_type() == HDAAFGWidgetType::PinComplex {
                        // Find out if the pin is connected to a speaker
                        let get_pin_config_defaults_command = HDANodeCommand::get_pin_config_defaults(codec_addr, widget_id);
                        corb.add_command(get_pin_config_defaults_command, &mut sound_device);
                        let pin_config_defaults_resp = rirb
                            .read_next_response(&mut sound_device)
                            .get_pin_config_defaults_resp()
                            .unwrap();
                        println!("Port connectivity: {:?}, default device: {:?}",
                            pin_config_defaults_resp.port_connectivity(),
                            pin_config_defaults_resp.default_device());
                        if pin_config_defaults_resp.port_connectivity() == PortConnectivity::FixedFuncDevice
                            && pin_config_defaults_resp.default_device() == DefaultDevice::Speaker {
                                println!("Found pin connected to speaker");
                            }
                        let get_conn_list_command = HDANodeCommand::get_conn_list_len(codec_addr, widget_id);
                        corb.add_command(get_conn_list_command, &mut sound_device);
                        let conn_list_len_resp = rirb
                            .read_next_response(&mut sound_device)
                            .get_conn_list_len_resp();
                        if conn_list_len_resp.is_ok() {
                            let conn_list_len_resp = conn_list_len_resp.unwrap();
                            
                            // To find the entries in the pin's connection list
                            let mut conn_list_index_iter = (0..conn_list_len_resp.conn_list_len()).step_by(4);
                            if conn_list_len_resp.long_form() {
                                conn_list_index_iter = (0..conn_list_len_resp.conn_list_len()).step_by(2);
                            }
                            for conn_idx in conn_list_index_iter {
                                let get_conn_list_entry_command = HDANodeCommand::get_conn_list_entry(codec_addr, widget_id, conn_idx);
                                corb.add_command(get_conn_list_entry_command, &mut sound_device);
                                let get_conn_list_entry_resp = rirb
                                    .read_next_response(&mut sound_device)
                                    .get_conn_list_entry_resp(conn_list_len_resp.long_form())
                                    .unwrap();
                                // Looking for a DAC connected to the pin
                                for connected_node_id in get_conn_list_entry_resp.entries() {
                                    let afg_widget_cap_command = HDANodeCommand::afg_widget_capabilities(codec_addr, connected_node_id.as_u8());
                                    corb.add_command(afg_widget_cap_command, &mut sound_device);
                                    let widget_cap_resp = rirb
                                        .read_next_response(&mut sound_device)
                                        .afg_widget_capabilities_resp()
                                        .unwrap();
                                    // The DAC connected to the pin
                                    if widget_cap_resp.widget_type() == HDAAFGWidgetType::AudioOutput {
                                        // ...
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }*/
    loop {}
}

/// Gets the max number of entries supported for the
/// CORB and RIRB
fn get_corb_rirb_sizes(sound_device: &SoundDevice) -> (HDARingBufferSize, HDARingBufferSize) {
    let mut corb_size = HDARingBufferSize::TwoFiftySix;
    let mut rirb_size = HDARingBufferSize::TwoFiftySix;
    let corb_size_capability = sound_device.corb_size().size_capability();
    let rirb_size_capability = sound_device.rirb_size().size_capability();
    if !corb_size_capability.size256_supported() {
        if corb_size_capability.size16_supported() {
            corb_size = HDARingBufferSize::Sixteen;
        } else {
            corb_size = HDARingBufferSize::Two;
        }
    }
    if !rirb_size_capability.size256_supported() {
        if rirb_size_capability.size16_supported() {
            rirb_size = HDARingBufferSize::Sixteen;
        } else {
            rirb_size = HDARingBufferSize::Two;
        }
    }
    (corb_size, rirb_size)
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
    const STATUS_AND_COMMAND_OFFSET: u32 = 0x04;
    const CLASSCODE_AND_SUBCLASS_OFFSET: u32 = 0x8;
    const HEADER_TYPE_OFFSET: u32 = 0xc;
    const BAR0_OFFSET: u32 = 0x10;
    const BAR1_OFFSET: u32 = 0x14;
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

    fn bar0(&self) -> PCIBaseAddrReg {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let (mut addr_port, data_port) = self.ports();
        let addr: u32 = self.reg_addr(Self::BAR0_OFFSET);
        addr_port.write(addr);
        let val1 = data_port.read();
        let addr = self.reg_addr(Self::BAR1_OFFSET);
        addr_port.write(addr);
        let val2 = data_port.read();
        let bar = PCIBaseAddrReg::try_from((val1, val2)).unwrap();
        assert_eq!(bar.kind(), PCIBaseAddrKind::Memory);
        bar
    }

    /*fn size_of_addr_space_needed(&self) -> u32 {
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
    }*/

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

    fn status(&self) -> u16 {
        let (mut addr_port, data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::STATUS_AND_COMMAND_OFFSET);
        addr_port.write(reg_addr);
        (data_port.read() >> 16) as u16
    }

    fn command(&self) -> u16 {
        let (mut addr_port, data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::STATUS_AND_COMMAND_OFFSET);
        addr_port.write(reg_addr);
        data_port.read() as u16
    }

    fn set_command(&mut self, val: u16) {
        let (mut addr_port, mut data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::STATUS_AND_COMMAND_OFFSET);
        addr_port.write(reg_addr);
        data_port.write(val.into());
    }

    fn enable_memory_space_accesses(&mut self) {
        let mut val = self.command();

        // Added for experimenting
        val.set_bit(0);
        val.set_bit(2);
        val.set_bit(3);
        val.set_bit(4);
        val.set_bit(8);
        //

        val.set_bit(1);
        self.set_command(val);
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
    /// Sets up the CORB to a ready state for communication
    /// with the HDA controller
    fn init_corb(&mut self, corb: &CORB) {
        if self.corb_control().corb_dma_engine_enabled() {
            self.enable_corb_dma_engine(false);
        }
        /*let mut corb_ctrl_reg = self.corb_control();
        // The engine must not be running yet
        //if corb_ctrl_reg.corb_dma_engine_enabled() {
            // The DMA engine must first be stopped
            corb_ctrl_reg.enable_corb_dma_engine(false);
            self.set_corb_control(corb_ctrl_reg);
            // The value of false must be read back to verify
            // that the engine stopped
            loop {
                if !self.corb_control().corb_dma_engine_enabled() {
                    break;
                }
            }*/
        //}
        let mut corb_size_reg = self.corb_size();
        let corb_size_capability = corb_size_reg.size_capability();
        corb_size_reg.set_corb_size(corb.size());
        self.set_corb_size(corb_size_reg);
        self.set_corb_addr(&corb.commands as *const _ as u64);

        let mut corbwp_reg = self.corbwp();
        corbwp_reg.set_write_pointer(0);
        self.set_corbwp(corbwp_reg);

        let mut corbrp_reg = self.corbrp();
        corbrp_reg.set_read_pointer_reset(true);
        self.set_corbrp(corbrp_reg);
        loop {
            // The value must be read back to verify that
            // it was reset
            let corbrp_reg = self.corbrp();
            if corbrp_reg.read_pointer_reset() {
                break;
            }
        }
        
        // The read pointer reset must then be cleared again
        let mut corbrp_reg = self.corbrp();
        corbrp_reg.set_read_pointer_reset(false);
        self.set_corbrp(corbrp_reg);
        loop {
            // The value must be read back to verify that it was reset
            let corbrp_reg = self.corbrp();
            if !corbrp_reg.read_pointer_reset() {
                break;
            }
        }
        
        //self.enable_corb_dma_engine(true);
    }

    fn init_rirb(&mut self, rirb: &RIRB) {
        self.enable_rirb_dma_engine(false);
        /*let mut rirb_ctrl_reg = self.rirb_control();
        //if rirb_ctrl_reg.rirb_dma_engine_enabled() {
            // The DMA engine must first be stopped
            rirb_ctrl_reg.enable_rirb_dma_engine(false);
            self.set_rirb_control(rirb_ctrl_reg);
            // The value of false must be read back to verify
            // that the engine stopped
            loop {
                if !self.rirb_control().rirb_dma_engine_enabled() {
                    break;
                }
            }*/
        //}
        let mut rirb_size_reg = self.rirb_size();
        let rirb_size_capability = rirb_size_reg.size_capability();
        rirb_size_reg.set_rirb_size(rirb.size());
        self.set_rirb_size(rirb_size_reg);
        self.set_rirb_addr(&rirb.responses as *const _ as u64);

        let mut rirbwp_reg = self.rirbwp();
        rirbwp_reg.reset_write_pointer();
        self.set_rirbwp(rirbwp_reg);

        //let mut rirb_ctrl_reg = self.rirb_control();
        //rirb_ctrl_reg.enable_rirb_dma_engine(true);
        //rirb_ctrl_reg.set_response_overrun_interrupt_control(true);
        //self.set_rirb_control(rirb_ctrl_reg);

        let mut rintcnt_reg = self.response_interrupt_count();
        rintcnt_reg.set_response_interrupt_count(255);
        self.set_response_interrupt_count(rintcnt_reg);

        // The value of true must be read back to verify that
        // the DMA engine is in a running state
        //self.enable_rirb_dma_engine(true);
        /*let mut timeout = 0;
        loop {
            let mut rirb_ctrl_reg = self.rirb_control();
            if rirb_ctrl_reg.rirb_dma_engine_enabled() || timeout == 100_000 {
                break;
            }
            timeout += 1;
        }*/
    }
}

impl SoundDevice {
    // Register offsets
    const GLOBAL_CAPABILTIES_OFFSET: isize = 0x00;
    const GLOBAL_CONTROL_OFFSET: isize = 0x08;
    const WAKE_ENABLE_OFFSET: isize = 0x0c;
    const STATE_CHANGE_STATUS_OFFSET: isize = 0x0e;
    const INTERRUPT_CONTROL_OFFSET: isize = 0x20;
    const INTERRUPT_STATUS_OFFSET: isize = 0x24;
    const CORB_LOWER_BASE_ADDR_OFFSET: isize = 0x40;
    const CORB_UPPER_BASE_ADDR_OFFSET: isize = 0x44;
    const CORB_WRITE_POINTER_OFFSET: isize = 0x48;
    const CORB_READ_POINTER_OFFSET: isize = 0x4a;
    const CORB_CONTROL_OFFSET: isize = 0x4c;
    const CORB_STATUS_OFFSET: isize = 0x4d;
    const CORB_SIZE_OFFSET: isize = 0x4e;
    const RIRB_LOWER_BASE_ADDR_OFFSET: isize = 0x50;
    const RIRB_UPPER_BASE_ADDR_OFFSET: isize = 0x54;
    const RIRB_WRITE_POINTER_OFFSET: isize = 0x58;
    const RESPONSE_INTERRUPT_COUNT_OFFSET: isize = 0x5a;
    const RIRB_CONTROL_OFFSET: isize = 0x5c;
    const RIRB_STATUS_OFFSET: isize = 0x5d;
    const RIRB_SIZE_OFFSET: isize = 0x5e;
    const IMMEDIATE_COMMAND_OUTPUT_OFFSET: isize = 0x60;
    const IMMEDIATE_RESPONSE_INPUT_OFFSET: isize = 0x64;
    const DMA_POS_LOWER_BASE_ADDR_OFFSET: isize = 0x70;
    const DMA_POS_UPPER_BASE_ADDR_OFFSET: isize = 0x74;
    const STREAM_DESCRIPTOR_CONTROL_OFFSET: isize = 0x80;
    const STREAM_DESCRIPTOR_STATUS_OFFSET: isize = 0x83;
    const STREAM_DESCRIPTOR_LINK_POSITION_OFFSET: isize = 0x84;
    const STREAM_DESCRIPTOR_CYCLIC_BUFFER_LENGTH_OFFSET: isize = 0x88;
    const STREAM_DESCRIPTOR_LAST_VALID_INDEX_OFFSET: isize = 0x8c;
    const STREAM_DESCRIPTOR_FIFO_SIZE_OFFSET: isize = 0x90;
    const STREAM_DESCRIPTOR_FORMAT_OFFSET: isize = 0x92;
    const STREAM_DESCRIPTOR_BDL_POINTER_LOWER_BASE_ADDR_OFFSET: isize = 0x98;
    const STREAM_DESCRIPTOR_BDL_POINTER_UPPER_BASE_ADDR_OFFSET: isize = 0x9c;

    fn start(&mut self) {
        /*println!("CORB DMA: {}, RIRB DMA: {}", self.corb_control().corb_dma_engine_enabled(),
            self.rirb_control().rirb_dma_engine_enabled());
            loop {}
        // When the controller is first brought up, it will be in reset
        // and to start operation, it has to be taken out of reset
        let mut corb_ctrl_reg = self.corb_control();
        // The engine must not be running yet
        if corb_ctrl_reg.corb_dma_engine_enabled() {
            // The DMA engine must first be stopped
            corb_ctrl_reg.enable_corb_dma_engine(false);
            self.set_corb_control(corb_ctrl_reg);
            // The value of false must be read back to verify
            // that the engine stopped
            loop {
                if !self.corb_control().corb_dma_engine_enabled() {
                    break;
                }
            }
        }
        let mut rirb_ctrl_reg = self.rirb_control();
        if rirb_ctrl_reg.rirb_dma_engine_enabled() {
            // The DMA engine must first be stopped
            rirb_ctrl_reg.enable_rirb_dma_engine(false);
            self.set_rirb_control(rirb_ctrl_reg);
            // The value of false must be read back to verify
            // that the engine stopped
            loop {
                if !self.rirb_control().rirb_dma_engine_enabled() {
                    break;
                }
            }
        }*/
        /*let mut scs_reg = self.state_change_status();
        scs_reg.clear_sdin_state_change_status();
        self.set_state_change_status(scs_reg);*/

        /*let mut global_ctrl_reg = self.global_control();
        global_ctrl_reg.set_controller_reset(false);
        self.set_global_control(global_ctrl_reg);
        loop {
            // The HDA spec dictates that software should wait
            // after changing the reset value to verify that the value
            // changed
            // This keeps loading the controller reset value until
            // it shows that it is no longer in reset state
            if !self.global_control().controller_reset() {
                break;
            }
        }*/
        
        let mut global_ctrl_reg = self.global_control();
        // Asserting the bit removes the controller from the reset state
        global_ctrl_reg.set_controller_reset(true);
        self.set_global_control(global_ctrl_reg);
        loop {
            if self.global_control().controller_reset() {
                break;
            }
        }

        // After reset de-assertion, 521 us should be waited
        let mut timeout = 0;
        while timeout < 1_000_000 {
            timeout += 1 ;
        }

        while self.state_change_status().sdin_state_change_status() == 0 {}
        
        //let mut rintcnt_reg = self.response_interrupt_count();
        //rintcnt_reg.set_response_interrupt_count(255);
        //self.set_response_interrupt_count(rintcnt_reg);
    }

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

    fn set_corb_addr(&mut self, addr: u64) {
        let lower = (addr & 0xffffffff) as u32;
        let upper = (addr >> 32) as u32;
        let ptr = self.reg_ptr(Self::CORB_LOWER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(lower) };
        let ptr = self.reg_ptr(Self::CORB_UPPER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(upper) };
    }

    fn corbwp(&self) -> HDACORBWritePointerReg {
        let ptr = self.reg_ptr(Self::CORB_WRITE_POINTER_OFFSET).cast::<u16>();
        unsafe { HDACORBWritePointerReg::from(ptr.read()) }
    }

    /// Sets the device's CORBWP pointer to val, which points to
    /// the index of the last valid command in the CORB
    fn set_corbwp(&mut self, val: HDACORBWritePointerReg) {
        let ptr = self.reg_ptr(Self::CORB_WRITE_POINTER_OFFSET).cast::<u16>();
        unsafe { ptr.write(val.into()) }
    }

    fn corbrp(&self) -> HDACORBReadPointerReg {
        let ptr = self.reg_ptr(Self::CORB_READ_POINTER_OFFSET).cast::<u16>();
        unsafe { HDACORBReadPointerReg::from(ptr.read()) }
    }

    fn set_corbrp(&mut self, val: HDACORBReadPointerReg) {
        let ptr = self.reg_ptr(Self::CORB_READ_POINTER_OFFSET).cast::<u16>();
        unsafe { ptr.write(val.into()) }
    }

    fn corb_control(&self) -> HDACORBControlReg {
        let ptr = self.reg_ptr(Self::CORB_CONTROL_OFFSET).cast::<u8>();
        unsafe { HDACORBControlReg::from(ptr.read()) }
    }

    fn set_corb_control(&mut self, val: HDACORBControlReg) {
        let ptr = self.reg_ptr(Self::CORB_CONTROL_OFFSET).cast::<u8>();
        unsafe { ptr.write(val.into()) }
    }

    fn enable_corb_dma_engine(&mut self, enable: bool) {
        let mut ctrl_reg = self.corb_control();
        ctrl_reg.enable_corb_dma_engine(enable);
        self.set_corb_control(ctrl_reg);
        println!("About to verify CORB enablement");
        // Must read back
        loop {
            if enable == self.corb_control().corb_dma_engine_enabled() {
                break;
            }
            let mut ctrl_reg = self.corb_control();
            ctrl_reg.enable_corb_dma_engine(enable);
            self.set_corb_control(ctrl_reg);   
        }
        println!("Verified CORB enablement");
    }

    fn corb_status(&self) -> HDACORBStatusReg {
        let ptr = self.reg_ptr(Self::CORB_STATUS_OFFSET).cast::<u8>();
        unsafe { HDACORBStatusReg::from(ptr.read()) }
    }

    fn corb_size(&self) -> HDACORBSizeReg {
        let ptr = self.reg_ptr(Self::CORB_SIZE_OFFSET).cast::<u8>();
        unsafe { HDACORBSizeReg::from(ptr.read()) }
    }

    fn set_corb_size(&mut self, val: HDACORBSizeReg) {
        let ptr = self.reg_ptr(Self::CORB_SIZE_OFFSET).cast::<u8>();
        unsafe { ptr.write(val.into()) }
    }

    fn set_rirb_addr(&mut self, addr: u64) {
        let lower = addr as u32;
        let upper = (addr >> 32) as u32;
        let ptr = self.reg_ptr(Self::RIRB_LOWER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(lower) };
        let ptr = self.reg_ptr(Self::RIRB_UPPER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(upper) };
    }

    fn rirbwp(&self) -> HDARIRBWritePointerReg {
        let ptr = self.reg_ptr(Self::RIRB_WRITE_POINTER_OFFSET).cast::<u16>();
        unsafe { HDARIRBWritePointerReg::from(ptr.read()) }
    }

    fn set_rirbwp(&mut self, val: HDARIRBWritePointerReg) {
        let ptr = self.reg_ptr(Self::RIRB_WRITE_POINTER_OFFSET).cast::<u16>();
        unsafe { ptr.write(val.into()) }
    }

    fn response_interrupt_count(&self) -> HDAResponseInterruptCountReg {
        let ptr = self.reg_ptr(Self::RESPONSE_INTERRUPT_COUNT_OFFSET).cast::<u16>();
        unsafe { HDAResponseInterruptCountReg::from(ptr.read()) }
    }
    
    fn set_response_interrupt_count(&mut self, val: HDAResponseInterruptCountReg) {
        let ptr = self.reg_ptr(Self::RESPONSE_INTERRUPT_COUNT_OFFSET).cast::<u16>();
        unsafe { ptr.write(val.into()) }
    }

    fn rirb_control(&self) -> HDARIRBControlReg {
        let ptr = self.reg_ptr(Self::RIRB_CONTROL_OFFSET).cast::<u8>();
        unsafe { HDARIRBControlReg::from(ptr.read()) }
    }

    fn set_rirb_control(&mut self, val: HDARIRBControlReg) {
        let ptr = self.reg_ptr(Self::RIRB_CONTROL_OFFSET).cast::<u8>();
        unsafe { ptr.write(val.into()) }
    }

    fn rirb_status(&self) -> HDARIRBStatusReg {
        let ptr = self.reg_ptr(Self::RIRB_STATUS_OFFSET).cast::<u8>();
        unsafe { HDARIRBStatusReg::from(ptr.read()) }
    }

    fn rirb_size(&self) -> HDARIRBSizeReg {
        let ptr = self.reg_ptr(Self::RIRB_SIZE_OFFSET).cast::<u8>();
        unsafe { HDARIRBSizeReg::from(ptr.read()) }
    }

    fn set_rirb_size(&mut self, val: HDARIRBSizeReg) {
        let ptr = self.reg_ptr(Self::RIRB_SIZE_OFFSET).cast::<u8>();
        unsafe { ptr.write(val.into()) };
    }

    fn enable_rirb_dma_engine(&mut self, enable: bool) {
        let mut rirb_ctrl_reg = self.rirb_control();
        rirb_ctrl_reg.enable_rirb_dma_engine(enable);
        self.set_rirb_control(rirb_ctrl_reg);
        // The value of true must be read back to verify that
        // the DMA engine is in a running state
        loop {
            let rirb_ctrl_reg = self.rirb_control();
            if enable == rirb_ctrl_reg.rirb_dma_engine_enabled() {
                break;
            }
            let mut rirb_ctrl_reg = self.rirb_control();
            rirb_ctrl_reg.enable_rirb_dma_engine(enable);
            self.set_rirb_control(rirb_ctrl_reg);
        }
    }

    fn set_dma_pos_buffer_addr(&mut self, addr: u64) {
        let lower = addr as u32;
        let upper = (addr >> 32) as u32;
        let ptr = self.reg_ptr(Self::DMA_POS_LOWER_BASE_ADDR_OFFSET).cast::<u32>();
        let mut lower_addr_reg = unsafe { HDADMAPosLowerBaseAddrReg::from(ptr.read()) };
        lower_addr_reg.set_lower_base_addr(lower);
        unsafe { ptr.write(lower_addr_reg.into()) };
        let ptr = self.reg_ptr(Self::DMA_POS_UPPER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(upper) };
    }

    fn stream_descriptor_control(&self) -> HDAStreamDescriptorControlReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_CONTROL_OFFSET).cast::<u8>();
        let first_byte = unsafe { ptr.read() };
        let second_byte = unsafe { ptr.offset(1).read() };
        let third_byte = unsafe { ptr.offset(2).read() };
        let val = u32::from_be_bytes([0, first_byte, second_byte, third_byte]);
        HDAStreamDescriptorControlReg::from(val)
    }

    fn steam_descriptor_status(&self) -> HDAStreamDescriptorStatusReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_STATUS_OFFSET).cast::<u8>();
        unsafe { HDAStreamDescriptorStatusReg::from(ptr.read()) }
    }

    fn stream_descriptor_link_position(&self) -> HDAStreamDescriptorLinkPosReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_LINK_POSITION_OFFSET).cast::<u32>();
        unsafe { HDAStreamDescriptorLinkPosReg::from(ptr.read()) }
    }

    fn stream_descriptor_cyclic_buffer_length(&self) -> HDAStreamDescriptorCyclicBufferLenReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_CYCLIC_BUFFER_LENGTH_OFFSET).cast::<u32>();
        unsafe { HDAStreamDescriptorCyclicBufferLenReg::from(ptr.read()) }
    }

    fn stream_descriptor_last_valid_index(&self) -> HDAStreamDescriptorLastValidIndexReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_LAST_VALID_INDEX_OFFSET).cast::<u16>();
        unsafe { HDAStreamDescriptorLastValidIndexReg::from(ptr.read()) }
    }

    fn stream_descriptor_fifo_size(&self) -> HDAStreamDescriptorFIFOSizeReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_FIFO_SIZE_OFFSET).cast::<u16>();
        unsafe { HDAStreamDescriptorFIFOSizeReg::from(ptr.read()) }
    }

    fn stream_descriptor_format(&self) -> HDAStreamDescriptorFormatReg {
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_FORMAT_OFFSET).cast::<u16>();
        unsafe { HDAStreamDescriptorFormatReg::from(ptr.read()) }
    }

    fn set_stream_descriptor_bdl_ptr_addr(&mut self, addr: u64) {
        let lower = addr as u32;
        let upper = (addr >> 32) as u32;
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_BDL_POINTER_LOWER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(lower) };
        let ptr = self.reg_ptr(Self::STREAM_DESCRIPTOR_BDL_POINTER_UPPER_BASE_ADDR_OFFSET).cast::<u32>();
        unsafe { ptr.write(upper) };
    }

    fn global_capabilities(&self) -> HDAGlobalCapabilitiesReg {
        let ptr = self.reg_ptr(Self::GLOBAL_CAPABILTIES_OFFSET).cast::<u16>();
        unsafe { HDAGlobalCapabilitiesReg::from(ptr.read()) }
    }

    fn global_control(&self) -> HDAGlobalControlReg {
        let ptr = self.reg_ptr(Self::GLOBAL_CONTROL_OFFSET).cast::<u32>();
        unsafe { HDAGlobalControlReg::from(ptr.read()) }
    }

    fn set_global_control(&mut self, val: HDAGlobalControlReg) {
        let ptr = self.reg_ptr(Self::GLOBAL_CONTROL_OFFSET).cast::<u32>();
        unsafe { ptr.write(val.into()) }
    }

    fn wake_enable(&self) -> HDAWakeEnableReg {
        let ptr = self.reg_ptr(Self::WAKE_ENABLE_OFFSET).cast::<u16>();
        unsafe { HDAWakeEnableReg::from(ptr.read()) }
    }

    fn state_change_status(&self) -> HDAStateChangeStatusReg {
        let ptr = self.reg_ptr(Self::STATE_CHANGE_STATUS_OFFSET).cast::<u16>();
        unsafe { HDAStateChangeStatusReg::from(ptr.read()) }
    }

    fn set_state_change_status(&mut self, val: HDAStateChangeStatusReg)  {
        let ptr = self.reg_ptr(Self::STATE_CHANGE_STATUS_OFFSET).cast::<u16>();
        unsafe { ptr.write(val.into()) }
    }

    fn interrupt_control(&self) -> HDAInterruptControlReg {
        let ptr = self.reg_ptr(Self::INTERRUPT_CONTROL_OFFSET).cast::<u32>();
        unsafe { HDAInterruptControlReg::from(ptr.read()) }
    }

    fn interrupt_status(&self) -> HDAInterruptStatusReg {
        let ptr = self.reg_ptr(Self::INTERRUPT_STATUS_OFFSET).cast::<u32>();
        unsafe { HDAInterruptStatusReg::from(ptr.read()) }
    }
}

impl SoundDevice {

    fn immediate_command_output(&mut self, command: HDANodeCommand) {
        // Setting the ICB bit which is a necessity to use the ICO
        // interface
        let ptr = self.reg_ptr(0x68).cast::<u16>();
        let mut val = unsafe { ptr.read() };
        val.set_bit(0);
        unsafe { ptr.write(val) };

        // Clear IRV
        let ptr = self.reg_ptr(0x68).cast::<u16>();
        unsafe { ptr.write(0x2) };

        let ptr = self.reg_ptr(Self::IMMEDIATE_COMMAND_OUTPUT_OFFSET).cast::<u32>();
        unsafe { ptr.write(command.into()) }

        let ptr = self.reg_ptr(0x68).cast::<u16>();
        unsafe { ptr.write(0b11) };
    }

    fn immediate_response_input(&self) -> HDANodeResponse {
        let ptr = self.reg_ptr(Self::IMMEDIATE_RESPONSE_INPUT_OFFSET).cast::<u32>();
        unsafe { HDANodeResponse::from(ptr.read()) }
    }

    fn immediate_response_received(&self) -> bool {
        let ptr = self.reg_ptr(0x68).cast::<u16>();
        let val = unsafe { ptr.read() };
        val.get_bit(1) == BitState::Set && val.get_bit(0) == BitState::Unset
    }
}

impl From<PCIDevice> for SoundDevice {
    fn from(mut pci_device: PCIDevice) -> SoundDevice {
        println!("PCI Command: {:b}", pci_device.command());
        pci_device.enable_memory_space_accesses();
        println!("PCI Command: {:b}", pci_device.command());
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

/// Provides global level control over the sound controller and link
#[repr(transparent)]
struct HDAGlobalControlReg(u32);

impl HDAGlobalControlReg {
    /// Tells whether or not unsolicited responses are
    /// accepted by the sound controller and placed into the RIRB
    fn unsolicited_response_accepted(&self) -> bool {
        (self.0 >> 8) & 0b1 == 1
    }

    fn set_unsolicited_response_accepted(&mut self, enable: bool) {
        if enable {
            self.0 = self.0 | (1 << 8);
        } else {
            self.0 = self.0 & !(1 << 8);
        }
    }

    /// Setting the flush control initiates a flush
    fn set_flush_control(&mut self) {
        self.0 = self.0 | (1 << 1);
    }

    /// Returns the value in the CRST bit
    fn controller_reset(&self) -> bool {
        // A value of 0 means reset
        self.0.get_bit(0) == BitState::Set
    }

    /// Sets the CRST bit
    ///
    /// Settings the CRST to 0 causes the HDA controller to transition
    /// to the reset state. Except for certain registers, all registers
    /// and state machines will be reset
    ///
    /// After setting CRST to 0, a 0 must be read to verify that the controller
    /// reset
    fn set_controller_reset(&mut self, set: bool) {
        if set {
            self.0.set_bit(0);
        } else {
            self.0.unset_bit(0);
        }
    }
}

impl From<u32> for HDAGlobalControlReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl Into<u32> for HDAGlobalControlReg {
    fn into(self) -> u32 {
        self.0
    }
}

/// Indicates which bits in the STATESTS register may cause either a wake
/// event or an interrupt
#[repr(transparent)]
struct HDAWakeEnableReg(u16);

impl HDAWakeEnableReg {
    /// Bits that control which sdin signal may generate a wake
    /// or processor interrupt
    ///
    /// If bit n is set, then the sdin signal which corresponds to
    /// codec n will generate a wake event or processor interrupt
    fn sdin_wake_enable(&self) -> u16 {
        // get rid of the bit on the left
        self.0 << 1 >> 1
    }

    /// Sets bit `bit` in the sdin wake enable flags
    /// so the sdin signal corresponding to codec n will generate
    /// a wake event or processor interrupt
    fn set_sdin_wake_enable(&mut self, bit: u8) {
        assert!(bit < 16);
        self.0 = self.0 | (1 << bit) as u16
    }
}

impl From<u16> for HDAWakeEnableReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

/// Indicates that a status change has occured on the link,
/// which usually indicates that a codec has just come out of reset
/// or that a codec is signalling a wake event
///
/// The setting of one of these bits by the controller will cause
/// a processor interrupt to occur if the corresponding bits in the
/// HDAWakeEnableReg is set
#[repr(transparent)]
struct HDAStateChangeStatusReg(u16);

impl HDAStateChangeStatusReg {
    /// Indicates which SDIN signals received a state change event
    fn sdin_state_change_status(&self) -> u16 {
        self.0 << 1 >> 1
    }

    fn clear_sdin_state_change_status(&mut self) {
        // Writing 1s clears the status
        self.0 = 0xffff;
    }
}

impl From<u16> for HDAStateChangeStatusReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl Into<u16> for HDAStateChangeStatusReg {
    fn into(self) -> u16 {
        self.0
    }
}

/// Provides a central point for controlling and monitoring
/// interrupt generation, along with the HDAInterruptStatusReg
#[repr(transparent)]
struct HDAInterruptControlReg(u32);

impl HDAInterruptControlReg {
    /// Tells whether or not device interrupt generation is
    /// enabled
    fn global_interrupt_enable(&self) -> bool {
        // 1 signifies enabled
        self.0 >> 31 == 1
    }

    /// Enables or disables interrupts from the HDA controller device
    fn set_global_interrupt_enable(&mut self, enable: bool) {
        if enable {
            self.0 = self.0 | (1 << 31);
        } else {
            self.0 = self.0 & !(1 << 31);
        }
    }

    /// Tells whether or not general interrupts are enabled for controller functions
    fn controller_interrupt_enable(&self) -> bool {
        self.0 >> 30 & 0b1 == 1
    }

    /// Enables or disables interrupts when a status bit gets set
    /// due to a response interrupt, a response buffer overrun and wake events
    fn set_controller_interrupt_enable(&mut self, enable: bool) {
        if enable {
            self.0 = self.0 | (1 << 30);
        } else {
            self.0 = self.0 & !(1 << 30);
        }
    }

    /// Indicates the current interrupt status of each
    /// interrupt source
    fn stream_interrupt_enable(&self) -> u32 {
        self.0 << 2 >> 2
    }

    fn set_stream_interrupt_enable(&mut self, stream_bit: u8) {
        assert!(stream_bit < 6);
        self.0 = self.0 | (1 << stream_bit);
    }
}

impl From<u32> for HDAInterruptControlReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAInterruptStatusReg(u32);

impl HDAInterruptStatusReg {
    /// True if any interrupt status bit is set
    fn global_interrupt_status(&self) -> bool {
        self.0 >> 31 == 0b1
    }

    /// Status of the general controller interrupt
    ///
    /// A true indicates that an interrupt condition occured due
    /// to a response interrupt, a response overrun or a codec
    /// state change request
    fn controller_interrupt_status(&self) -> bool {
        (self.0 >> 30) & 0b1 == 1
    }

    /// A 1 indicates that an interrupt occured on the corresponding stream
    fn stream_interrupt_status(&self) -> u32 {
        self.0 << 2 >> 2
    }
}

impl From<u32> for HDAInterruptStatusReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDACORBWritePointerReg(u16);

impl HDACORBWritePointerReg {
    /// The offset of the last valid command in the CORB
    ///
    /// This is to be updated manually by the software after
    /// the addition of any commands into the CORB
    fn write_pointer(&self) -> u8 {
        self.0.as_u8()
    }

    fn set_write_pointer(&mut self, wp: u8) {
        self.0.set_bits(0..8, wp.as_u16());
    }
}

impl From<u16> for HDACORBWritePointerReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl Into<u16> for HDACORBWritePointerReg {
    fn into(self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
struct HDACORBReadPointerReg(u16);

impl HDACORBReadPointerReg {
    fn read_pointer_reset(&self) -> bool {
        self.0.get_bit(15) == BitState::Set
    }
    
    /// Sets the read pointer reset field in the register
    ///
    /// Setting this field resets the CORB read pointer to 0
    /// and the hardware will physically set this bit when
    /// the reset is complete.
    /// After setting, the field must be read as set to
    /// verify that the reset completed successfully,
    /// then this field must be set to false again and that
    /// value of false must be read back to verify that the
    /// clear completed successfully
    fn set_read_pointer_reset(&mut self, set: bool) {
        if set {
            self.0.set_bit(15);
        } else {
            self.0.unset_bit(15);
        }
    }

    /// The offset of the last command in the CORB which
    /// the controller has successfully read
    fn read_pointer(&self) -> u8 {
        self.0.get_bits(0..8).as_u8()
    }
}

impl From<u16> for HDACORBReadPointerReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl Into<u16> for HDACORBReadPointerReg {
    fn into(self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
struct HDACORBControlReg(u8);

impl HDACORBControlReg {
    /// Either stops or runs the CORB DMA engine (when read pointer lags write pointer)
    ///
    /// After setting, the value must be read back to verify that
    /// it was set
    fn enable_corb_dma_engine(&mut self, enable: bool) {
        if enable {
            // 1 means run
            self.0.set_bit(1);
        } else {
            // 0 means stop
            self.0.unset_bit(1);
        }
    }

    fn corb_dma_engine_enabled(&self) -> bool {
        self.0.get_bit(1) == BitState::Set
    }

    fn enable_memory_error_interrupt(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(0);
        } else {
            self.0.unset_bit(0);
        }
    }

    /// Tells the controller to generate an interrupt
    /// if the Memory Error Interrupt status bit is set
    fn memory_error_interrupt_enabled(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }
}

impl From<u8> for HDACORBControlReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for HDACORBControlReg {
    fn into(self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
struct HDACORBStatusReg(u8);

impl HDACORBStatusReg {
    /// Tells whether or not the controller has detected an error
    /// between the controller and memory
    fn memory_error_indication(&self) -> bool {
        self.0 & 0b1 == 1
    }
}

impl From<u8> for HDACORBStatusReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDACORBSizeReg(u8);

impl HDACORBSizeReg {
    fn size_capability(&self) -> HDARingBufferSizeCapability {
        HDARingBufferSizeCapability(self.0 >> 4)
    }

    /// The number of entries that can be in the CORB at once
    ///
    /// This value determines when the address counter in the DMA controller
    /// will wrap around
    fn corb_size(&self) -> HDARingBufferSize {
        match self.0.get_bits(0..2) {
            0b00 => HDARingBufferSize::Two,
            0b01 => HDARingBufferSize::Sixteen,
            0b10 => HDARingBufferSize::TwoFiftySix,
            _ => unreachable!("0b11 is reserved and all other values are impossible")
        }
    }

    fn set_corb_size(&mut self, size: HDARingBufferSize) {
        self.0.set_bits(0..2, size as u8);
    }
}

/// A bitmask indicating the CORB sizes supported by the controller
///
/// 0001 - 2 entries
/// 0010 - 16 entries
/// 0100 - 256 entries
/// 1000 - reserved
#[repr(transparent)]
struct HDARingBufferSizeCapability(u8);

impl HDARingBufferSizeCapability {
    fn size2_supported(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }
    fn size16_supported(&self) -> bool {
        self.0.get_bit(1) == BitState::Set
    }
    fn size256_supported(&self) -> bool {
        self.0.get_bit(2) == BitState::Set
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum HDARingBufferSize {
    Two = 0b00,
    Sixteen = 0b01,
    TwoFiftySix = 0b10
}

impl HDARingBufferSize {
    fn entries_as_u16(&self) -> u16 {
        match self {
            Self::Two => 2,
            Self::Sixteen => 16,
            Self::TwoFiftySix => 256
        }
    }
}

impl From<u8> for HDACORBSizeReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for HDACORBSizeReg {
    fn into(self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
struct HDARIRBWritePointerReg(u16);

impl HDARIRBWritePointerReg {
    /// Resets the RIRB write pointer to 0
    ///
    /// The DMA engine must be stopped prior to
    /// calling this function, or else DMA transfer may be corrupted.
    /// This bit will always be read as 0
    fn reset_write_pointer(&mut self) {
        // Writing a 1 resets the bit to 0
        self.0.set_bit(15)
    }

    /// Indicates the offset of the last valid RIRB entry written
    /// by the DMA controller
    fn write_pointer(&self) -> u8 {
        self.0 as u8
    }
}

impl From<u16> for HDARIRBWritePointerReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl Into<u16> for HDARIRBWritePointerReg {
    fn into(self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
struct HDAResponseInterruptCountReg(u16);

impl HDAResponseInterruptCountReg {
    /// The number of responses that have been sent
    /// to the RIRB
    fn response_interrupt_count(&self) -> u16 {
        if self.0 as u8 == 0 {
            256
        } else {
            self.0 as u8 as u16
        }
    }

    fn set_response_interrupt_count(&mut self, val: u8) {
        self.0.set_bits(0..8, val.into());
    }
}

impl From<u16> for HDAResponseInterruptCountReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl Into<u16> for HDAResponseInterruptCountReg {
    fn into(self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
struct HDARIRBControlReg(u8);

impl HDARIRBControlReg {
    /// Signifies if the hardware will generate an interrupt
    /// when the Response Overrun Interrupt Status bit is set
    fn response_overrun_interrupt_control(&self) -> bool {
        self.0.get_bit(2) == BitState::Set
    }

    fn set_response_overrun_interrupt_control(&mut self, set: bool) {
        if set {
            self.0.unset_bit(2);
        } else {
            self.0.set_bit(2);
        }
    }

    /// Either stops or runs the RIRB DMA engine (when response queue is not empty)
    ///
    /// After setting, the value must be read back to verify that
    /// it was set
    fn rirb_dma_engine_enabled(&self) -> bool {
        self.0.get_bit(1) == BitState::Set
    }

    fn enable_rirb_dma_engine(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(1);
        } else {
            self.0.unset_bit(1);
        }
    }

    fn enable_interrupt(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(0);
        } else {
            self.0.unset_bit(0);
        }
    }
}

impl From<u8> for HDARIRBControlReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for HDARIRBControlReg {
    fn into(self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
struct HDARIRBStatusReg(u8);

impl HDARIRBStatusReg {
    /// Returns true when an overrun occurs in the RIRB
    ///
    /// An interrupt may be generated if the Response Overrun
    /// Interrupt Control bit is set.
    /// A value of true signifies that the RIRB DMA engine is not
    /// able to write the incoming responses to memory before
    /// additional incoming responses overrun the internal FIFO
    ///
    /// When hardware detects an overrun, it will drop the responses
    /// which overrun the buffer and set this status to indicate the
    /// error condition
    fn response_overrun_interrupt_status(&self) -> bool {
        (self.0 >> 2) & 1 == 1
    }

    fn clear_response_overrun_interrupt_status(&mut self) {
        // Writing a 1 clears the status
        self.0 = self.0 | (1 << 2);
    }

    /// This returns true when an interrupt has been generated
    /// after n number of responses are sent to the RIRB or when an
    /// empty response slot is encountered on all SDATAIN inputs
    fn response_interrupt_flag(&self) -> bool {
        self.0 & 1 == 1
    }

    fn clear_response_interrupt_flag(&mut self) {
        // Writing a 1 clears the flag
        self.0 = self.0 | 0b1;
    }
}

impl From<u8> for HDARIRBStatusReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDARIRBSizeReg(u8);

impl HDARIRBSizeReg {
    fn size_capability(&self) -> HDARingBufferSizeCapability {
        HDARingBufferSizeCapability(self.0 >> 4)
    }

    /// The number of entries that can be in the RIRB at once
    ///
    /// This value determines when the address counter in the DMA controller
    /// will wrap around
    fn rirb_size(&self) -> HDARingBufferSize {
        match self.0 & 0b11 {
            0b00 => HDARingBufferSize::Two,
            0b01 => HDARingBufferSize::Sixteen,
            0b10 => HDARingBufferSize::TwoFiftySix,
            _ => unreachable!("0b11 is reserved and all other values are impossible")
        }
    }

    fn set_rirb_size(&mut self, size: HDARingBufferSize) {
        self.0.set_bits(0..2, size as u8);
    }
}

impl From<u8> for HDARIRBSizeReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for HDARIRBSizeReg {
    fn into(self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
struct HDADMAPosLowerBaseAddrReg(u32);

impl HDADMAPosLowerBaseAddrReg {
    fn lower_base_addr(&self) -> u32 {
        self.0.get_bits(7..32)
    }

    fn set_lower_base_addr(&mut self, addr: u32) {
        self.0.set_bits(7..32, addr >> 7);
    }

    /// Returns true if the controller writes the DMA
    /// positions of each of the DMA engines to the buffer
    /// in main memory periodically
    fn dma_position_buffer_enabled(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }

    fn enable_dma_position_buffer(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(0)
        } else {
            self.0.unset_bit(0)
        }
    }
}

impl From<u32> for HDADMAPosLowerBaseAddrReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl Into<u32> for HDADMAPosLowerBaseAddrReg {
    fn into(self) -> u32 {
        self.0
    }
}

// Only the lower 3 bytes are the values in the 
// actual register, since the register size is 3 bytes
#[repr(transparent)]
struct HDAStreamDescriptorControlReg(u32);

impl HDAStreamDescriptorControlReg {
    /// Returns a number between 0 and 15, where 0 means unused
    /// and 1 to 15 is the tag associated with the data being
    /// transferred on the link 
    fn stream_number(&self) -> u8 {
        self.0.get_bits(20..24) as u8
    }

    fn set_stream_number(&mut self, n: u8) {
        assert!(n > 0 && n < 16);
        self.0.set_bits(20..24, n as u32);
    }

    /// Returns true if an interrupt will be generated
    /// when the Descriptor Error Status bit is set
    fn descriptor_error_interrupt_enabled(&self) -> bool {
        self.0.get_bit(4) == BitState::Set
    }

    fn enable_descriptor_error_interrupt(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(4);
        } else {
            self.0.unset_bit(4);
        }
    }

    /// Returns true if an interrupt will be generated when 
    /// an FIFO error occurs (overrun for input, under run for output)
    fn fifo_interrupt_enabled(&self) -> bool {
        self.0.get_bit(3) == BitState::Set
    }

    fn enable_fifo_interrupt(&mut self, enable: bool) {
        if enable {
            self.0.set_bit(3);
        } else {
            self.0.unset_bit(3);
        }
    }

    /// Returns true if an interrupt will be generated when
    /// a buffer completes with the InterruptOnCompletion bit
    /// set in its descriptor
    fn interrupt_on_completion_enabled(&self) -> bool {
        self.0.get_bit(2) == BitState::Set
    }

    /// Returns true if the DMA engine associated with this
    /// input stream is enabled to transfer data in the FIFO
    /// to main memory
    fn stream_run(&self) -> bool {
        self.0.get_bit(1) == BitState::Set
    }

    /// When set to false, the DMA engine associated with this
    /// stream is disabled
    fn set_stream_run(&mut self, run: bool) {
        if run {
            self.0.set_bit(1);
        } else {
            self.0.unset_bit(1);
        }
    }

    /// Tells whether or not the stream is in a reset state
    fn stream_reset(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }

    /// Places the stream in a reset state
    ///
    /// After resetting the stream, a true
    /// must be returned from `stream_reset` to verify that
    /// the stream is in reset
    fn enter_stream_reset(&mut self) {
        self.0.set_bit(0);
    }

    /// Removes the stream from its reset state
    ///
    /// After exiting reset, a false must be returned from
    /// `stream_reset` to verify that the stream is ready to begin
    /// operation
    fn exit_stream_reset(&mut self) {
        self.0.unset_bit(0);
    }
}

impl From<u32> for HDAStreamDescriptorControlReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorStatusReg(u8);

impl HDAStreamDescriptorStatusReg {
    /// Returns true when the output DMA FIFO contains
    /// enough data to maintain the stream on the link
    fn fifo_ready(&self) -> bool {
        self.0.get_bit(5) == BitState::Set
    }

    /// Returns true if an error has occured during the
    /// fetch of a descriptor
    fn descriptor_error(&self) -> bool {
        self.0.get_bit(4) == BitState::Set
    }

    /// Returns true when an FIFO error occurs
    fn fifo_error(&self) -> bool {
        self.0.get_bit(3) == BitState::Set
    }

    fn clear_fifo_error(&mut self) {
        // The bit is cleared by writing a 1 to the position
        self.0.set_bit(3);
    }

    /// Returns true if the last byte of data for the current
    /// descriptor has been fetched from memory and put into the
    /// DMA FIFO and the current descriptor has the InterruptOnCompletion
    /// bit set
    fn buffer_completion_interrupt_status(&self) -> bool {
        self.0.get_bit(2) == BitState::Set
    }
}

impl From<u8> for HDAStreamDescriptorStatusReg {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorLinkPosReg(u32);

impl HDAStreamDescriptorLinkPosReg {
    /// Indicates the number of bytes that have been received
    /// off the link
    fn link_pos_in_buffer(&self) -> u32 {
        self.0
    }
}

impl From<u32> for HDAStreamDescriptorLinkPosReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorCyclicBufferLenReg(u32);

impl HDAStreamDescriptorCyclicBufferLenReg {
    /// Indicates the number of bytes in the complete
    /// cyclic buffer
    ///
    /// The link position in buffer will be reset when it
    /// reaches this value (because it is cyclic)
    fn cyclic_buffer_len(&self) -> u32 {
        self.0
    }

    /// Sets the cyclic buffer length
    ///
    /// This function can only be called after 
    /// global reset, controller reset or stream reset has
    /// occured.
    ///
    /// This must not be called until the next reset occurs and
    /// the run bit is 0
    fn set_cyclic_buffer_len(&mut self, len: u32) {
        self.0 = len;
    }
}

impl From<u32> for HDAStreamDescriptorCyclicBufferLenReg {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorLastValidIndexReg(u16);

impl HDAStreamDescriptorLastValidIndexReg {
    /// Indicates the last valid index for the last valid
    /// buffer in the BufferDescriptorList
    /// 
    /// After the controller has processed this descriptor,
    /// it will wrap back to the first descriptor in the list
    /// and continue processing
    fn last_valid_index(&self) -> u8 {
        self.0 as u8
    }

    fn set_last_valid_index(&mut self, idx: u8) {
        self.0.set_bits(0..8, idx.into());
    }
}

impl From<u16> for HDAStreamDescriptorLastValidIndexReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorFIFOSizeReg(u16);

impl HDAStreamDescriptorFIFOSizeReg {
    /// Indicates the maximum number off bytes that could be fetched
    /// by the controller at a time
    fn fifo_size(&self) -> u16 {
        self.0
    }
}

impl From<u16> for HDAStreamDescriptorFIFOSizeReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

#[repr(transparent)]
struct HDAStreamDescriptorFormatReg(u16);

impl HDAStreamDescriptorFormatReg {
    fn sample_base_rate(&self) -> SampleBaseRate {
        match self.0.get_bit(14) {
            BitState::Set => SampleBaseRate::KHz44P1,
            BitState::Unset => SampleBaseRate::KHz48
        }
    }

    fn set_sample_base_rate(&mut self, rate: SampleBaseRate) {
        match rate {
            SampleBaseRate::KHz44P1 => self.0.set_bit(14),
            SampleBaseRate::KHz48 => self.0.unset_bit(14)
        }
    }

    fn sample_base_rate_multiple(&self) -> SampleBaseRateMultiple {
        self.0.get_bits(11..14).as_u8().try_into().unwrap()
    }

    fn set_sample_base_rate_multiple(&mut self, rate_mult: SampleBaseRateMultiple) {
        self.0.set_bits(11..14, rate_mult as u8 as u16);
    }

    fn sample_base_rate_divisor(&self) -> SampleBaseRateDivisor {
        self.0.get_bits(8..11).as_u8().try_into().unwrap()
    }

    fn set_sample_base_rate_divisor(&mut self, rate_divisor: SampleBaseRateDivisor) {
        self.0.set_bits(8..11, rate_divisor as u8 as u16);
    }

    fn bits_per_sample(&self) -> BitsPerSample {
        self.0.get_bits(4..7).as_u8().try_into().unwrap()
    }

    fn set_bits_per_sample(&mut self, bps: BitsPerSample) {
        self.0.set_bits(4..7, bps as u8 as u16);
    }

    fn number_of_channels(&self) -> NumOfChannels {
        self.0.get_bits(0..4).as_u8().try_into().unwrap()
    }

    fn set_number_of_channels(&mut self, n: NumOfChannels) {
        self.0.set_bits(0..4, n as u8 as u16);
    }
}

impl From<u16> for HDAStreamDescriptorFormatReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

enum SampleBaseRate {
    // 44.1 kHz
    KHz44P1,
    // 48 kHz
    KHz48
}

#[repr(u8)]
enum SampleBaseRateMultiple {
    KHz48OrLess = 0b000,
    X2 = 0b001,
    X3 = 0b010,
    X4 = 0b011
}

impl TryInto<SampleBaseRateMultiple> for u8 {
    type Error = ();
    fn try_into(self) -> Result<SampleBaseRateMultiple, ()> {
        match self {
            0b000 => Ok(SampleBaseRateMultiple::KHz48OrLess),
            0b001 => Ok(SampleBaseRateMultiple::X2),
            0b010 => Ok(SampleBaseRateMultiple::X3),
            0b011 => Ok(SampleBaseRateMultiple::X4),
            _ => Err(())
        }
    }
}

#[repr(u8)]
enum SampleBaseRateDivisor {
    One = 0b000,
    Two = 0b001,
    Three = 0b010,
    Four = 0b011,
    Five = 0b100,
    Six = 0b101,
    Seven = 0b110,
    Eight = 0b111
}

impl TryInto<SampleBaseRateDivisor> for u8 {
    type Error = ();
    fn try_into(self) -> Result<SampleBaseRateDivisor, ()> {
        match self {
            0b000 => Ok(SampleBaseRateDivisor::One),
            0b001 => Ok(SampleBaseRateDivisor::Two),
            0b010 => Ok(SampleBaseRateDivisor::Three),
            0b011 => Ok(SampleBaseRateDivisor::Four),
            0b100 => Ok(SampleBaseRateDivisor::Five),
            0b101 => Ok(SampleBaseRateDivisor::Six),
            0b110 => Ok(SampleBaseRateDivisor::Seven),
            0b111 => Ok(SampleBaseRateDivisor::Eight),
            _ => Err(())
        }
    }
}

#[derive(PartialEq)]
#[repr(u8)]
enum BitsPerSample {
    Eight = 0b000,
    Sixteen = 0b001,
    Twenty = 0b010,
    TwentyFour = 0b011,
    ThirtyTwo = 0b100
}

impl TryInto<BitsPerSample> for u8 {
    type Error = ();
    fn try_into(self) -> Result<BitsPerSample, ()> {
        match self {
            0b000 => Ok(BitsPerSample::Eight),
            0b001 => Ok(BitsPerSample::Sixteen),
            0b010 => Ok(BitsPerSample::Twenty),
            0b011 => Ok(BitsPerSample::TwentyFour),
            0b100 => Ok(BitsPerSample::ThirtyTwo),
            _ => Err(())
        }
    }
}

#[repr(u8)]
enum NumOfChannels {
    One = 0b0000,
    Two = 0b0001,
    Three = 0b0010,
    Four = 0b0011,
    Five = 0b0100,
    Six = 0b0101,
    Seven = 0b0110,
    Eight = 0b0111,
    Nine = 0b1000,
    Ten = 0b1001,
    Eleven = 0b1010,
    Twelve = 0b1011,
    Thirteen = 0b1100,
    Fourteen = 0b1101,
    Fifteen = 0b1110,
    Sixteen = 0b1111
}

impl TryInto<NumOfChannels> for u8 {
    type Error = ();
    fn try_into(self) -> Result<NumOfChannels, ()> {
        match self {
            0b0000 => Ok(NumOfChannels::One),
            0b0001 => Ok(NumOfChannels::Two),
            0b0010 => Ok(NumOfChannels::Three),
            0b0011 => Ok(NumOfChannels::Four),
            0b0100 => Ok(NumOfChannels::Five),
            0b0101 => Ok(NumOfChannels::Six),
            0b0110 => Ok(NumOfChannels::Seven),
            0b0111 => Ok(NumOfChannels::Eight),
            0b1000 => Ok(NumOfChannels::Nine),
            0b1001 => Ok(NumOfChannels::Ten),
            0b1010 => Ok(NumOfChannels::Eleven),
            0b1011 => Ok(NumOfChannels::Twelve),
            0b1100 => Ok(NumOfChannels::Thirteen),
            0b1101 => Ok(NumOfChannels::Fourteen),
            0b1110 => Ok(NumOfChannels::Fifteen),
            0b1111 => Ok(NumOfChannels::Sixteen),
            _ => Err(())
        }
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
enum PCIBaseAddrReg {
    Memory(MemBAR),
    IO(IOBAR)
}

#[derive(Debug, PartialEq)]
enum PCIBaseAddrKind {
    Memory,
    IO
}

impl PCIBaseAddrReg {
    fn addr(&self) -> u64 {
        match self {
            Self::Memory(mbar) => mbar.addr(),
            Self::IO(iobar) => iobar.addr()
        }
    }

    fn kind(&self) -> PCIBaseAddrKind {
        match self {
            Self::Memory(_) => PCIBaseAddrKind::Memory,
            Self::IO(_) => PCIBaseAddrKind::IO
        }
    }
}

impl TryFrom<(u32, u32)> for PCIBaseAddrReg {
    type Error = &'static str;
    fn try_from(val: (u32, u32)) -> Result<PCIBaseAddrReg, Self::Error> {
        match val.0 & 0x1 {
            0 => Ok(Self::Memory(MemBAR(val.0, val.1))),
            1 => Ok(Self::IO(IOBAR(val.0))),
            _ => Err("Expected either a 0 or 1 in bit 0")
        }
    }
}

struct MemBAR(u32, u32);

impl MemBAR {
    /// Returns the 16 byte aligned base address
    fn addr(&self) -> u64 {
        match self.0.get_bits(1..3) {
            // 32 bit address
            0x0 => (self.0 & 0xfffffff0).as_u64(),
            // 64 bit address
            0x2 => (self.0 & 0xfffffff0).as_u64() + ((self.1 & 0xffffffff).as_u64() << 32),
            _ => panic!("Unexpected memory space BAR type")
        }
    }
}

struct IOBAR(u32);

impl IOBAR {
    /// Returns the 4 byte aligned base address
    fn addr(&self) -> u64 {
        (self.0 & 0xfffffffc).as_u64()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct InterruptOnCompletion(u32);

impl InterruptOnCompletion {
    fn is_set(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }

    fn set(&mut self, set: bool) {
        if set {
            self.0.set_bit(0);
        } else {
            self.0.unset_bit(0);
        }
    }

    /// Defaults to setting interrupt on completion
    /// to false
    fn new() -> Self {
        Self(0)
    }
}

/// A description of a SampleBuffer which is a piece of
/// the whole cyclic stream buffer
#[derive(Clone, Copy)]
#[repr(C)]
struct BufferDescriptorListEntry {
    /// The starting address of the sample buffer, which
    /// must be 128 byte aligned
    addr: *const u8,
    /// The length of the buffer described in bytes
    len: u32,
    /// Interrupt on Completion
    ///
    /// Used to determine if the controller will generate an
    /// interrupt when the last byte of the buffer has been
    /// fetched by the DMA engine (If enabled by the stream's interrupt
    /// on completion enable bit)
    interrupt_on_completion: InterruptOnCompletion
}

impl BufferDescriptorListEntry {
    fn new(addr: *const u8, len: u32) -> Self {
        Self {
            addr,
            len,
            interrupt_on_completion: InterruptOnCompletion::new()
        }
    }

    fn null() -> Self {
        Self {
            addr: core::ptr::null_mut() as *mut u8,
            len: 0,
            interrupt_on_completion: InterruptOnCompletion::new()
        }
    }
}

#[repr(C, align(128))]
struct BufferDescriptorList {
    // 256 is the max allowed number of entries
    entries: [BufferDescriptorListEntry; 256],
    curr_index: usize
}

impl BufferDescriptorList {
    fn new() -> Self {
        Self {
            entries: [BufferDescriptorListEntry::null(); 256],
            curr_index: 0
        }
    }

    fn add_entry(&mut self, entry: BufferDescriptorListEntry) -> Result<(), ()> {
        if self.curr_index >= 256 {
            return Err(());
        }
        self.entries[self.curr_index] = entry;
        self.curr_index += 1;
        Ok(())
    }
}

/// A 20 bit HDA verb
#[repr(transparent)]
struct HDANodeCommandVerb(u32);

impl HDANodeCommandVerb {
    // Verb ids
    const GET_PARAMETER: u32 = 0xf00;
    const SET_BEEP: u32 = 0x70a;
    const GET_CONN_LIST_ENTRY: u32 = 0xf02;
    const CONFIG_DEFAULT: u32 = 0xf1c;
    
    fn get_parameter(param_id: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, param_id.into());
        val.set_bits(8..20, Self::GET_PARAMETER);
        Self(val)
    }

    fn get_conn_list_entry(entry_index: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, entry_index.into());
        val.set_bits(8..20, Self::GET_CONN_LIST_ENTRY);
        Self(val)
    }

    fn get_pin_config_defaults() -> Self {
        let mut val = 0u32;
        val.set_bits(8..20, Self::CONFIG_DEFAULT);
        Self(val)
    }
}

impl Into<u32> for HDANodeCommandVerb {
    fn into(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
struct HDANodeCommand(u32);

impl HDANodeCommand {
    // Parameter Ids for getting parameters
    const PARAMETER_NODE_COUNT: u8 = 0x04;
    const PARAMETER_FUNC_GROUP_TYPE: u8 = 0x05;
    const PARAMETER_AFG_WIDGET_CAPABILITIES: u8 = 0x09;
    const PARAMETER_CONN_LIST_LEN: u8 = 0x0e;

    /// The null command
    fn null() -> HDANodeCommand {
        Self(0)
    }

    /// Returns a command to retrieve info about a specific
    /// codec root node, function group or widget with a node
    /// id of `node_id` in a codec at codec address `codec_addr`
    fn get_parameter(codec_addr: u8, node_id: u8, param_id: u8) -> Self {
        // Bits 28..=31 is the codec_addr
        // Bits 20..=27 is the node_id
        // Bits 0..=19 is the verb
        let verb = HDANodeCommandVerb::get_parameter(param_id);
        Self::command(codec_addr, node_id, verb)
    }

    fn get_node_count(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_NODE_COUNT)
    }

    fn function_group_type(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_FUNC_GROUP_TYPE)
    }

    fn afg_widget_capabilities(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_AFG_WIDGET_CAPABILITIES)
    }

    fn get_conn_list_len(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_CONN_LIST_LEN)
    }

    fn get_conn_list_entry(codec_addr: u8, node_id: u8, entry_index: u8) -> Self {
        let verb = HDANodeCommandVerb::get_conn_list_entry(entry_index);
        Self::command(codec_addr, node_id, verb)
    }

    fn get_pin_config_defaults(codec_addr: u8, node_id: u8) -> Self {
        let verb = HDANodeCommandVerb::get_pin_config_defaults();
        Self::command(codec_addr, node_id, verb)
    }

    fn command(codec_addr: u8, node_id: u8, verb: HDANodeCommandVerb) -> Self {
        let mut val = 0u32;
        val.set_bits(0..20, verb.into());
        val.set_bits(20..28, node_id.into());
        val.set_bits(28..32, codec_addr.into());
        Self(val)
    }
}

impl Into<u32> for HDANodeCommand {
    fn into(self) -> u32 {
        self.0
    }
}

/// A response received from the HDA controller into
/// the RIRB
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C, packed)]
struct HDANodeResponse {
    /// The response data received from the codec
    response: u32,
    /// Extra information added to the response by the controller
    response_info: ResponseInfo
}

impl HDANodeResponse {
    fn null() -> Self {
        Self {
            response: 0,
            response_info: ResponseInfo(0)
        }
    }

    /// Interpretes the response field as a response
    /// to a `get_node_count` command
    fn node_count_resp(&self) -> Result<HDANodeResponseNodeCount, ()> {
        if self.response != 0 {
            Ok(HDANodeResponseNodeCount(self.response))
        } else {
            Err(())
        }
    }

    /// Interpretes the response field as a response
    /// to a `function_group_type` command
    fn func_group_type_resp(&self) -> Result<HDANodeResponseFunctionGroupType, ()> {
        if self.response != 0 {
            Ok(HDANodeResponseFunctionGroupType(self.response))
        } else {
            Err(())
        }
    }

    fn afg_widget_capabilities_resp(&self) -> Result<HDANodeResponseAFGWidgetCap, ()> {
        if self.response != 0 {
            Ok(HDANodeResponseAFGWidgetCap(self.response))
        } else {
            Err(())
        }
    }

    fn get_conn_list_entry_resp(&self, long: bool) -> Result<HDANodeResponseGetConnListEntry, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(HDANodeResponseGetConnListEntry { resp: self.response, long })
        }
    }

    fn get_conn_list_len_resp(&self) -> Result<HDANodeResponseGetConnListLen, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(HDANodeResponseGetConnListLen(self.response))
        }
    }

    fn get_pin_config_defaults_resp(&self) -> Result<HDANodeResponsePinConfigDefaults, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(HDANodeResponsePinConfigDefaults(self.response))
        }
    }
}

impl From<u32> for HDANodeResponse {
    fn from(val: u32) -> Self {
        Self {
            response: val,
            response_info: ResponseInfo(0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
struct ResponseInfo(u32);

impl ResponseInfo {
    /// The address of the codec that sent the response
    ///
    /// This is always a value in the range 0..=15
    fn codec(&self) -> u8 {
        self.0.get_bits(0..4).as_u8()
    }
    
    /// Tells whether or not the response is solicited
    fn solicited(&self) -> bool {
        self.0.get_bit(4) == BitState::Unset
    }
}

#[repr(transparent)]
struct HDANodeResponseNodeCount(u32);

impl HDANodeResponseNodeCount {
    /// The node id of the first node in the function
    /// or widget group belonging to the node address
    /// which the corresponding `get_node_count` command
    /// was sent with
    fn start_node_number(&self) -> u8 {
        self.0.get_bits(16..24).as_u8()
    }

    /// The number of nodes in the function or widget
    /// group belonging to the node address which the
    /// corresponding `get_node_count` command was sent with
    ///
    /// This parameter can be used to determine the addresses of
    /// the other nodes in the group, along with the start node number
    /// because node ids are consecutive
    fn number_of_nodes(&self) -> u8 {
        self.0.get_bits(0..8).as_u8()
    }
}

#[repr(transparent)]
struct HDANodeResponseFunctionGroupType(u32);

impl HDANodeResponseFunctionGroupType {
    /// Tells what type of function group is the node
    /// which corresponds to the codec address and node id
    /// which was sent with the command that prompted this response
    fn node_type(&self) -> HDANodeFunctionGroupType {
        self.0.get_bits(0..8).as_u8().try_into().unwrap()
    }

    fn capable_of_unsolicited_responses(&self) -> bool {
        self.0.get_bit(8) == BitState::Set
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum HDANodeFunctionGroupType {
    /// Audio Function Group
    ///
    /// This type is used for playing audio
    AFG = 0x01,
    /// Modem Function Group
    MFG = 0x02
}

impl TryInto<HDANodeFunctionGroupType> for u8 {
    type Error = ();
    fn try_into(self) -> Result<HDANodeFunctionGroupType, ()> {
        match self {
            0x01 => Ok(HDANodeFunctionGroupType::AFG),
            0x02 => Ok(HDANodeFunctionGroupType::MFG),
            _ => Err(())
        }
    }
}

#[repr(transparent)]
struct HDANodeResponseAFGWidgetCap(u32);

impl HDANodeResponseAFGWidgetCap {
    fn widget_type(&self) -> HDAAFGWidgetType {
        self.0.get_bits(20..24).as_u8().try_into().unwrap()
    }

    fn connection_list_present(&self) -> bool {
        self.0.get_bit(8) == BitState::Set
    }
}

struct HDANodeResponseGetConnListEntry {
    resp: u32,
    /// Determines the number of connection list entries
    /// that are in `resp`
    long: bool
}

impl HDANodeResponseGetConnListEntry {
    fn entries(&self) -> ConnListEntryIter {
        ConnListEntryIter {
            entries: self.resp,
            long: self.long,
            curr_index: 0
        }
    }
}

struct ConnListEntryIter {
    entries: u32,
    long: bool,
    curr_index: u8
}

impl Iterator for  ConnListEntryIter {
    type Item = u16;
    fn next(&mut self) -> Option<Self::Item> {
        if self.long && self.curr_index >= 2 {
            return None;
        }
        if !self.long && self.curr_index >= 4 {
            return None;
        }
        let entries: [u16; 4];
        if self.long {
            entries = [self.entries as u16, (self.entries >> 16) as u16, 0, 0];
        } else {
            entries = [
                self.entries.as_u8().as_u16(),
                (self.entries >> 8).as_u8().as_u16(),
                (self.entries >> 16).as_u8().as_u16(),
                (self.entries >> 24).as_u8().as_u16()
            ];
        }
        self.curr_index += 1;
        match entries[(self.curr_index - 1).as_usize()] {
            0 => None,
            other => Some(other)
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct HDANodeResponseGetConnListLen(u32);

impl HDANodeResponseGetConnListLen {
    /// Indicates whether or not the items in the connection
    /// are in a long form (16 bits) or a short form (8 bits)
    fn long_form(&self) -> bool {
        self.0.get_bit(7) == BitState::Set
    }

    fn conn_list_len(&self) -> u8 {
        self.0.get_bits(0..7).as_u8()
    }
}

struct HDANodeResponsePinConfigDefaults(u32);

impl HDANodeResponsePinConfigDefaults {
    /// Indicates the external connectivity of
    /// the pin complex
    fn port_connectivity(&self) -> PortConnectivity {
        match self.0.get_bits(30..32) {
            0b00 => PortConnectivity::Jack,
            0b01 => PortConnectivity::None,
            0b10 => PortConnectivity::FixedFuncDevice,
            0b11 => PortConnectivity::JackAndInternalDevice,
            _ => unreachable!()
        }
    }

    fn default_device(&self) -> DefaultDevice {
        match self.0.get_bits(20..24) {
            0x0 => DefaultDevice::LineOut,
            0x1 => DefaultDevice::Speaker,
            0x2 => DefaultDevice::HPOut,
            0x3 => DefaultDevice::CD,
            0x4 => DefaultDevice::SPDIFOut,
            0x5 => DefaultDevice::DigitalOtherOut,
            0x6 => DefaultDevice::ModemLineSide,
            0x7 => DefaultDevice::ModemHandsetSide,
            0x8 => DefaultDevice::LineIn,
            0x9 => DefaultDevice::AUX,
            0xa => DefaultDevice::MicIn,
            0xb => DefaultDevice::Telephony,
            0xc => DefaultDevice::SPDIFIn,
            0xd => DefaultDevice::DigitalOtherIn,
            0xe..=0xf => DefaultDevice::Other,
            _ => unreachable!()
        }
    }
}

/// Tells the physical connection status of a
/// pin complex
#[derive(Debug, Clone, Copy, PartialEq)]
enum PortConnectivity {
    /// The port complex is connected to a jack
    Jack,
    /// A fixed function device (integrated speaker, integrated mic) is attached
    FixedFuncDevice,
    /// Both a jack and an internal device are connected
    JackAndInternalDevice,
    /// No physical connection
    None
}

/// Tells the intended use of the jack or device
/// connected to a pin complex
#[derive(Debug, Clone, Copy, PartialEq)]
enum DefaultDevice {
    LineOut,
    Speaker,
    HPOut,
    CD,
    SPDIFOut,
    DigitalOtherOut,
    ModemLineSide,
    ModemHandsetSide,
    LineIn,
    AUX,
    MicIn,
    Telephony,
    SPDIFIn,
    DigitalOtherIn,
    Other
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HDAAFGWidgetType {
    AudioOutput,
    AudioInput,
    AudioMixer,
    AudioSelector,
    PinComplex,
    Power,
    VolumeKnob,
    BeepGenerator,
    Other
}

impl TryInto<HDAAFGWidgetType> for u8 {
    type Error = ();
    fn try_into(self) -> Result<HDAAFGWidgetType, ()> {
        match self {
            0x0 => Ok(HDAAFGWidgetType::AudioOutput),
            0x1 => Ok(HDAAFGWidgetType::AudioInput),
            0x2 => Ok(HDAAFGWidgetType::AudioMixer),
            0x3 => Ok(HDAAFGWidgetType::AudioSelector),
            0x4 => Ok(HDAAFGWidgetType::PinComplex),
            0x5 => Ok(HDAAFGWidgetType::Power),
            0x6 => Ok(HDAAFGWidgetType::VolumeKnob),
            0x7 => Ok(HDAAFGWidgetType::BeepGenerator),
            0xf => Ok(HDAAFGWidgetType::Other),
            _ => Err(())
        }
    }
}

/// The Command Outbound Ring buffer as specified in
/// section 4.4.1 of the HDA spec, revision 1.0a
///
/// The CORB is a circular buffer (circular means when the buffer is read through,
/// reading starts from the beginning again), in memory used to pass commands to the
/// codecs connected to the HDA link
///
/// According to the spec, the entry number is programmable to 2, 16 or 256,
#[repr(C, align(128))]
struct CORB {
    /// The commands to be fetched by the HDA controller
    commands: [HDANodeCommand; 256],
    /// Indicates to the hardware the last valid command in the `commands` array
    write_pointer: usize,
    /// The max number of possible entries which is
    /// programmable to 2, 16 or 256
    size: HDARingBufferSize
}

impl CORB {
    fn new(size: HDARingBufferSize) -> Self {
        Self {
            commands: [HDANodeCommand::null(); 256],
            write_pointer: 0,
            size
        }
    }

    fn add_command(&mut self, command: HDANodeCommand, sound_device: &mut SoundDevice) {
        self.write_pointer = (self.write_pointer + 1) % self.size.entries_as_u16().as_usize();
        self.commands[self.write_pointer] = command;
        let mut corbwp_reg = sound_device.corbwp();
        corbwp_reg.set_write_pointer(self.write_pointer.as_u8());
        sound_device.set_corbwp(corbwp_reg);
        /*if !sound_device.corb_control().corb_dma_engine_enabled() {
            sound_device.enable_corb_dma_engine(true);
        }*/
    }
    
    fn size(&self) -> HDARingBufferSize {
        self.size
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
    read_pointer: usize,
    /// The number of possible entries
    size: HDARingBufferSize
}

impl RIRB {
    fn new(size: HDARingBufferSize) -> Self {
        Self {
            responses: [HDANodeResponse::null(); 256],
            read_pointer: 0,
            size
        }
    }

    fn read_next_response(&mut self, sound_device: &mut SoundDevice) -> HDANodeResponse {
        /*if !sound_device.rirb_control().rirb_dma_engine_enabled() {
            sound_device.enable_rirb_dma_engine(true);
        }*/
        // The buffer is circular, so when the last entry is reached
        // the read pointer should wrap around
        self.read_pointer = (self.read_pointer + 1) % self.size.entries_as_u16().as_usize();
        self.responses[self.read_pointer]
    }

    fn size(&self) -> HDARingBufferSize {
        self.size
    }
}

impl Index<usize> for RIRB {
    type Output = HDANodeResponse;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.responses[idx]
    }
}

/// A 16-bit sample container as specified in the HDA spec
#[repr(align(2))]
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