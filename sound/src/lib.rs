#![no_std]
#![feature(array_windows)]
#![allow(unaligned_references, dead_code)]

use core::ops::{Index, DerefMut};
use machine::port::{Port, PortReadWrite};
use machine::interrupts::IRQ;
use num::{Integer, BitState};
use collections::vec;
use collections::vec::Vec;
use event_hook::{EventKind, box_fn, HandlerId, BoxedFn};

mod wav;
pub mod macros;
pub use wav::WavFile;
mod printer;
mod font;

static mut SOUND_DEVICE: Option<SoundDevice> = None;

unsafe impl Sync for SoundDevice {}

pub fn init() -> Result<(), &'static str> {
    if unsafe { SOUND_DEVICE.is_none() } {
        let sound_device = find_sound_device().ok_or("Couldn't find the sound device")?;
        // The SOUND_DEVICE static must be initialized before starting
        // to prevent registers in the sound controller from getting
        // temporary stack addresses written to them
        unsafe { SOUND_DEVICE = Some(sound_device) };
        let sound_device = unsafe { SOUND_DEVICE.as_mut().unwrap() };
        sound_device.start()?;
    }
    Ok(())
}

pub fn play_sound(sound: &Sound, action_on_end: ActionOnEnd) {
    let sd = get_sound_device().unwrap();
    sd.play_sound(*sound, action_on_end);
}

pub fn stop_sound() -> Result<(), ()> {
    let sd = get_sound_device().unwrap();
    sd.stop_sound()
}

fn get_sound_device() -> Option<&'static mut SoundDevice> {
    unsafe { SOUND_DEVICE.as_mut() }
}

/// Searches all buses on the PCI until it finds the HDA
///
/// # References
///
/// * https://wiki.osdev.org/PCI
/// * https://wiki.osdev.org/Intel_High_Definition_Audio#Identifying_HDA_on_a_machine
fn find_sound_device() -> Option<SoundDevice> {
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

type SampleDerefMut = &'static mut dyn DerefMut<Target=[Sample]>;

#[derive(Clone, Copy)]
pub struct Sound {
    file: WavFile,
    sample_buffer: &'static [Sample]
}

impl Sound {
    fn sample_len(&self) -> usize {
        self.sample_buffer.len()
    }

    fn sample_buffer_ptr(&self) -> *const Sample {
        self.sample_buffer.as_ptr()
    }
}

impl Sound {
    pub fn new(file: WavFile, sample_buffer: SampleDerefMut) -> Self {
        let sample_bytes = file.data_bytes();
        let sample_bytes_len = sample_bytes.len();
        let sample_ptr = sample_bytes.as_ptr();
        for i in 0..sample_bytes_len {
            unsafe { sample_buffer[i] = Sample(sample_ptr.offset(i.as_isize()).read())
            };
        }
        Self {
            file,
            sample_buffer: sample_buffer
        }
    }
}

type StreamTag = usize;

/// An output stream that represents a connection
/// between sound sample buffers and the HDA sound controller
///
/// This stream assumes that the sound samples in the wav file
/// have a sample rate of 44.1kHz, 16 bits per sample and 2 channels
struct OutputStream {
    regs: &'static mut StreamDescriptorRegs,
    bdl: BufferDescriptorList,
    /// A number in the range 1..=15 that is used to identify
    /// a stream by the controller
    tag: StreamTag
}

impl OutputStream {
    fn new(regs: &'static mut StreamDescriptorRegs, tag: StreamTag) -> Self {
        assert!(tag < 16);
        Self {
            regs,
            tag,
            bdl: BufferDescriptorList::new()
        }
    }

    // A seperate init function is needed because the controller
    // has to be setup before writing to registers
    fn init(&mut self) {
        self.regs.format.set_sample_base_rate(SampleBaseRate::KHz44P1);
        self.regs.format.set_sample_base_rate_multiple(SampleBaseRateMultiple::KHz48OrLess);
        self.regs.format.set_sample_base_rate_divisor(SampleBaseRateDivisor::One);
        self.regs.format.set_bits_per_sample(BitsPerSample::Sixteen);
        self.regs.format.set_number_of_channels(NumOfChannels::Two);
        self.regs.last_valid_index.set_last_valid_index(1);
        self.regs.control.set_stream_number(self.tag.as_u8());
        self.regs.control.set_interrupt_on_completion_enable(true);
        self.regs.set_bdl_base_addr(&self.bdl);
    }

    fn setup_sound_stream(&mut self, sound: Sound) {
        let bdl_entry = BufferDescriptorListEntry {
            addr: sound.sample_buffer_ptr(),
            len: sound.sample_len().as_u32(),
            interrupt_on_completion: InterruptOnCompletion::new()
        };
        // BDL should be empty before starting a stream to make sure no
        // other stream is currently running
        assert!(self.bdl.next_index == 0);
        // The HDA spec dictates that there must be at least 2 entries
        // in the BDL
        self.bdl.add_entry(bdl_entry).unwrap();
        self.bdl.add_entry(bdl_entry).unwrap();
        self.regs.cyclic_buffer_len.set_cyclic_buffer_len(self.bdl.data_bytes_len());
    }

    fn stop(&mut self) {
        self.regs.control.set_stream_run(false);
        // The HDA spec doesn't say anything about waiting here
        // but is seems necessary on my computer
        while self.regs.control.stream_run() == true {}
    }

    fn start(&mut self) {
        self.regs.control.set_stream_run(true);
    }

    fn reset(&mut self) {
        self.regs.control.enter_stream_reset();
        let mut time = 0;
        // Waiting is necessary according to the HDA spec
        while time < 1000 && self.regs.control.stream_reset() == false { time += 1; }
        time = 0;
        self.regs.control.exit_stream_reset();
        while time < 1000 && self.regs.control.stream_reset() == true { time += 1; }
        self.bdl.clear_entries();
    }

    fn has_initialized(&self) -> bool {
        !self.regs.control.stream_reset() && self.bdl.no_of_entries() == 2
    }
}

/// Indicates the action to be taken when a stream
/// has ended
#[derive(Debug)]
pub enum ActionOnEnd {
    Stop,
    Replay,
    Action(BoxedFn<'static>)
}

/// The codec address and node id of a node in a codec
#[derive(Clone, Copy, Debug, PartialEq)]
struct NodeAddr(u8, u8);

impl NodeAddr {
    fn codec_addr(&self) -> u8 {
        self.0
    }
    fn node_id(&self) -> u8 {
        self.1
    }
    fn has_conn_list(&self, commander: &mut Commander) -> bool {
        let cmd = HDANodeCommand::get_conn_list_len(self.0, self.1);
        let resp = commander.command(cmd)
            .get_conn_list_len_resp();
        resp.is_ok()
    }
}

impl Widget for NodeAddr {
    fn addr(&self) -> Self {
        *self
    }
}

struct ConnectedNode {
    addr: NodeAddr,
    conn_list: Vec<'static, (u8, NodeAddr)>
}

impl ConnectedNode {
    fn new(node: NodeAddr) -> Self {
        Self {
            addr: node,
            conn_list: vec!(item_type => (u8, NodeAddr), capacity => 5)
        }
    }
}

impl Widget for ConnectedNode {
    fn addr(&self) -> NodeAddr {
        self.addr
    }
}

impl NodeWithConnList for ConnectedNode {
    fn conn_list(&self) -> &Vec<'static, (u8, NodeAddr)> {
        &self.conn_list
    }
}

trait Widget {
    fn addr(&self) -> NodeAddr;

    fn widget_cap(&self, commander: &mut Commander) -> HDANodeResponseAFGWidgetCap {
        let NodeAddr(codec_addr, widget_id) = self.addr();
        let afg_widget_cap_command = HDANodeCommand::afg_widget_capabilities(
            codec_addr,
            widget_id
        );
        let widget_cap_resp = commander
            .command(afg_widget_cap_command)
            .afg_widget_capabilities_resp()
            .unwrap();
        widget_cap_resp
    }

    fn widget_type(&self, commander: &mut Commander) -> HDAAFGWidgetType {
        self.widget_cap(commander).widget_type()
    }
}

trait NodeWithConnList: Widget {
    fn conn_list(&self) -> &Vec<'static, (u8, NodeAddr)>;

    fn conn_list_contains(&self, node_addr: NodeAddr) -> bool {
        for (_, node) in self.conn_list().iter() {
            if *node == node_addr {
                return true;
            }
        }
        false
    }

    fn conn_list_idx(&self, node_addr: NodeAddr) -> Option<u8> {
        for (idx, node) in self.conn_list().iter() {
            if *node == node_addr {
                return Some(*idx);
            }
        }
        return None;
    }

    fn get_active_input(&self, commander: &mut Commander) -> GetConnSelCtrlResp {
        let cmd = HDANodeCommand::get_conn_sel_ctrl(self.addr());
        let resp = commander.command(cmd)
            .get_conn_sel_ctrl_resp()
            .unwrap();
        resp
    }

    fn set_active_input(&mut self, idx: u8, commander: &mut Commander) {
        let cmd = HDANodeCommand::set_conn_sel_ctrl(self.addr(), idx);
        commander.command(cmd);
    }
}

struct RootNode(u8);

impl RootNode {
    fn new(codec_addr: u8) -> Self {
        RootNode(codec_addr)
    }

    fn func_group_nodes(&self, commander: &mut Commander) -> FuncGroupIter {
        let get_node_count_command = HDANodeCommand::get_node_count(self.0, 0);
        let node_count_resp = commander.command(get_node_count_command)
            .node_count_resp()
            .unwrap();
        let start_node_id = node_count_resp.start_node_number();
        FuncGroupIter {
            start_node: NodeAddr(self.0, start_node_id),
            num_of_nodes: node_count_resp.number_of_nodes(),
            index: 0
        }
    }
}

struct FuncGroupIter {
    start_node: NodeAddr,
    num_of_nodes: u8,
    index: u8
}

impl Iterator for FuncGroupIter {
    type Item = FuncGroup;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.num_of_nodes {
            None
        } else {
            let node = NodeAddr(
                self.start_node.codec_addr(),
                self.start_node.node_id() + self.index
            );
            self.index += 1;
            Some(FuncGroup { addr: node })
        }
    }
}

