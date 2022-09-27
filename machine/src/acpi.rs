use core::{mem, slice};
use core::iter::Iterator;

/// The Root System Description Pointer (RSDP) contains the info
/// used to find the RSDT
#[derive(Debug, PartialEq)]
pub enum RSDP {
    V1(&'static RSDPDescriptorV1),
    V2(&'static RSDPDescriptorV2),
    None
}

impl RSDP {
    /// Check if the RSDP's checksum is valid
    pub fn is_valid(&self) -> bool {
        match *self {
            Self::V1(rsdp_descr) => rsdp_descr.checksum_is_valid(),
            Self::V2(rsdp_descr) => rsdp_descr.checksum_is_valid(),
            Self::None => false
        }
    }

    /// Retrieve the address of the RSDT
    pub fn rsdt_ptr(&self) -> *const RSDT {
        match *self {
            Self::V1(rsdp) => rsdp.rsdt_ptr(),
            Self::V2(rsdp) => rsdp.rsdt_ptr(),
            Self::None => unreachable!()
        }
    }
}

/// The Root System Description Pointer (RSDP) is a data structure used
/// in the ACPI programming interface

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RSDPDescriptorV1 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32
}

impl RSDPDescriptorV1 {
    /// Checks if the checksum is valid
    ///
    /// To do so, every byte has to be added up and it is valid if the lower byte
    /// of the addition is 0
    fn checksum_is_valid(&self) -> bool {
        let bytes = unsafe { slice::from_raw_parts(self as *const Self as *const u8, mem::size_of::<Self>()) };
        let sum = bytes
            .iter()
            .fold(0u64, |sum, x| sum + *x as u64);
        sum & 0xff == 0
    }

    /// Retrieve the location of the RSDT
    fn rsdt_ptr(&self) -> *const RSDT {
        self.rsdt_address as *const RSDT
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RSDPDescriptorV2 {
    first_part: RSDPDescriptorV1,
    length: u8,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3]
}

impl RSDPDescriptorV2 {
    /// Checks if the checksum is valid
    ///
    /// Does the same thing as the RSDPDescriptorV1 struct for the `first_part`
    /// then does that same thing for the other fields 
    fn checksum_is_valid(&self) -> bool {
        let bytes = unsafe { slice::from_raw_parts(self as *const Self as *const u8, mem::size_of::<Self>()) };
        let sum = bytes
            .iter()
            .fold(0u64, |sum, x| sum + *x as u64);
        sum & 0xff == 0 && self.first_part.checksum_is_valid()
    }

    /// Retrieve the location of the RSDT
    fn rsdt_ptr(&self) -> *const RSDT {
        self.first_part.rsdt_address as *const RSDT
    }
}

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

/// The RSDP is located either within the first 1KB of the extended BIOS data area (EBDA)
/// or in the memory region from 0x000E0000 to 0x000FFFFF
///
/// To find it, the string "RSD PTR " has to be found in one of the two areas
#[cfg(feature = "bios")]
pub unsafe fn detect_rsdp() -> Option<RSDP> {
    let ebda = 0x9fc00 as *const u8;
    let mut rsdp = Some(RSDP::None);
    // Searching the first 1Kib for the RSDP
    for i in 0..2isize.pow(10) {
        let curr_ptr = ebda.offset(i) as *const RSDPDescriptorV1;
        if &(*curr_ptr).signature == RSDP_SIGNATURE {
            rsdp = parse_rsdp(curr_ptr);
            break;
        }
    }
    // Searching the other possible location
    for i in 0x000e0000..=0x000fffff {
        let mut curr_ptr = i as *const RSDPDescriptorV1;
        if &(*curr_ptr).signature == RSDP_SIGNATURE {
            rsdp = parse_rsdp(curr_ptr);
            break;
        }
    }
    rsdp
}

#[cfg(not(feature = "bios"))]
pub unsafe fn detect_rsdp() -> Option<RSDP> {
    use crate::uefi::{get_systable, EFIConfigurationTableEntry, Guid};
    // The GUID of the RSDP structure in ACPI 1.0 according to the ACPI
    // specification 6.2, section 5.2.5.2
    const ACPI_1_RSDP_GUID: Guid = Guid {
        first: 0xeb9d2d30,
        second: 0x2d88,
        third: 0x11d3,
        fourth: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]
    };
    // The GUID of the RSDP structure in ACPI 2.0
    const ACPI_2_RSDP_GUID: Guid = Guid {
        first: 0x8868e871,
        second: 0xe4f1,
        third: 0x11d3,
        fourth: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81]
    };
    let systable = get_systable();
    if systable.is_none() {
        return None;
    }
    let systable = systable.unwrap();
    let no_of_entries = systable.no_of_entries_in_config_table();
    let config_table = systable.config_table() as *const EFIConfigurationTableEntry;
    for i in 0..no_of_entries as isize {
        let entry_ptr = config_table.offset(i);
        let entry = entry_ptr.read();
        if entry.vendor_guid == ACPI_1_RSDP_GUID {
            let rsdp = entry_ptr.read().vendor_table as *mut RSDPDescriptorV1;
            return Some(RSDP::V1(&*rsdp));
        }
        if entry.vendor_guid == ACPI_2_RSDP_GUID {
            let rsdp = entry_ptr.read().vendor_table as *mut RSDPDescriptorV2;
            return Some(RSDP::V2(&*rsdp));
        }
    }
    None
}