struct FuncGroup {
    addr: NodeAddr
}

impl FuncGroup {
    fn grp_type(&self, commander: &mut Commander) -> HDANodeFunctionGroupType {
        let func_group_type_command = HDANodeCommand::function_group_type(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        let func_group_type_resp = commander.command(func_group_type_command)
            .func_group_type_resp()
            .unwrap();
        func_group_type_resp.node_type()
    }

    fn afg_cap(&self, commander: &mut Commander) -> AFGCapResp {
        let afg_cap_command = HDANodeCommand::afg_capabilities(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        let afg_cap_resp = commander.command(afg_cap_command)
            .afg_cap_resp()
            .unwrap();
        afg_cap_resp
    }

    fn has_beep_gen(&self, commander: &mut Commander) -> bool {
        self.afg_cap(commander).has_beep_gen()
    }

    fn nodes(&self, commander: &mut Commander) -> NodeIter {
        let get_node_count_command = HDANodeCommand::get_node_count(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        let node_count_resp = commander.command(get_node_count_command)
            .node_count_resp()
            .unwrap();
        NodeIter {
            start_node: NodeAddr(self.addr.codec_addr(), node_count_resp.start_node_number()),
            num_of_nodes: node_count_resp.number_of_nodes(),
            index: 0
        }
    }
}

struct NodeIter {
    start_node: NodeAddr,
    num_of_nodes: u8,
    index: u8
}

impl Iterator for NodeIter {
    type Item = NodeAddr;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.num_of_nodes {
            None
        } else {
            let node = NodeAddr(
                self.start_node.codec_addr(),
                self.start_node.node_id() + self.index
            );
            self.index += 1;
            Some(node)
        }
    }
}

#[derive(Clone, Debug)]
struct Pin {
    addr: NodeAddr,
    conn_list: Vec<'static, (u8, NodeAddr)>
}

impl Pin {
    fn new(codec_addr: u8, node_id: u8) -> Self {
        Self {
            addr: NodeAddr(codec_addr, node_id),
            conn_list: vec!(item_type => (u8, NodeAddr), capacity => 5)
        }
    }

    fn pin_cap(&self, commander: &mut Commander) -> HDANodeResponsePinCapabilities {
        let pin_cap_command = HDANodeCommand::get_pin_capabilities(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        let pin_cap = commander.command(pin_cap_command)
            .pin_capabilities_resp()
            .unwrap();
        pin_cap
    }

    fn config_defaults(&self, commander: &mut Commander) -> HDANodeResponsePinConfigDefaults {
        let pin_config_default_command = HDANodeCommand::get_pin_config_defaults(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        let config_defaults = commander.command(pin_config_default_command)
            .get_pin_config_defaults_resp()
            .unwrap();
        config_defaults
    }

    fn enable(&mut self, commander: &mut Commander) {
        let pin_ctrl = PinControl::new()
            .input_enabled(true)
            .output_enabled(true);
        let pin_widget_ctrl_command = HDANodeCommand::set_pin_widget_control(
            self.addr.codec_addr(),
            self.addr.node_id(),
            pin_ctrl
        );
        commander.command(pin_widget_ctrl_command);
    }

    fn eapd_enable(&self, commander: &mut Commander) -> EAPDEnable {
        let cmd = HDANodeCommand::eapd_enable(self.addr);
        commander.command(cmd)
            .eapd_enable_resp()
            .unwrap()
    }

    // Not working either
    fn enable_eapd(&mut self, commander: &mut Commander) {
        let eapd_enable = EAPDEnableBuilder::new()
            .eapd(true)
            .value();
        let cmd = HDANodeCommand::set_eapd_enable(self.addr, eapd_enable.into());
        commander.command(cmd);
    }

    fn num_of_inputs(&self, commander: &mut Commander) -> u8 {
        let conn_list_len_command = HDANodeCommand::get_conn_list_len(self.addr.codec_addr(), self.addr.node_id());
        let resp = commander.command(conn_list_len_command)
            .get_conn_list_len_resp()
            .unwrap();
        resp.conn_list_len()
    }

    fn power_ctrl_supported(&self, commander: &mut Commander) -> bool {
        let afg_widget_cap = HDANodeCommand::afg_widget_capabilities(self.addr.codec_addr(), self.addr.node_id());
        let resp = commander.command(afg_widget_cap)
            .afg_widget_capabilities_resp()
            .unwrap();
        resp.power_ctrl_supported()
    }

    fn power_up(&self, commander: &mut Commander) {
        let set_power_command = HDANodeCommand::set_power_state(
            self.addr.codec_addr(),
            self.addr.node_id(),
            PowerState::D0
        );
        commander.command(set_power_command);
    }

    fn unmute(&mut self, commander: &mut Commander) {
        // Unmute DAC amplifier
        let amp_gain = AmpGain::new()
            .mute(false)
            .output_amp(true)
            .left_amp(true)
            .right_amp(true)
            .index(0)
            .gain(0x7f);
        let set_amp_gain_command = HDANodeCommand::set_amp_gain(
            self.addr.codec_addr(),
            self.addr.node_id(),
            amp_gain
        );
        commander.command(set_amp_gain_command);
    }
}

impl NodeWithConnList for Pin {
    fn conn_list(&self) -> &Vec<'static, (u8, NodeAddr)> {
        &self.conn_list
    }
}

impl Widget for Pin {
    fn addr(&self) -> NodeAddr {
        self.addr
    }
}

impl From<NodeAddr> for Pin {
    fn from(addr: NodeAddr) -> Self {
        Self::new(addr.codec_addr(), addr.node_id())
    }
}

#[derive(Clone, Copy, Debug)]
struct DAC {
    addr: NodeAddr
}

impl DAC {
    fn new(addr: NodeAddr) -> Self {
        Self { addr }
    }

    fn setup_stream_and_channel(&mut self, commander: &mut Commander, stream: u8, channel: u8) {
        // Set up the DAC to receive input from the stream
        let converter_ctrl = ConverterControl::new()
            .stream(stream)
            .channel(channel);
        let converter_control_command = HDANodeCommand::set_converter_control(
            self.addr.codec_addr(),
            self.addr.node_id(),
            converter_ctrl
        );
        commander.command(converter_control_command);
    }

    fn output_amp_cap(&self, commander: &mut Commander) -> HDANodeResponseAmpCapabilities {
        let get_amp_cap_command = HDANodeCommand::get_out_amp_capabilties(
            self.addr.codec_addr(),
            self.addr.node_id()
        );
        commander.command(get_amp_cap_command)
            .amp_capabilities_resp()
            .unwrap()
    }

    fn unmute(&mut self, commander: &mut Commander) {
        //let amp_cap = self.output_amp_cap(commander);
        // Unmute DAC amplifier
        let amp_gain = AmpGain::new()
            .mute(false)
            .output_amp(true)
            .left_amp(true)
            .right_amp(true)
            .index(0)
            .gain(0x7f);
        let set_amp_gain_command = HDANodeCommand::set_amp_gain(
            self.addr.codec_addr(),
            self.addr.node_id(),
            amp_gain
        );
        commander.command(set_amp_gain_command);
    }

    fn power_up(&self, commander: &mut Commander) {
        let set_power_command = HDANodeCommand::set_power_state(
            self.addr.codec_addr(),
            self.addr.node_id(),
            PowerState::D0
        );
        commander.command(set_power_command);
    }

    fn set_converter_format(&mut self, format: u16, commander: &mut Commander) {
        let set_format = HDANodeCommand::set_converter_format(self.addr, format);
        commander.command(set_format);
    }

    fn digital_ctrl(&self, commander: &mut Commander) -> DigitalConverterControl {
        let digi_ctrl_command = HDANodeCommand::get_digital_converter_ctrl(self.addr);
        commander.command(digi_ctrl_command)
            .digital_converter_ctrl_resp()
            .unwrap()
    }

    // For some currently unknown reason, this function doesn't
    // actually change anything
    fn set_digital_ctrl(&mut self, digi_ctrl: DigitalConverterControl, commander: &mut Commander) {
        let set_digi_ctrl_cmds = HDANodeCommand::set_digital_converter_ctrl(self.addr, digi_ctrl.into());
        for cmd in set_digi_ctrl_cmds {
            commander.command(cmd);
        }
    }
}

impl PartialEq<NodeAddr> for DAC {
    fn eq(&self, rhs: &NodeAddr) -> bool {
        self.addr == *rhs
    }
}

#[derive(Clone, Debug)]
struct Mixer {
    addr: NodeAddr,
    conn_list: Vec<'static, (u8, NodeAddr)>
}

impl Mixer {
    fn new(codec_addr: u8, node_id: u8) -> Self {
        Self {
            addr: NodeAddr(codec_addr, node_id),
            conn_list: vec!(item_type => (u8, NodeAddr), capacity => 5)
        }
    }

    fn num_of_inputs(&self, commander: &mut Commander) -> u8 {
        let conn_list_len_command = HDANodeCommand::get_conn_list_len(self.addr.codec_addr(), self.addr.node_id());
        let resp = commander.command(conn_list_len_command)
            .get_conn_list_len_resp()
            .unwrap();
        resp.conn_list_len()
    }

    fn power_ctrl_supported(&self, commander: &mut Commander) -> bool {
        let afg_widget_cap = HDANodeCommand::afg_widget_capabilities(self.addr.codec_addr(), self.addr.node_id());
        let resp = commander.command(afg_widget_cap)
            .afg_widget_capabilities_resp()
            .unwrap();
        resp.power_ctrl_supported()
    }    
}

impl NodeWithConnList for Mixer {
    fn conn_list(&self) -> &Vec<'static, (u8, NodeAddr)> {
        &self.conn_list
    }
}

impl Widget for Mixer {
    fn addr(&self) -> NodeAddr {
        self.addr
    }
}

impl From<NodeAddr> for Mixer {
    fn from(addr: NodeAddr) -> Self {
        Self::new(addr.codec_addr(), addr.node_id())
    }
}

#[derive(Clone, Debug)]
struct Selector {
    addr: NodeAddr,
    conn_list: Vec<'static, NodeAddr>
}

impl Selector {
    fn new(codec_addr: u8, node_id: u8) -> Self {
        Self {
            addr: NodeAddr(codec_addr, node_id),
            conn_list: vec!(item_type => NodeAddr, capacity => 5)
        }
    }
}

impl Widget for Selector {
    fn addr(&self) -> NodeAddr {
        self.addr
    }
}

/// A device on the PCI bus
///
/// It is assumed that the device has a PCI configuration header of type 0x0
///
/// # References
///
/// * The OSDev wiki <https://wiki.osdev.org/PCI>
#[derive(Clone, Copy)]
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

    fn interrupt_pin(&self) -> u8 {
        assert_eq!(self.header_type(), PCIHeaderType::Standard);
        let (mut addr_port, data_port) = self.ports();
        let reg_addr = self.reg_addr(Self::INTERRUPT_PIN_LINE_OFFSET);
        addr_port.write(reg_addr);
        (data_port.read() >> 8) as u8
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
        let mut val = data_port.read();
        val.set_bits(8..16, line.as_u8().as_u32());
        data_port.write(val);
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
    /// The sound device's PCI interface
    pci_config: PCIDevice,
    /// The pins attached to speakers that can be used to play sound
    ///
    /// This vector will be populated after starting the device
    output_pins: Vec<'static, Pin>,
    /// The DACs connected to output pins that can be used
    /// to set up a sound stream with the controller
    output_converters: Vec<'static, DAC>,
    /// The mixers for playing more than 1 stream at a time
    mixers: Vec<'static, Mixer>,
    /// The addresses of valid codecs in the controller
    codec_addrs: Vec<'static, u8>,
    /// Communicates with the controller with the CORB and RIRB
    commander: Commander,
    /// A connection with a DAC through which sound samples
    /// are channeled
    output_stream: OutputStream,
    /// A node that can generate beeps with the HDA beep commands
    beep_gen: Option<NodeAddr>,
    /// The sound id of the sound that is currently playing in the
    /// output streams
    ///
    /// This corresponds to the handler id of the action_on_end event hook
    /// that will be executed when the current sound stream ends
    currently_playing_sound_id: Option<HandlerId>,
    //active_dac_index: Option<usize>
}

impl SoundDevice {
    // Register offsets
    const CONTROLLER_REGS_OFFSET: isize = 0x00;
    const INTERRUPT_REGS_OFFSET: isize = 0x20;
    const CORB_REGS_OFFSET: isize = 0x40;
    const RIRB_REGS_OFFSET: isize = 0x50;
    const IMMEDIATE_COMMAND_OUTPUT_OFFSET: isize = 0x60;
    const IMMEDIATE_RESPONSE_INPUT_OFFSET: isize = 0x64;

    fn new(pci_config: PCIDevice) -> Self {
        Self {
            pci_config,
            output_pins: vec!(item_type => Pin, capacity => 10),
            output_converters: vec!(item_type => DAC, capacity => 10),
            mixers: vec!(item_type => Mixer, capacity => 10),
            codec_addrs: vec!(item_type => u8, capacity => 15),
            commander: Commander::new(Self::corb_regs_mut_base(pci_config), Self::rirb_regs_mut_base(pci_config)),
            output_stream: OutputStream::new(Self::stream_descriptor_regs_mut_base(pci_config, 0).unwrap(), 1),
            currently_playing_sound_id: None,
            beep_gen: None
        }
    }
    
    /// Plays a sound
    ///
    /// The returned SoundId is used to identify the sound to stop
    fn play_sound(&mut self, sound: Sound, action_on_end: ActionOnEnd) {
        if self.currently_playing_sound_id.is_some() {
            self.stop_sound().unwrap();
        }
        let output_stream = &mut self.output_stream;
        // For some reason, this init function has to be called
        // again before playing a new stream
        output_stream.init();
        output_stream.setup_sound_stream(sound);
        let action_on_end_hook_id = match action_on_end {
            ActionOnEnd::Stop => event_hook::hook_event(EventKind::Sound, box_fn!(move |_| {
                stop_sound().unwrap();
            })),
            ActionOnEnd::Replay => event_hook::hook_event(EventKind::Sound, box_fn!(move |_| {
                let sd = get_sound_device().unwrap();
                sd.output_stream.stop();
                sd.output_stream.reset();
                sd.output_stream.init();
                sd.output_stream.setup_sound_stream(sound);
                sd.output_stream.start();
            })),
            ActionOnEnd::Action(func) => event_hook::hook_event(EventKind::Sound, func)
        };
        self.currently_playing_sound_id = Some(action_on_end_hook_id);
        output_stream.start();
    }

    fn stop_sound(&mut self) -> Result<(), ()> {
        if let Some(id) = self.currently_playing_sound_id.take() {
            self.output_stream.stop();
            self.output_stream.reset();
            event_hook::unhook_event(id, EventKind::Sound);
            Ok(())
        } else {
            Err(())
        }
    }

    fn set_beep_gen(&mut self, beep_node: NodeAddr) {
        self.beep_gen = Some(beep_node);
    }

    fn start(&mut self) -> Result<(), &'static str> {
        let controller_regs = self.controller_regs_mut();
        // Asserting the bit removes the controller from reset state
        controller_regs.control.set_controller_reset(true);
        while !controller_regs.control.controller_reset() {}
        // After reset de-assertion, 521 us should be waited
        let mut timeout = 0;
        while timeout < 1_000_000 { timeout += 1; }
        // Waiting for the codecs to initialize
        while controller_regs.state_change_status.sdin_state_change_status() == 0 {}

        // After starting the device the addresses of the codecs
        // are the set bit positions in the state change status register
        let sdin_state_change_stat = self
            .controller_regs()
            .state_change_status
            .sdin_state_change_status();
        (0..16u8)
            .for_each(|i| if sdin_state_change_stat.get_bit(i.into()) == BitState::Set {
                self.codec_addrs.push(i);
            });
        
        let interrupt_regs = self.interrupt_regs_mut();

        // Enable interrupts from the controller
        interrupt_regs.control.set_global_interrupt_enable(true);

        // Enable interrupts from output streams
        let num_of_input_streams = controller_regs.capabilities.num_of_input_streams();
        let num_of_output_streams = controller_regs.capabilities.num_of_output_streams();
        if num_of_output_streams < 2 {
            return Err("No enough output streams for sound operation");
        }
        for stream_idx in 0..num_of_output_streams {
            // The output streams bits in the interrupt control reg
            // start after the input streams
            interrupt_regs.control.set_stream_interrupt_enable(num_of_input_streams + stream_idx);
        }

        // Enable all possible streams to run in stream sync
        interrupt_regs.stream_sync.unblock_all_streams();

        self.pci_config.set_interrupt_line(IRQ::Sound);

        // The commander must be initialized first
        self.commander.init();
        // Widgets must be discovered before preparing to play sound
        self.discover_widgets();
        // Output stream must be initialized before preparing to play sound
        self.output_stream.init();
        self.prepare_to_play_sound()?;
        Ok(())
    }

    fn prepare_to_play_sound(&mut self) -> Result<(), &'static str> {
        if self.output_pins.len() < 1 {
            return Err("No enough output pins to play sound");
        }
        if self.output_converters.len() < 1 {
            return Err("No enough output converters to play sound");
        }
        let pin = &mut self.output_pins[0];
        let mut dac: Option<DAC> = None;
        for (_, dac_) in self.output_converters.iter().enumerate() {
            if pin.conn_list_contains(dac_.addr) {
                dac = Some(*dac_);
                break;
            }
        }

        let mut dac = dac.ok_or("No output suitable DAC was found in the output pin connection list")?;

        dac.power_up(&mut self.commander);
        dac.set_converter_format(self.output_stream.regs.format.reg_value(), &mut self.commander);
        dac.setup_stream_and_channel(&mut self.commander, self.output_stream.tag.as_u8(), 0);

        dac.unmute(&mut self.commander);

        pin.enable_eapd(&mut self.commander);
        pin.enable(&mut self.commander);
        pin.unmute(&mut self.commander);
        if pin.power_ctrl_supported(&mut self.commander) {
            pin.power_up(&mut self.commander);
        }

        Ok(())
    }

    fn discover_widgets(&mut self) {
        for i in 0..self.codec_addrs.len() {
            let codec_addr = self.codec_addrs[i];
            let root_node = RootNode::new(codec_addr);
            for func_group in root_node.func_group_nodes(&mut self.commander) {
                if func_group.grp_type(&mut self.commander) != HDANodeFunctionGroupType::AFG {
                    continue;
                }
                if func_group.has_beep_gen(&mut self.commander) {
                    self.set_beep_gen(NodeAddr(codec_addr, func_group.addr.node_id()));
                }
                for node in func_group.nodes(&mut self.commander) {
                    match node.widget_type(&mut self.commander) {
                        HDAAFGWidgetType::AudioOutput => {
                            self.output_converters.push(DAC::new(node));
                        }
                        HDAAFGWidgetType::AudioMixer => {
                            let mut mixer = Mixer::new(codec_addr, node.addr().node_id());
                            build_conn_list(mixer.addr, &mut mixer.conn_list, &mut self.commander).unwrap();
                            self.mixers.push(mixer);
                        }
                        HDAAFGWidgetType::PinComplex => {
                            let mut pin = Pin::new(codec_addr, node.addr().node_id());
                            let pin_cap = pin.pin_cap(&mut self.commander);
                            if !pin_cap.output_capable() { continue; }
                            let config_defaults = pin.config_defaults(&mut self.commander);
                            if !(config_defaults.port_connectivity() != PortConnectivity::None
                                && config_defaults.default_device() == DefaultDevice::Speaker) {
                                    continue;
                                }
                            build_conn_list(pin.addr, &mut pin.conn_list, &mut self.commander).unwrap();
                            self.output_pins.push(pin);
                        },
                        _ => ()
                    };
                }
            }
        }
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

    fn reg_ptr_base(pci_config: PCIDevice, offset: isize) -> *mut u8 {
        let base_ptr = pci_config.bar0().addr() as *mut u8;
        unsafe { base_ptr.offset(offset) }
    }

    fn controller_regs(&self) -> &'static ControllerRegs {
        Self::controller_regs_base(self.pci_config)
    }