pub trait SDTTable {
    /// Checks if the table is valid
    ///
    /// The sum of all values in an SDT table mod 0x100 must equal 0
    unsafe fn is_valid(&self) -> bool;
}

/// The Root System Description Table (RSDT).
/// This is the main System Description Table.
/// It contains pointers to all other System Description Tables
#[repr(C)]
pub struct RSDT {
    header: SDTHeader
}

impl RSDT {
    /// Returns a slice to the entries in the RSDT
    ///
    /// These entries are the 32-bit addresses to the other SDTs pointed to
    /// by the RSDT.
    unsafe fn entries_bytes(&self) -> &[u8] {
        // The number of entries * 4, since the entries are 32 bit addresses
        let size_of_entries = self.header.length as usize - SDT_HEADER_SIZE;
        // Had to use a pointer to u8s instead of u32s because of alignment issues
        // with the slice::from_raw_parts function
        let entries_start_ptr = (self as *const Self as *const u8).offset(SDT_HEADER_SIZE as isize);
        slice::from_raw_parts(entries_start_ptr, size_of_entries)
    }

    /// The FADT's address is one of the addresses in the RSDT's
    /// `address_of_other_STDs`. It's signature is "FACP"
    pub unsafe fn find_fadt(&self) -> Option<&FADT> {
        self.find_table::<FADT>(FADT_SIGNATURE)
    }

    pub unsafe fn find_madt(&self) -> Option<&MADT> {
        self.find_table::<MADT>(MADT_SIGNATURE)
    }

    unsafe fn find_table<T>(&self, table_sig: ACPITableSig) -> Option<&T> {
        for sdt_addr_array in self.entries_bytes().array_windows::<4>() {
            let sdt_addr = u32::from_le_bytes(*sdt_addr_array);
            let sdt_header = sdt_addr as *const u32 as *const SDTHeader;
            if &(*sdt_header).signature == table_sig {
                return Some(&*(sdt_addr as *const u32 as *const T))
            }
        }
        None
    }
}

impl SDTTable for RSDT {
    /// Checks if the RSDT is valid
    unsafe fn is_valid(&self) -> bool {
        is_valid(self, self.header.length)
    }
}


const SDT_HEADER_SIZE: usize = mem::size_of::<SDTHeader>();

/// The header in a System Description Table
// 288 bytes
#[repr(C)]
struct SDTHeader {
    /// Signature of the SDT
    signature: [u8; 4],
    /// The total size of the table: header size + all entries in the SDT table itself
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32
}

const FADT_SIGNATURE: &[u8; 4] = b"FACP";

/// The Fixed ACPI Description Table (FADT)
///
/// Contains the DSDT pointer which will be used to ...
#[repr(C)]
pub struct FADT {
    header: SDTHeader,
    firmware_ctrl: u32,
    /// The address of the DSDT
    dsdt_address: u32,
    unneeded_fields1: [u8; 20],
    pm1a_ctrl_block: u32,
    pm1b_ctrl_block: u32,
    unneeded_fields2: [u8; 17],
    pm1_ctrl_length: u8,
    /// We don't need the other fields
    others: [u8; 152]
}

impl FADT {
    /// Retrives the pointer to the DSDT
    pub fn dsdt_ptr(&self) -> *const DSDT {
        self.dsdt_address as *const DSDT
    }

    pub fn pm1a_ctrl_block(&self) -> u32 {
        self.pm1a_ctrl_block
    }

    pub fn pm1b_ctrl_block(&self) -> u32 {
        self.pm1b_ctrl_block
    }
}

impl SDTTable for FADT {
    /// Checks if the FADT is valid
    unsafe fn is_valid(&self) -> bool {
        is_valid(self, self.header.length)
    }
}

/// The Differentiated System Description Table
#[repr(C)]
pub struct DSDT {
    header: SDTHeader
}

impl DSDT {
    /// Gets the value of SLP_TYPa and SLP_TYPb from the AML \_S5 object bytecode
    /// The _s5 object contains one of the values needed to shut down the computer
    pub unsafe fn get_slp_typ(&self) -> Option<(u8, u8)> {
        let start_ptr = (self as *const Self as *const u8).offset(SDT_HEADER_SIZE as isize);
        let bytes = slice::from_raw_parts(start_ptr, self.header.length as usize - SDT_HEADER_SIZE);
        for chunk in bytes.array_windows::<4>() {
            if chunk == b"_S5_" {
                if self.s5_object_is_valid(chunk as *const u8) {
                    let byteprefix = 0x0a;
                    let mut slp_typa_ptr = (chunk as *const u8).offset(7);
                    if *slp_typa_ptr == byteprefix {
                        slp_typa_ptr = slp_typa_ptr.offset(1);
                    }
                    let mut slp_typb_ptr = slp_typa_ptr.offset(1);
                    if *slp_typb_ptr == byteprefix {
                        slp_typb_ptr = slp_typb_ptr.offset(1);
                    }
                    return Some((*slp_typa_ptr, *slp_typb_ptr));
                }
            }
        }
        None
    }

    /// This functions checks if the S5 object has the expected beginning bytes
    /// of an AML structure. `s5_obj_ptr` points to the "_S5_" string in the bytecode
    ///
    /// bytecode of the \_S5 object
    /// -----------------------------------------
    ///        | (optional) |    |    |    |   
    /// NameOP | \          | _  | S  | 5  | _
    /// 08     | 5A         | 5F | 53 | 35 | 5F
    ///
    /// -----------------------------------------------------------------------------------------------------------
    ///           |           |              | ( SLP_TYPa   ) | ( SLP_TYPb   ) | ( Reserved   ) | (Reserved    )
    /// PackageOP | PkgLength | NumElements  | byteprefix Num | byteprefix Num | byteprefix Num | byteprefix Num
    /// 12        | 0A        | 04           | 0A         05  | 0A          05 | 0A         05  | 0A         05
    ///
    ///----this-structure-was-also-seen----------------------
    /// PackageOP | PkgLength | NumElements |
    /// 12        | 06        | 04          | 00 00 00 00
    ///
    /// (Pkglength bit 6-7 encode additional PkgLength bytes [shouldn't be the case here])
    unsafe fn s5_object_is_valid(&self, s5_obj_ptr: *const u8) -> bool {
        *s5_obj_ptr.offset(-1) == 0x08
            || (*s5_obj_ptr.offset(-2) == 0x08 && *s5_obj_ptr.offset(-1) ==b'\\' )
            && *s5_obj_ptr.offset(4) == 12
    }
}

impl SDTTable for DSDT {
    /// Checks if the DSDT is valid
    unsafe fn is_valid(&self) -> bool {
        is_valid(self, self.header.length)
    }
}

/// Checks if an SDT table is valid
///
/// All bytes of the table summed together must be equal to 0 mod 0x100
fn is_valid<T>(table: &T, size: u32) -> bool {
    let bytes = unsafe { slice::from_raw_parts(table as *const T as *const u8, size as usize) };
    let sum = bytes
        .iter()
        .fold(0u64, |sum, x| sum + *x as u64);
    sum % 0x100 == 0
}

unsafe fn parse_rsdp(raw_rsdp_ptr: *const RSDPDescriptorV1) -> Option<RSDP> {
    let version = get_acpi_version(raw_rsdp_ptr);
    if version.is_none() {
        return None
    }
    let version = version.unwrap();
    if version == ACPIVersion::Other {
        let rsdp_ptr = raw_rsdp_ptr as *const RSDPDescriptorV2;
        Some(RSDP::V2(&*rsdp_ptr))
    } else {
        Some(RSDP::V1(&*raw_rsdp_ptr))
    }
}