    fn controller_regs_base(pci_config: PCIDevice) -> &'static ControllerRegs {
        let ptr = Self::reg_ptr_base(pci_config, Self::CONTROLLER_REGS_OFFSET).cast::<ControllerRegs>();
        unsafe { &*ptr }
    }

    fn controller_regs_mut(&self) -> &'static mut ControllerRegs {
        let ptr = self.reg_ptr(Self::CONTROLLER_REGS_OFFSET).cast::<ControllerRegs>();
        unsafe { &mut *ptr }
    }

    fn interrupt_regs(&self) -> &'static InterruptRegs {
        let ptr = self.reg_ptr(Self::INTERRUPT_REGS_OFFSET).cast::<InterruptRegs>();
        unsafe { &*ptr }
    }

    fn interrupt_regs_mut(&self) -> &'static mut InterruptRegs {
        let ptr = self.reg_ptr(Self::INTERRUPT_REGS_OFFSET).cast::<InterruptRegs>();
        unsafe { &mut *ptr }
    }

    fn corb_regs(&self) -> &'static CORBRegs {
        let ptr = self.reg_ptr(Self::CORB_REGS_OFFSET).cast::<CORBRegs>();
        unsafe { &*ptr }
    }

    fn corb_regs_mut(&self) -> &'static mut CORBRegs {
        Self::corb_regs_mut_base(self.pci_config)
    }

    fn corb_regs_mut_base(pci_config: PCIDevice) -> &'static mut CORBRegs {
        let ptr = Self::reg_ptr_base(pci_config, Self::CORB_REGS_OFFSET).cast::<CORBRegs>();
        unsafe { &mut *ptr }
    }

    fn rirb_regs(&self) -> &'static RIRBRegs {
        let ptr = self.reg_ptr(Self::RIRB_REGS_OFFSET).cast::<RIRBRegs>();
        unsafe { &*ptr }
    }

    fn rirb_regs_mut(&self) -> &'static mut RIRBRegs {
        Self::rirb_regs_mut_base(self.pci_config)
    }

    fn rirb_regs_mut_base(pci_config: PCIDevice) -> &'static mut RIRBRegs {
        let ptr = Self::reg_ptr_base(pci_config, Self::RIRB_REGS_OFFSET).cast::<RIRBRegs>();
        unsafe { &mut *ptr }
    }

    /// The offset of the output stream descriptor register n
    ///
    /// Returns None when the output stream descriptor n does not exist
    fn output_stream_descriptor_offset(&self, n: u8) -> Option<isize> {
        Self::output_stream_descriptor_offset_base(self.pci_config, n)
    }

    fn output_stream_descriptor_offset_base(pci_config: PCIDevice, n: u8) -> Option<isize> {
        let controller_regs = Self::controller_regs_base(pci_config);
        if n > 15 {
            None
        } else if n > controller_regs.capabilities.num_of_output_streams() {
            None
        } else {
            // Calculations as described in the HDA spec
            let x = 0x80 + (controller_regs.capabilities.num_of_input_streams().as_isize() * 0x20);
            Some(x + n.as_isize() * 0x20)
        }
    }
    
    /// Returns the pointer to stream descriptor registers at offset n
    fn stream_descriptor_regs_ptr(&self, n: u8) -> Option<*mut StreamDescriptorRegs> {
        Self::stream_descriptor_regs_ptr_base(self.pci_config, n)
    }

    fn stream_descriptor_regs_ptr_base(pci_config: PCIDevice, n: u8) -> Option<*mut StreamDescriptorRegs> {
        let offset = Self::output_stream_descriptor_offset_base(pci_config, n);
        if offset.is_none() {
            return None;
        }
        let ptr = Self::reg_ptr_base(pci_config, offset.unwrap()).cast::<StreamDescriptorRegs>();
        Some(ptr)
    }

    fn stream_descriptor_regs(&self, n: u8) -> Option<&'static StreamDescriptorRegs> {
        let ptr = self.stream_descriptor_regs_ptr(n);
        if ptr.is_none() { return None; }
        let ptr = ptr.unwrap();
        Some(unsafe { &*ptr })
    }

    fn stream_descriptor_regs_mut(&self, n: u8) -> Option<&'static mut StreamDescriptorRegs> {
        Self::stream_descriptor_regs_mut_base(self.pci_config, n)
    }

    fn stream_descriptor_regs_mut_base(pci_config: PCIDevice, n: u8) -> Option<&'static mut StreamDescriptorRegs> {
        let ptr = Self::stream_descriptor_regs_ptr_base(pci_config, n);
        if ptr.is_none() { return None; }
        let ptr = ptr.unwrap();
        Some(unsafe { &mut *ptr })
    }
}

fn build_conn_list(node: NodeAddr, conn_list: &mut Vec<(u8, NodeAddr)>, commander: &mut Commander) -> Result<(), ()> {
    let get_conn_list_command = HDANodeCommand::get_conn_list_len(node.codec_addr(), node.node_id());
    let conn_list_len_resp = commander.command(get_conn_list_command)
        .get_conn_list_len_resp();
    if conn_list_len_resp.is_err() { return Err(()); }
    let conn_list_len_resp = conn_list_len_resp.unwrap();
    
    let mut conn_list_index_iter = (0..conn_list_len_resp.conn_list_len()).step_by(4);
    let mut no_in_batch = 4;
    if conn_list_len_resp.long_form() {
        conn_list_index_iter = (0..conn_list_len_resp.conn_list_len()).step_by(2);
        no_in_batch = 2;
    }
    for conn_idx in conn_list_index_iter {
        let get_conn_list_entry_command = HDANodeCommand::get_conn_list_entry(
            node.codec_addr(),
            node.node_id(),
            conn_idx
        );
        let get_conn_list_entry_resp = commander.command(get_conn_list_entry_command)
            .get_conn_list_entry_resp(conn_list_len_resp.long_form())
            .unwrap();
        
        for (entry_idx, connected_node_id) in get_conn_list_entry_resp.entries().enumerate() {
            assert!((connected_node_id & 0xff) == connected_node_id.as_u8().as_u16());
            conn_list.push(
                (conn_idx * no_in_batch + entry_idx.as_u8(), NodeAddr(node.codec_addr(), connected_node_id.as_u8()))
            );
        }
    }
    return Ok(())
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
        pci_device.enable_memory_space_accesses();
        SoundDevice::new(pci_device)
    }
}

#[repr(packed)]
struct ControllerRegs {
    capabilities: HDAGlobalCapabilitiesReg,
    minor_version: u8,
    major_version: u8,
    output_payload_capability: u16,
    input_payload_capability: u16,
    control: HDAGlobalControlReg,
    wake_enable: HDAWakeEnableReg,
    state_change_status: HDAStateChangeStatusReg,
    status: u16,
    reserved1: [u8; 6],
    output_stream_payload_capability: u16,
    input_stream_payload_capability: u16
}

#[repr(packed)]
struct InterruptRegs {
    control: HDAInterruptControlReg,
    status: HDAInterruptStatusReg,
    reserved1: [u8; 8],
    wall_clock_counter: u32,
    reserved2: [u8; 4],
    stream_sync: StreamSyncReg
}

#[repr(packed)]
struct CORBRegs {
    corb_lower_base_addr: u32,
    corb_upper_base_addr: u32,
    corbwp: HDACORBWritePointerReg,
    corbrp: HDACORBReadPointerReg,
    control: HDACORBControlReg,
    status: HDACORBStatusReg,
    size: HDACORBSizeReg
}

impl CORBRegs {
    fn set_corb_addr(&mut self, addr: u64) {
        let lower = (addr & 0xffffffff) as u32;
        let upper = (addr >> 32) as u32;
        self.corb_lower_base_addr = lower;
        self.corb_upper_base_addr = upper;
    }
}

#[repr(packed)]
struct RIRBRegs {
    rirb_lower_base_addr: u32,
    rirb_upper_base_addr: u32,
    rirbwp: HDARIRBWritePointerReg,
    response_interrupt_count: HDAResponseInterruptCountReg,
    control: HDARIRBControlReg,
    status: HDARIRBStatusReg,
    size: HDARIRBSizeReg
}

impl RIRBRegs {
    fn set_rirb_addr(&mut self, addr: u64) {
        let lower = addr as u32;
        let upper = (addr >> 32) as u32;
        self.rirb_lower_base_addr = lower;
        self.rirb_upper_base_addr = upper;
    }
}

#[repr(packed)]
struct StreamDescriptorRegs {
    control: HDAStreamDescriptorControlReg,
    status: HDAStreamDescriptorStatusReg,
    link_pos_in_buffer: HDAStreamDescriptorLinkPosReg,
    cyclic_buffer_len: HDAStreamDescriptorCyclicBufferLenReg,
    last_valid_index: HDAStreamDescriptorLastValidIndexReg,
    reserved1: u16,
    fifo_size: HDAStreamDescriptorFIFOSizeReg,
    format: HDAStreamDescriptorFormatReg,
    reserved2: u32,
    bdl_ptr_lower_base_addr: u32,
    bdl_ptr_upper_base_addr: u32
}

impl StreamDescriptorRegs {
    fn set_bdl_base_addr(&mut self, bdl: &BufferDescriptorList) {
        let addr = &bdl.entries as *const _ as u64;
        let lower = addr.get_bits(0..32) as u32;
        let upper = addr.get_bits(32..64) as u32;
        self.bdl_ptr_lower_base_addr = lower;
        self.bdl_ptr_upper_base_addr = upper;
    }
}

/// Indicates the capabilities of the HDA controller
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
struct StreamSyncReg(u32);

impl StreamSyncReg {
    /// Sets the stream sync bit for a stream
    /// descriptor with the number n
    fn set_stream_bit(&mut self, n: u8, set: bool) {
        assert!(n < 30);
        if set {
            self.0.set_bit(n.into());
        } else {
            self.0.unset_bit(n.into());
        }
    }

    fn set_stream_sync(&mut self, n: u32) {
        // The highest 2 bits must be 0
        assert!(n < (u32::MAX << 2 >> 2));
        self.0 = n;
    }