/// The revision field is used to figure out the version of ACPI the BIOS
/// is using
///
/// A value of 0 in the revision field means ACPI version 1.0 is used,
/// and a value of 2 is used for versions 2.0 to 6.1
unsafe fn get_acpi_version(rsdp_ptr: *const RSDPDescriptorV1) -> Option<ACPIVersion> {
    match (*rsdp_ptr).revision {
        0 => Some(ACPIVersion::One),
        2 => Some(ACPIVersion::Other),
        v => None
    }
}

#[derive(PartialEq)]
enum ACPIVersion {
    One,
    Other
}

const MADT_SIGNATURE: &[u8; 4] = b"APIC";

/// The Multiple APIC Description Table (MADT)
#[repr(C)]
pub struct MADT {
    header: SDTHeader,
    /// A 32 bit physical address at which each processor can access
    /// its local interrupt controller
    local_interrupt_controller_addr: u32,
    /// Multiple APIC flags
    // After this is a list of interrupt controller structures
    // that describe the interrupt features of the machine
    flags: MultipleAPICFlags
}

impl MADT {
    /*/// Returns a slice to the entries in the RSDT
    ///
    /// These entries are the 32-bit addresses to the other SDTs pointed to
    /// by the RSDT.
    unsafe fn entries_bytes(&self) -> &[u8] {
        // The number of entries * 4, since the entries are 32 bit addresses
        let size_of_entries = self.header.length as usize - SDT_HEADER_SIZE;
        // Had to use a pointer to u8s instead of u32s because of alignment issues
        // with the slice::from_raw_parts function
        let entries_start_ptr = (self as *const Self as *const u8).offset(SDT_HEADER_SIZE as isize);
        slice::from_raw_parts(entries_start_ptr, size_of_entries)
    }*/
    pub fn flags(&self) -> MultipleAPICFlags {
        self.flags
    }

    pub fn interrupt_controllers(&self) -> InterruptControllersIter {
        let start_ptr = unsafe { (self as *const _ as *mut u8).offset((SDT_HEADER_SIZE + core::mem::size_of::<u32>() + mem::size_of::<MultipleAPICFlags>()) as isize) };
        InterruptControllersIter {
            start_ptr,
            entries_size: self.header.length as usize - SDT_HEADER_SIZE,
            curr_entry_ptr: start_ptr
        }
    }

    pub fn local_interrupt_controller_addr(&self) -> u32 {
        self.local_interrupt_controller_addr
    }
}

impl SDTTable for MADT {
    /// Checks if the RSDT is valid
    unsafe fn is_valid(&self) -> bool {
        is_valid(self, self.header.length)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MultipleAPICFlags(u32);

impl MultipleAPICFlags {
    /// A one in bit 0 indicates that the system also has a PC-AT-compatible
    /// dual-8259 setup.
    /// The 8259 vectors must be disabled (that is,
    /// masked) when enabling the ACPI APIC operation.
    pub fn pc_at_compatible(&self) -> bool {
        self.0 & 0x1 == 1
    }
}

/// An entry in the MADT that describe the interrupt features
/// of the machine
#[repr(C)]
pub struct InterruptController {
    type_: u8,
    length: u8
}

impl InterruptController {
    pub fn type_(&self) -> u8 {
        self.type_
    }
}

type ACPITableSig = &'static [u8; 4];

pub struct InterruptControllersIter {
    start_ptr: *const u8,
    entries_size: usize,
    curr_entry_ptr: *const u8
}

impl Iterator for InterruptControllersIter {
    type Item = &'static InterruptController;
    fn next(&mut self) -> Option<Self::Item> {
        if (self.curr_entry_ptr as usize - self.start_ptr as usize) as usize >= self.entries_size {
            None
        } else {
            let ptr = self.curr_entry_ptr.cast::<InterruptController>();
            let curr_entry_len = unsafe { ptr.read().length } as isize;
            assert!(curr_entry_len != 0);
            self.curr_entry_ptr = unsafe { self.curr_entry_ptr.offset(curr_entry_len) };
            unsafe { Some(&*ptr) }
        }
    }
}