    fn unblock_all_streams(&mut self) {
        // Clearing a bit unblocks the stream that corresponds
        // to it
        self.0 = 0;
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
        (self.0.get_bits(0..8) & 0xff).as_u8()
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


#[repr(packed)]
struct HDAStreamDescriptorControlReg {
    low: u16,
    high: u8
}

impl HDAStreamDescriptorControlReg {
    fn as_u32(&self) -> u32 {
        self.low.as_u32() | (self.high.as_u32() << 16)
    }

    fn set_u32(&mut self, n: u32) {
        self.low = (n & 0xffff).as_u16();
        self.high = ((n >> 16) & 0xff).as_u8();
    }

    /// Returns a number between 0 and 15, where 0 means unused
    /// and 1 to 15 is the tag associated with the data being
    /// transferred on the link 
    fn stream_number(&self) -> u8 {
        self.as_u32().get_bits(20..24).as_u8()
    }

    fn set_stream_number(&mut self, n: u8) {
        assert!(n > 0 && n < 16);
        let mut val = self.as_u32();
        val.set_bits(20..24, n as u32);
        self.set_u32(val);
    }

    /// Returns true if an interrupt will be generated
    /// when the Descriptor Error Status bit is set
    fn descriptor_error_interrupt_enabled(&self) -> bool {
        self.as_u32().get_bit(4) == BitState::Set
    }

    fn enable_descriptor_error_interrupt(&mut self, enable: bool) {
        let mut val = self.as_u32();
        if enable {
            val.set_bit(4);
        } else {
            val.unset_bit(4);
        }
        self.set_u32(val);
    }

    /// Returns true if an interrupt will be generated when 
    /// an FIFO error occurs (overrun for input, under run for output)
    fn fifo_interrupt_enabled(&self) -> bool {
        self.as_u32().get_bit(3) == BitState::Set
    }

    fn enable_fifo_interrupt(&mut self, enable: bool) {
        let mut val = self.as_u32();
        if enable {
            val.set_bit(3);
        } else {
            val.unset_bit(3);
        }
        self.set_u32(val);
    }

    /// Returns true if an interrupt will be generated when
    /// a buffer completes with the InterruptOnCompletion bit
    /// set in its descriptor
    fn interrupt_on_completion_enabled(&self) -> bool {
        self.as_u32().get_bit(2) == BitState::Set
    }

    fn set_interrupt_on_completion_enable(&mut self, enable: bool) {
        let mut val = self.as_u32();
        if enable {
            val.set_bit(2);
        } else {
            val.unset_bit(2);
        }
        self.set_u32(val);
    }

    /// Returns true if the DMA engine associated with this
    /// input stream is enabled to transfer data in the FIFO
    /// to main memory
    fn stream_run(&self) -> bool {
        self.as_u32().get_bit(1) == BitState::Set
    }

    // When set to false, the DMA engine associated with this
    // stream is disabled
    fn set_stream_run(&mut self, run: bool) {
        let mut val = self.as_u32();
        if run {
            val.set_bit(1);
        } else {
            val.unset_bit(1);
        }
        self.set_u32(val);
    }

    /// Tells whether or not the stream is in a reset state
    fn stream_reset(&self) -> bool {
        self.as_u32().get_bit(0) == BitState::Set
    }

    /// Places the stream in a reset state
    ///
    /// After resetting the stream, a true
    /// must be returned from `stream_reset` to verify that
    /// the stream is in reset
    fn enter_stream_reset(&mut self) {
        let mut val = self.as_u32();
        val.set_bit(0);
        self.set_u32(val);
    }

    /// Removes the stream from its reset state
    ///
    /// After exiting reset, a false must be returned from
    /// `stream_reset` to verify that the stream is ready to begin
    /// operation
    fn exit_stream_reset(&mut self) {
        let mut val = self.as_u32();
        val.unset_bit(0);
        self.set_u32(val);
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
    fn set_cyclic_buffer_len(&mut self, len: usize) {
        assert!(len <= u32::MAX.as_usize());
        self.0 = len.as_u32();
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

    fn reg_value(&self) -> u16 {
        self.0
    }
}

impl From<u16> for HDAStreamDescriptorFormatReg {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

#[derive(Debug)]
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

#[derive(Clone, Copy, Debug)]
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
    /// to true
    fn new() -> Self {
        Self(0b1)
    }
}

/// A description of a SampleBuffer which is a piece of
/// the whole cyclic stream buffer
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct BufferDescriptorListEntry {
    /// The starting address of the sample buffer, which
    /// must be 128 byte aligned
    addr: *const Sample,
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
    fn new(addr: *const Sample, len: usize) -> Self {
        assert!(len <= u32::MAX.as_usize());
        Self {
            addr,
            len: len.as_u32(),
            interrupt_on_completion: InterruptOnCompletion::new()
        }
    }

    fn null() -> Self {
        Self {
            addr: core::ptr::null_mut() as *mut Sample,
            len: 0,
            interrupt_on_completion: InterruptOnCompletion::new()
        }
    }

    fn len(&self) -> usize {
        self.len.as_usize()
    }
}

/// A structure which describes all the buffers in memory
/// which makes up the virtual cyclic buffer 
#[repr(C, align(128))]
struct BufferDescriptorList {
    // 256 is the max allowed number of entries
    entries: [BufferDescriptorListEntry; 256],
    next_index: usize
}

impl BufferDescriptorList {
    /// Creates a new BufferDescriptorList
    ///
    /// The HDA spec dictates that there must be at least 2
    /// entries in the list
    fn new() -> Self {
        let list = [BufferDescriptorListEntry::null(); 256];
        Self {
            entries: list,
            next_index: 0
        }
    }

    fn add_entry(&mut self, entry: BufferDescriptorListEntry) -> Result<(), ()> {
        if self.next_index >= 256 {
            return Err(());
        }
        self.entries[self.next_index] = entry;
        self.next_index += 1;
        Ok(())
    }

    fn data_bytes_len(&self) -> usize {
        (0..self.next_index)
            .fold(0, |acc, i| acc + self.entries[i].len())
    }

    fn clear_entries(&mut self) {
        // No need to actually remove the entries
        self.next_index = 0;
    }

    fn no_of_entries(&self) -> usize {
        self.next_index
    }
}

impl Index<usize> for BufferDescriptorList {
    type Output = BufferDescriptorListEntry;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.entries[idx]
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
    const SET_PIN_WIDGET_CONTROL: u32 = 0x707;
    const SET_CONVERTER_CONTROL: u32 = 0x706;
    const SET_POWER_STATE: u32 = 0x705;
    const SET_AMP_GAIN: u32 = 0x3;
    const SET_CONN_SELECT_CTRL: u32 = 0x701;
    const SET_BEEP_GEN: u32 = 0x70a;
    const SET_CONVERTER_FORMAT: u32 = 0x02;
    const GET_CONVERTER_CONTROL: u32 = 0xf0d;
    const SET_CONVERTER_CONTROL1: u32 = 0x70d;
    const SET_CONVERTER_CONTROL2: u32 = 0x70e;
    const SET_CONVERTER_CONTROL3: u32 = 0x73e;
    const SET_CONVERTER_CONTROL4: u32 = 0x73f;
    const SET_EAPD_ENABLE: u32 = 0x70c;
    const GET_EAPD_ENABLE: u32 = 0xf0c;
    const GET_CONN_SEL_CTRL: u32 = 0xf01;
    const SET_CONN_SEL_CTRL: u32 = 0x701;
    
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

    fn set_pin_widget_control(pin_ctrl: PinControl) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, pin_ctrl.into());
        val.set_bits(8..20, Self::SET_PIN_WIDGET_CONTROL);
        Self(val)
    }

    fn set_converter_control(ctrl: ConverterControl) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, ctrl.into());
        val.set_bits(8..20, Self::SET_CONVERTER_CONTROL);
        Self(val)
    }

    fn set_power_state(state: PowerState) -> Self {
        let mut val = 0u32;
        val.set_bits(0..4, state.into());
        val.set_bits(8..20, Self::SET_POWER_STATE);
        Self(val)
    }

    fn set_amp_gain(gain: AmpGain) -> Self {
        let mut val = 0u32;
        val.set_bits(0..16, gain.into());
        val.set_bits(16..20, Self::SET_AMP_GAIN);
        Self(val)
    }

    fn set_conn_select_ctrl(conn_idx: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, conn_idx.into());
        val.set_bits(8..20, Self::SET_CONN_SELECT_CTRL);
        Self(val)
    }

    fn set_beep_gen(divider: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, divider.into());
        val.set_bits(8..20, Self::SET_BEEP_GEN);
        Self(val)
    }

    fn set_converter_format(format: u16) -> Self {
        let mut val = 0u32;
        val.set_bits(0..16, format.into());
        val.set_bits(16..20, Self::SET_CONVERTER_FORMAT);
        Self(val)
    }

    fn get_digital_converter_ctrl() -> Self {
        let mut val = 0u32;
        val.set_bits(8..20, Self::GET_CONVERTER_CONTROL);
        Self(val)
    }

    fn set_digital_converter_ctrl1(bits: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, bits.into());
        val.set_bits(8..20, Self::SET_CONVERTER_CONTROL1);
        Self(val)
    }

    fn set_digital_converter_ctrl2(bits: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, bits.into());
        val.set_bits(8..20, Self::SET_CONVERTER_CONTROL2);
        Self(val)
    }

    fn set_digital_converter_ctrl3(bits: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, bits.into());
        val.set_bits(8..20, Self::SET_CONVERTER_CONTROL3);
        Self(val)
    }

    fn set_digital_converter_ctrl4(bits: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, bits.into());
        val.set_bits(8..20, Self::SET_CONVERTER_CONTROL4);
        Self(val)
    }

    fn get_eapd_enable() -> Self {
        let mut val = 0u32;
        val.set_bits(8..20, Self::GET_EAPD_ENABLE);
        Self(val)
    }

    fn set_eapd_enable(eapd: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, eapd.into());
        val.set_bits(8..20, Self::SET_EAPD_ENABLE);
        Self(val)
    }

    fn get_conn_sel_ctrl() -> Self {
        let mut val = 0u32;
        val.set_bits(8..20, Self::GET_CONN_SEL_CTRL);
        Self(val)
    }

    fn set_conn_sel_ctrl(idx: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(0..8, idx.into());
        val.set_bits(8..20, Self::SET_CONN_SEL_CTRL);
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
    const PARAMETER_OUTPUT_AMP_CAPABILITIES: u8 = 0x12;
    const PARAMETER_PIN_CAPABILITIES: u8 = 0x0c;
    const PARAMETER_AFG_CAPABILITIES: u8 = 0x08;

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

    fn afg_capabilities(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_AFG_CAPABILITIES)
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

    fn set_pin_widget_control(codec_addr: u8, node_id: u8, val: PinControl) -> Self {
        let verb = HDANodeCommandVerb::set_pin_widget_control(val);
        Self::command(codec_addr, node_id, verb)
    }

    fn set_converter_control(codec_addr: u8, node_id: u8, val: ConverterControl) -> Self {
        let verb = HDANodeCommandVerb::set_converter_control(val);
        Self::command(codec_addr, node_id, verb)
    }

    fn set_power_state(codec_addr: u8, node_id: u8, val: PowerState) -> Self {
        let verb = HDANodeCommandVerb::set_power_state(val);
        Self::command(codec_addr, node_id, verb)
    }

    fn set_amp_gain(codec_addr: u8, node_id: u8, amp_gain: AmpGain) -> Self {
        let verb = HDANodeCommandVerb::set_amp_gain(amp_gain);
        Self::command(codec_addr, node_id, verb)
    }

    fn get_out_amp_capabilties(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_OUTPUT_AMP_CAPABILITIES)
    }

    fn get_amp_gain(codec_addr: u8, node_id: u8) -> Self {
        let mut val = 0u32;
        val.set_bits(16..20, 0xb);
        val.set_bits(0..16, 0b1010000000000000);
        val.set_bits(20..28, node_id.into());
        val.set_bits(28..32, codec_addr.into());
        Self(val)
    }

    fn get_pin_capabilities(codec_addr: u8, node_id: u8) -> Self {
        Self::get_parameter(codec_addr, node_id, Self::PARAMETER_PIN_CAPABILITIES)
    }

    fn set_conn_select_ctrl(codec_addr: u8, node_id: u8, conn_idx: u8) -> Self {
        let verb = HDANodeCommandVerb::set_conn_select_ctrl(conn_idx);
        Self::command(codec_addr, node_id, verb)
    }

    fn set_beep_gen(codec_addr: u8, node_id: u8, divider: u8) -> Self {
        let verb = HDANodeCommandVerb::set_beep_gen(divider);
        Self::command(codec_addr, node_id, verb)
    }

    fn set_converter_format(node_addr: NodeAddr, format: u16) -> Self {
        let verb = HDANodeCommandVerb::set_converter_format(format);
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
    }

    fn get_digital_converter_ctrl(node_addr: NodeAddr) -> Self {
        let verb = HDANodeCommandVerb::get_digital_converter_ctrl();
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
    }

    fn set_digital_converter_ctrl(node_addr: NodeAddr, digi_ctrl: u32) -> [Self; 4] {
        let bits1 = digi_ctrl.get_bits(0..8).as_u8();
        let bits2 = digi_ctrl.get_bits(8..16).as_u8();
        let bits3 = digi_ctrl.get_bits(16..24).as_u8();
        let bits4 = digi_ctrl.get_bits(24..32).as_u8();
        let verb1 = HDANodeCommandVerb::set_digital_converter_ctrl1(bits1);
        let verb2 = HDANodeCommandVerb::set_digital_converter_ctrl2(bits2);
        let verb3 = HDANodeCommandVerb::set_digital_converter_ctrl3(bits3);
        let verb4 = HDANodeCommandVerb::set_digital_converter_ctrl4(bits4);
        [
            Self::command(node_addr.codec_addr(), node_addr.node_id(), verb1),
            Self::command(node_addr.codec_addr(), node_addr.node_id(), verb2),
            Self::command(node_addr.codec_addr(), node_addr.node_id(), verb3),
            Self::command(node_addr.codec_addr(), node_addr.node_id(), verb4)
        ]
    }

    fn eapd_enable(node_addr: NodeAddr) -> Self {
        let verb = HDANodeCommandVerb::get_eapd_enable();
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
    }

    fn set_eapd_enable(node_addr: NodeAddr, val: u8) -> Self {
        let verb = HDANodeCommandVerb::set_eapd_enable(val);
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
    }

    fn get_conn_sel_ctrl(node_addr: NodeAddr) -> Self {
        let verb = HDANodeCommandVerb::get_conn_sel_ctrl();
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
    }

    fn set_conn_sel_ctrl(node_addr: NodeAddr, idx: u8) -> Self {
        let verb = HDANodeCommandVerb::set_conn_sel_ctrl(idx);
        Self::command(node_addr.codec_addr(), node_addr.node_id(), verb)
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

/// A structure that is used to control aspects of a pin widget
#[repr(transparent)]
struct PinControl(u8);

impl PinControl {
    fn new() -> Self {
        Self(0)
    }

    fn input_enabled(self, enable: bool) -> Self {
        let mut val = self.0;
        if enable {
            val.set_bit(5);
        } else {
            val.unset_bit(5);
        }
        Self(val)
    }

    fn output_enabled(self, enable: bool) -> Self {
        let mut val = self.0;
        if enable {
            val.set_bit(6);
        } else {
            val.unset_bit(6);
        }
        Self(val)
    }
}

impl Into<u32> for PinControl {
    fn into(self) -> u32 {
        self.0.as_u32()
    }
}

/// A structure that controls aspects of an input
/// or output converter
#[repr(transparent)]
struct ConverterControl(u8);

impl ConverterControl {
    fn new() -> Self {
        Self(0)
    }

    fn stream(self, stream: u8) -> Self {
        assert!(stream < 16);
        let mut val = self.0;
        val.set_bits(4..8, stream);
        Self(val)
    }

    fn channel(self, channel: u8) -> Self {
        let mut val = self.0;
        val.set_bits(0..4, channel);
        Self(val)
    }
}

impl Into<u32> for ConverterControl {
    fn into(self) -> u32 {
        self.0.as_u32()
    }
}

/// The power states that a widget can be in
#[derive(Debug, Clone, Copy)]
enum PowerState {
    /// Fully powered on
    D0,
    Other
}

impl Into<u32> for PowerState {
    fn into(self) -> u32 {
        match self {
            Self::D0 => 0b000,
            _ => unimplemented!("The other states aren't used here")
        }
    }
}

#[repr(transparent)]
struct AmpGain(u16);

impl AmpGain {
    fn new() -> Self {
        Self(0)
    }

    fn mute(self, mute: bool) -> Self {
        let mut val = self.0;
        if mute {
            val.set_bit(7);
        } else {
            val.unset_bit(7);
        }
        Self(val)
    }

    fn output_amp(self, amp: bool) -> Self {
        let mut val = self.0;
        if amp {
            val.set_bit(15);
        } else {
            val.unset_bit(15);
        }
        Self(val)
    }

    fn left_amp(self, amp: bool) -> Self {
        let mut val = self.0;
        if amp {
            val.set_bit(13);
        } else {
            val.unset_bit(13);
        }
        Self(val)
    }

    fn right_amp(self, amp: bool) -> Self {
        let mut val = self.0;
        if amp {
            val.set_bit(12);
        } else {
            val.unset_bit(12);
        }
        Self(val)
    }

    fn gain(self, gain: u8) -> Self {
        let mut val = self.0;
        val.set_bits(0..7, gain.as_u16());
        Self(val)
    }

    fn index(self, idx: u8) -> Self {
        assert!((idx & 0xf) == idx);
        let mut val = self.0;
        val.set_bits(8..12, idx.into());
        Self(val)
    }
}

impl From<AmpGain> for u32 {
    fn from(gain: AmpGain) -> u32 {
        gain.0.into()
    }
}

/// A response received from the HDA controller into
/// the RIRB
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
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

    fn amp_capabilities_resp(&self) -> Result<HDANodeResponseAmpCapabilities, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(HDANodeResponseAmpCapabilities(self.response))
        }
    }

    fn pin_capabilities_resp(&self) -> Result<HDANodeResponsePinCapabilities, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(HDANodeResponsePinCapabilities(self.response))
        }
    }

    fn afg_cap_resp(&self) -> Result<AFGCapResp, ()> {
        match self.response {
            0 => Err(()),
            _ => Ok(AFGCapResp(self.response))
        }
    }

    fn digital_converter_ctrl_resp(&self) -> Result<DigitalConverterControl, ()> {
        Ok(DigitalConverterControl(self.response))
    }

    fn eapd_enable_resp(&self) -> Result<EAPDEnable, ()> {
        Ok(EAPDEnable(self.response.as_u8()))
    }

    fn get_conn_sel_ctrl_resp(&self) -> Result<GetConnSelCtrlResp, ()> {
        Ok(GetConnSelCtrlResp(self.response))
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

#[derive(Debug)]
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
    AFG,
    /// Modem Function Group
    MFG,
    Other
}

impl TryInto<HDANodeFunctionGroupType> for u8 {
    type Error = ();
    fn try_into(self) -> Result<HDANodeFunctionGroupType, ()> {
        match self {
            0x01 => Ok(HDANodeFunctionGroupType::AFG),
            0x02 => Ok(HDANodeFunctionGroupType::MFG),
            0x80..=0xff => Ok(HDANodeFunctionGroupType::Other),
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

    /// Returns true when power state control is
    /// supported on the associated widget
    fn power_ctrl_supported(&self) -> bool {
        self.0.get_bit(10) == BitState::Set
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

#[repr(transparent)]
struct HDANodeResponseAmpCapabilities(u32);

impl HDANodeResponseAmpCapabilities {
    fn offset(&self) -> u8 {
        self.0.get_bits(0..7).as_u8()
    }
    fn step_size(&self) -> u8 {
        self.0.get_bits(16..23).as_u8()
    }
    fn num_of_steps(&self) -> u8 {
        self.0.get_bits(8..15).as_u8()
    }
}

#[repr(transparent)]
struct HDANodeResponsePinCapabilities(u32);

impl HDANodeResponsePinCapabilities {
    fn eapd_capable(&self) -> bool {
        self.0.get_bit(16) == BitState::Set
    }

    fn input_capable(&self) -> bool {
        self.0.get_bit(5) == BitState::Set
    }

    fn output_capable(&self) -> bool {
        self.0.get_bit(4) == BitState::Set
    }
}

#[repr(transparent)]
struct AFGCapResp(u32);

impl AFGCapResp {
    /// Returns true if the AFG associated with this
    /// response has a beep generator
    fn has_beep_gen(&self) -> bool {
        self.0.get_bit(16) == BitState::Set
    }

    fn input_delay(&self) -> u8 {
        self.0.get_bits(8..12).as_u8()
    }

    fn output_delay(&self) -> u8 {
        self.0.get_bits(0..4).as_u8()
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct GetConnSelCtrlResp(u32);

impl GetConnSelCtrlResp {
    fn active_idx(&self) -> u8 {
        self.0.get_bits(0..8).as_u8()
    }
}

#[repr(transparent)]
struct DigitalConverterControl(u32);

impl DigitalConverterControl {
    fn pro(&self) -> bool {
        self.0.get_bit(6) == BitState::Set
    }
    fn pcm(&self) -> bool {
        // 0 indicates PCM
        self.0.get_bit(5) == BitState::Unset
    }
    fn digital_enabled(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }
}

impl Into<u32> for DigitalConverterControl {
    fn into(self) -> u32 {
        self.0
    }
}

struct DigitalConverterControlBuilder(u32);

impl DigitalConverterControlBuilder {
    fn new() -> Self {
        Self(0)
    }
    fn pro(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.set_bit(6);
        } else {
            val.unset_bit(6);
        }
        Self(val)
    }
    fn pcm(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.unset_bit(5);
        } else {
            val.set_bit(5);
        }
        Self(val)
    }
    fn digital_enabled(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.set_bit(0);
        } else {
            val.unset_bit(0);
        }
        Self(val)
    }
    fn value(self) -> DigitalConverterControl {
        DigitalConverterControl(self.0)
    }
}

#[repr(transparent)]
struct EAPDEnable(u8);

impl EAPDEnable {
    fn lr_swap(&self) -> bool {
        self.0.get_bit(2) == BitState::Set
    }
    fn eapd(&self) -> bool {
        self.0.get_bit(1) == BitState::Set
    }
    fn btl(&self) -> bool {
        self.0.get_bit(0) == BitState::Set
    }
}

impl Into<u8> for EAPDEnable {
    fn into(self) -> u8 {
        self.0
    }
}

struct EAPDEnableBuilder(u8);

impl EAPDEnableBuilder {
    fn new() -> Self {
        Self(0)
    }
    fn eapd(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.set_bit(1);
        } else {
            val.unset_bit(1);
        }
        Self(val)
    }
    fn lr_swap(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.set_bit(2);
        } else {
            val.unset_bit(2);
        }
        Self(val)
    }
    fn btl(self, set: bool) -> Self {
        let mut val = self.0;
        if set {
            val.set_bit(0);
        } else {
            val.unset_bit(0);
        }
        Self(val)
    }
    fn value(self) -> EAPDEnable {
        EAPDEnable(self.0)
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
    size: HDARingBufferSize,
    /// The memory mapped registers controlling the CORB
    regs: &'static mut CORBRegs
}

impl CORB {
    fn new(regs: &'static mut CORBRegs) -> Self {
        let mut corb_size = HDARingBufferSize::TwoFiftySix;
        let corb_size_capability = regs.size.size_capability();
        if !corb_size_capability.size256_supported() {
            if corb_size_capability.size16_supported() {
                corb_size = HDARingBufferSize::Sixteen;
            } else {
                corb_size = HDARingBufferSize::Two;
            }
        }
        Self {
            commands: [HDANodeCommand::null(); 256],
            write_pointer: 0,
            size: corb_size,
            regs
        }
    }

    fn add_command(&mut self, command: HDANodeCommand) {
        assert!(self.regs.control.corb_dma_engine_enabled());
        while self.regs.corbwp.write_pointer() != self.regs.corbrp.read_pointer() {}
        self.write_pointer = (self.write_pointer + 1) % self.size.entries_as_u16().as_usize();
        self.commands[self.write_pointer] = command;
        self.regs.corbwp.set_write_pointer(self.write_pointer.as_u8());
    }
    
    fn size(&self) -> HDARingBufferSize {
        self.size
    }

    /// Sets up the CORB to a ready state for communication
    /// with the HDA controller
    fn init(&mut self) {
        if self.regs.control.corb_dma_engine_enabled() {
            self.regs.control.enable_corb_dma_engine(false);
        }
        
        self.regs.size.set_corb_size(self.size());
        self.regs.set_corb_addr(&self.commands as *const _ as u64);

        self.regs.corbwp.set_write_pointer(0);

        self.regs.corbrp.set_read_pointer_reset(true);
        // The value must be read back to verify that it was reset
        while !self.regs.corbrp.read_pointer_reset() {}
        
        // The read pointer reset must then be cleared again
        self.regs.corbrp.set_read_pointer_reset(false);
        while self.regs.corbrp.read_pointer_reset() {}
        self.regs.control.enable_corb_dma_engine(true);
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
    size: HDARingBufferSize,
    /// The memory mapped registers controlling the RIRB
    regs: &'static mut RIRBRegs
}

impl RIRB {
    fn new(regs: &'static mut RIRBRegs) -> Self {
        let mut rirb_size = HDARingBufferSize::TwoFiftySix;
        let rirb_size_capability = regs.size.size_capability();
        if !rirb_size_capability.size256_supported() {
            if rirb_size_capability.size16_supported() {
                rirb_size = HDARingBufferSize::Sixteen;
            } else {
                rirb_size = HDARingBufferSize::Two;
            }
        }
        Self {
            responses: [HDANodeResponse::null(); 256],
            read_pointer: 0,
            size: rirb_size,
            regs
        }
    }

    fn read_next_response(&mut self) -> HDANodeResponse {
        assert!(self.regs.control.rirb_dma_engine_enabled());
        // Wait for the responses to be written
        while self.regs.rirbwp.write_pointer() == self.read_pointer.as_u8() {}
        // The buffer is circular, so when the last entry is reached
        // the read pointer should wrap around
        self.read_pointer = (self.read_pointer + 1) % self.size.entries_as_u16().as_usize();
        if self.read_pointer == self.size().entries_as_u16().as_usize() - 1 {
            self.regs.rirbwp.reset_write_pointer();
        }

        self.responses[self.read_pointer]
    }

    fn size(&self) -> HDARingBufferSize {
        self.size
    }

    fn init(&mut self) {
        if self.regs.control.rirb_dma_engine_enabled() {
            self.regs.control.enable_rirb_dma_engine(false);
        }

        self.regs.size.set_rirb_size(self.size());
        self.regs.set_rirb_addr(&self.responses as *const _ as u64);

        self.regs.rirbwp.reset_write_pointer();

        self.regs.response_interrupt_count.set_response_interrupt_count(255);

        self.regs.control.enable_rirb_dma_engine(true);

    }
}

impl Index<usize> for RIRB {
    type Output = HDANodeResponse;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.responses[idx]
    }
}

struct Commander {
    corb: CORB,
    rirb: RIRB
}

impl Commander {
    fn new(corb_regs: &'static mut CORBRegs, rirb_regs: &'static mut RIRBRegs) -> Self {
        Self {
            corb: CORB::new(corb_regs),
            rirb: RIRB::new(rirb_regs)
        }
    }
    fn init(&mut self) {
        self.corb.init();
        self.rirb.init();
    }

    fn command(&mut self, command: HDANodeCommand) -> HDANodeResponse {
        self.corb.add_command(command);
        self.rirb.read_next_response()
    }
}

/// A 16-bit sample container as specified in the HDA spec
#[derive(Clone, Copy, Debug)]
#[repr(C, align(2))]
pub struct Sample(pub u16);

