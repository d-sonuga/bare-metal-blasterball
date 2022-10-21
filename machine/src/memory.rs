//! Abstractions for dealing with memory

use core::ops::{Add, Sub, BitAnd, Index, AddAssign};
use core::{slice, fmt};
use num::Integer;

const MAX_MEM_MAP_SIZE: usize = 100;

/// A wrapper around a u64 to ensure it always remains a valid
/// virtual address, that is, the 49th bit upwards is sign extended
/// because only the lower 48 bits are used as a valid virtual address
#[derive(Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Addr(u64);

impl Addr {
    #[inline]
    pub const fn new(n: u64) -> Addr {
        if n & 0xffff000000000000 != 0 {
            panic!("Address too big to be a valid virtual address");
        }
        // The upper 16 bits must be the same as the 48th bit
        // The case where The 48th bit is 1
        if (n >> 47) & 0x1 == 1 {
            Addr(n | 0xffff000000000000)
        } else {
            Addr(n)
        }
    }
    
    /// Returns a new VAddr clearing any upper 16 bits
    #[inline]
    pub fn new_trunc(n: u64) -> Addr {
        Addr::new(n & 0xffffffffffff)
    }

    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T) -> Addr {
        Self::new(ptr as u64)
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }
}

impl Add<u64> for Addr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Addr {
        Addr::new(self.0 + rhs)
    }
}

impl Add<usize> for Addr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Addr {
        Addr::new(self.0 + rhs as u64)
    }
}

impl Sub for Addr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Addr) -> Addr {
        Addr::new(self.0 - rhs.0)
    }
}

impl Sub<u64> for Addr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Addr {
        Addr::new(self.0 - rhs)
    }
}

impl BitAnd<u64> for Addr {
    type Output = Addr;

    #[inline]
    fn bitand(self, rhs: u64) -> Addr {
        Addr::new(self.0 & rhs)
    }
}

impl fmt::LowerHex for Addr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Addr({:#x})", self.0)
    }
}

impl fmt::Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self)
    }
}

impl AddAssign<u64> for Addr {
    fn add_assign(&mut self, rhs: u64){
        *self = Addr::new(self.0 + rhs);
    }
}

impl PartialEq<u64> for Addr {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Addr> for u64 {
    fn eq(&self, other: &Addr) -> bool {
        *self == other.as_u64()
    }
}

/// A firmware agnostic map of the computer's memory
#[repr(C)]
pub struct MemMap {
    /// The memory regions
    pub entries: [MemRegion; MAX_MEM_MAP_SIZE],
    /// The next index in `entries` to place a memory region
    next_entry_index: u64
}

impl MemMap {
    /// Creates a new empty memory map
    #[inline]
    pub fn new() -> MemMap {
        MemMap {
            entries: [MemRegion::empty(); MAX_MEM_MAP_SIZE],
            next_entry_index: 0
        }
    }

    /// Adds a new memory region to the map, if there is still
    /// in `entries`
    #[inline]
    pub fn add_region(&mut self, region: MemRegion) -> Result<(), MemMapError> {
        if self.next_entry_index >= MAX_MEM_MAP_SIZE as u64 {
            return Err(MemMapError::EntriesFull);
            //panic!("Too many regions in mem map");
        }
        self.entries[self.next_entry_index as usize] = region;
        self.next_entry_index += 1;
        self.sort();
        Ok(())
    }

    /// Sorts the regions in the memory map
    #[inline]
    pub fn sort(&mut self){
        fn is_less(r1: MemRegion, r2: MemRegion) -> bool {
            if r1.range.is_empty(){
                false
            } else if r2.range.is_empty(){
                true
            } else {
                if r1.range.start_addr != r2.range.start_addr {
                    r1.range.start_addr < r2.range.start_addr
                } else {
                    r1.range.end_addr < r2.range.end_addr
                }
            }
        }
        // Insertion sort
        // <https://en.wikipedia.org/wiki/Insertion_sort>
        for i in 1..self.entries.len(){
            let key = self.entries[i];
            let mut j = (i - 1) as isize;
            while j >= 0isize && is_less(key, self.entries[j as usize]){
                    self.entries[j as usize + 1] = self.entries[j as usize];
                    j = j as isize - 1;
            }
            j = j + 1;
            self.entries[j as usize] = key;
        }
    }

    fn remove_usable_region_overlaps(&mut self) {
        let mut mmap_iter = self.entries.iter_mut().peekable();
        while let Some(region) = mmap_iter.next(){
            if let Some(next) = mmap_iter.peek(){
                if region.range.end_addr > next.range.start_addr
                    && region.region_type == MemRegionType::Usable {
                        // region's end_addr overlaps with the next region's start_addr
                        // Remove the overlap
                    region.range.end_addr = next.range.start_addr;
                }
            }
        }
    }
}

impl Index<usize> for MemMap {
    type Output = MemRegion;
    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < MAX_MEM_MAP_SIZE);
        &self.entries[idx]
    }
}

#[derive(Debug)]
pub enum MemMapError {
    /// This error is returned when an attempt to add
    /// a memory region to a full memory map has been made
    EntriesFull
}

impl fmt::Debug for MemMap {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("MemMap {\n\t")?;
        f.write_str("entries: ")?;
        f.debug_list().entries(self.entries.iter()).finish()?;
        f.write_str("\n\t")?;
        f.write_str("next_entry_index: ")?;
        f.write_fmt(format_args!("{:?}", self.next_entry_index))?;
        f.write_str("\n}")?;
        Ok(())
    }
}

/// A firmware agnostic representation of a region of memory
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MemRegion {
    /// The expanse of the region
    pub range: AddrRange,
    /// The type of the region
    pub region_type: MemRegionType
}

impl MemRegion {
    /// Creates a empty memory region that spans no address
    #[inline]
    pub fn empty() -> MemRegion {
        MemRegion {
            range: AddrRange {
                start_addr: Addr::new(0),
                end_addr: Addr::new(0)
            },
            region_type: MemRegionType::Empty
        }
    }
}

impl fmt::Debug for MemRegion {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "MemRegion {{ range: {:?}, region_type: {:?} }}", self.range, self.region_type)
    }
}

/// A range of addresses of the form start_addr..end_addr,
/// that is, end_addr is not included in the range
#[derive(Copy, Clone)]
#[repr(C)]
pub struct AddrRange {
    pub start_addr: Addr,
    pub end_addr: Addr
}

impl AddrRange {
    /// Creates a new AddrRange that spans `start_addr`..=`end_addr`-1
    #[inline]
    pub fn new(start_addr: u64, end_addr: u64) -> AddrRange {
        let end_addr = end_addr.checked_sub(1).or(Some(0));
        AddrRange {
            start_addr: Addr::new(start_addr),
            end_addr: Addr::new(end_addr.unwrap())
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start_addr == self.end_addr
    }

    #[inline]
    pub fn start_addr(&self) -> Addr {
        self.start_addr
    }

    #[inline]
    pub fn end_addr(&self) -> Addr {
        self.end_addr
    }

    #[inline]
    pub fn size(&self) -> u64 {
        (self.end_addr - self.start_addr).as_u64()
    }
}

impl fmt::Debug for AddrRange {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AddrRange {{ {:#x}..{:#x} }}", self.start_addr(), self.end_addr())
    }
}


/// Tells what a region of memory is being used for
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MemRegionType {
    /// The region is free and available for use
    Usable,
    /// The region is being used
    InUse,
    /// The region is reserved
    Reserved,
    /// In use by ACPI
    AcpiReclaimable,
    /// In use by ACPI
    AcpiNvs,
    /// The region is bad and can't be used
    BadMem,
    /// The app code
    App,
    /// The app stack
    AppStack,
    /// Page tables
    PageTable,
    /// For the bootloader
    Bootloader,
    /// An empty region
    Empty,
    /// The region is being used for heap memory
    Heap
}

impl MemRegionType {
    #[inline]
    fn is_usable(&self) -> bool {
        *self == MemRegionType::Usable
    }

    #[inline]
    fn as_str(&self) -> &str {
        match *self {
            MemRegionType::Usable => "usable",
            MemRegionType::Reserved => "reserved",
            _ => "other"
        }
    }
}

/// A memory region detected by the BIOS INT 0x15,eax=0xe830 function
///
/// # References
///
/// * <https://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820>
#[repr(C)]
#[derive(Copy, Clone)]
pub struct E820MemRegion {
    pub start_addr: u64,
    pub len: u64,
    pub region_type: u32,
    pub acpi_extended_attrs: u32
}

impl From<E820MemRegion> for MemRegion {
    #[inline]
    fn from(region: E820MemRegion) -> MemRegion {
        let region_type = match region.region_type {
            1 => MemRegionType::Usable,
            2 => MemRegionType::Reserved,
            3 => MemRegionType::AcpiReclaimable,
            4 => MemRegionType::AcpiNvs,
            5 => MemRegionType::BadMem,
            t => panic!("Where the hell did this region type come from?! {}", t)
        };
        MemRegion {
            range: AddrRange::new(region.start_addr, region.start_addr + region.len),
            region_type
        }
    }
}

/// A memory region detected with the EFI_BOOT_SERVICES.GetMemoryMap
/// UEFI service as described in the UEFI spec, version 2.7, chapter 7, section 2
#[derive(Clone)]
#[repr(C)]
pub struct EFIMemRegion {
    /// The type of the memory region
    type_: EFIMemRegionType,
    /// For alignment dictated by UEFI
    //padding: u32,
    /// Physical address of the first byte in the memory region
    physical_start: Addr,
    /// Virtual address of the first byte in a memory region aligned on a 4Kib boundary
    ///
    /// For some reason, when the map is retrieved, this field will be 0, but since
    /// UEFI identity maps the addresses, that's not a problem, because `physical_start`
    /// will contain the valid address
    virtual_start: Addr,
    /// No of 4Kib pages in the mem region
    no_of_pages: u64,
    /// Attributes of the memory region that describe the bit
    /// mask of capabilities for that memory region, and not
    /// necessarily the current settings for that memory region.
    attribute: u64
}

impl fmt::Debug for EFIMemRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EFIMemDescriptor")
            .field("type", &self.type_)
            .field("physical_start", &self.physical_start)
            .field("virtual_start", &self.virtual_start)
            .field("no_of_pages", &Hex(self.no_of_pages))
            .field("attribute", &Hex(self.attribute))
            .finish()
    }
}

/// The type of an EFIMemRegion as defined by the
/// UEFI spec, version 2.7, chapter 7, section 2
#[derive(Debug, Clone)]
#[repr(u8)]
pub enum EFIMemRegionType {
    /// Unavailable for use
    Reserved = 0,
    /// The app code
    LoaderCode = 1,
    /// The app data
    LoaderData = 2,
    /// The code of the UEFI boot services
    ///
    /// After exiting boot services in the bootloader,
    /// these regions won't be needed anymore
    BootServicesCode = 3,
    /// The data of the UEFI boot services
    ///
    /// After exiting boot services in the bootloader,
    /// these regions won't be needed anymore
    BootServicesData = 4,
    /// The data of the UEFI runtime services
    ///
    /// According to the UEFI spec, memory in this range
    /// is to be preserved
    RuntimeServicesCode = 5,
    /// The data of the UEFI runtime services
    ///
    /// According to the UEFI spec, memory in this range
    /// is to be preserved
    RuntimeServicesData = 6,
    /// Free for general use
    Conventional = 7,
    /// Bad unusable memory
    Unusable = 8,
    /// Memory that holds ACPI tables
    ///
    /// According to the UEFI spec, this memory is
    /// to be preserved until after ACPI is enabled
    AcpiReclaimable = 9,
    /// Address space reserved for use by the firmware
    ///
    /// According to the UEFI spec, this memory is
    /// to be preserved
    AcpiNvs = 10,
    /// Used by system firmware memory mapped IO management
    ///
    /// According to the UEFI spec, this memory is not to
    /// be used at all
    MemMappedIO = 11,
    /// System memory mapped IO region used to translate memory
    /// cycles to IO cycles by the processor
    ///
    /// According to the UEFI spec, this memory is not to
    /// be used at all
    MemMappedIOPortSpace = 12,
    /// Address space reserved by the firmware for code that
    /// is part of the processor
    PalCode = 13,
    /// A memory region that operates as conventional memory
    /// and supports byte addressable non-volatility
    Persistent = 14
}

impl From<EFIMemRegion> for MemRegion {
    /// Converts an EFIMemRegion into a firmware agnostic MemRegion
    ///
    /// This function assumes that boot services have already been exited
    /// because it marks boot services code and data as usable
    fn from(region: EFIMemRegion) -> MemRegion {
        const PAGE_SIZE_4KIB: u64 = 4 * 2u64.pow(10);
        let region_type = match region.type_ {
            EFIMemRegionType::Reserved => MemRegionType::Reserved,
            EFIMemRegionType::LoaderCode => MemRegionType::App,
            EFIMemRegionType::LoaderData => MemRegionType::App,
            EFIMemRegionType::BootServicesCode => MemRegionType::InUse,
            EFIMemRegionType::BootServicesData => MemRegionType::InUse,
            EFIMemRegionType::RuntimeServicesCode => MemRegionType::InUse,
            EFIMemRegionType::RuntimeServicesData => MemRegionType::InUse,
            EFIMemRegionType::Conventional => MemRegionType::Usable,
            EFIMemRegionType::Unusable => MemRegionType::BadMem,
            EFIMemRegionType::AcpiReclaimable => MemRegionType::AcpiReclaimable,
            EFIMemRegionType::AcpiNvs => MemRegionType::AcpiNvs,
            EFIMemRegionType::MemMappedIO => MemRegionType::InUse,
            EFIMemRegionType::MemMappedIOPortSpace => MemRegionType::InUse,
            EFIMemRegionType::PalCode => MemRegionType::InUse,
            EFIMemRegionType::Persistent => MemRegionType::InUse
        };
        MemRegion {
            range: AddrRange::new(
                region.physical_start.as_u64(),
                (region.physical_start + region.no_of_pages * PAGE_SIZE_4KIB).as_u64()
            ),
            region_type
        }
    }
}

/// A chunk of allocated memory
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MemChunk {
    pub start_addr: Addr,
    pub size: u64
}

impl MemChunk {
    #[inline]
    pub fn start_addr(&self) -> Addr {
        self.start_addr
    }

    #[inline]
    pub fn end_addr(&self) -> Addr {
        self.start_addr + self.size
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[inline]
    pub fn range(&self) -> AddrRange {
        AddrRange {
            start_addr: self.start_addr,
            end_addr: self.start_addr + self.size
        }
    }
}

/// A structure used by the bootloader to assign
/// memory ranges to specific uses
pub struct MemAllocator<'a> {
    mmap: &'a mut MemMap
}

impl<'b> MemAllocator<'b> {
    #[inline]
    pub fn new(mmap: &mut MemMap) -> MemAllocator {
        MemAllocator {
            mmap
        }
    }
    
    pub fn mark_alloc_region(&mut self, region: MemRegion){
        for r in self.mmap.entries.iter_mut(){
            if region.range.start_addr < r.range.end_addr {
                if region.range.end_addr > r.range.start_addr {
                    if !r.region_type.is_usable() {
                        panic!("Supposedly, region {:?} seems to be unusable", region);
                    }
                    if region.range.start_addr == r.range.start_addr {
                        if region.range.end_addr < r.range.end_addr {
                            r.range.start_addr = region.range.end_addr;
                            self.mmap.add_region(region).unwrap();
                        } else {
                            *r = region;
                        }
                    } else if region.range.start_addr > r.range.start_addr {
                        if region.range.end_addr < r.range.end_addr {
                            let mut left_r = r.clone();
                            left_r.range.end_addr = region.range.start_addr;
                            r.range.start_addr = region.range.end_addr;
                            self.mmap.add_region(left_r).unwrap();
                            self.mmap.add_region(region).unwrap();
                        } else {
                            r.range.end_addr = region.range.start_addr;
                            self.mmap.add_region(region).unwrap();
                        }
                    } else {
                        r.range.start_addr = region.range.end_addr;
                        self.mmap.add_region(region).unwrap();
                    }
                    return;
                }
            }
        }
        panic!("Supposedly, region {:?} is not usable", region);
    }

    pub fn alloc_mem(&mut self, region_type: MemRegionType, size: u64) -> Option<MemChunk> {
        let mut mmap_regions = self.mmap.entries.iter_mut().peekable();
        while let Some(region) = mmap_regions.next(){
            if region.region_type == region_type {
                if let Some(next_region) = mmap_regions.peek() {
                    let space_left = size - region.range.size();
                    if next_region.range.start_addr == region.range.end_addr
                        && next_region.range.size() >= space_left
                        && next_region.region_type.is_usable()
                    {
                        region.range.end_addr += space_left;
                        mmap_regions.next().unwrap().range.start_addr += space_left;
                        return Some(MemChunk {
                            start_addr: region.range.start_addr,
                            size
                        })
                    }
                }
            }
        }
        
        // Made this an inner function so won't have to borrow self mutably more than once
        fn split_usable_region<'a, I: Iterator<Item=&'a mut MemRegion>>(
            regions: &mut I,
            size: u64
        ) -> Option<(MemChunk, AddrRange)> {
            for region in regions {
                if region.region_type.is_usable() && region.range.size() >= size {
                    let newly_allocd_mem_start_addr = region.range.start_addr;
                    let newly_allocd_mem_end_addr = newly_allocd_mem_start_addr + size;
                    region.range.start_addr = newly_allocd_mem_end_addr;
                    let range = AddrRange {
                        start_addr: newly_allocd_mem_start_addr,
                        end_addr: newly_allocd_mem_end_addr
                    };
                    return Some((MemChunk {
                        start_addr: range.start_addr,
                        size
                    }, range));
                }
            }
            None
        }

        let allocd_mem = split_usable_region(&mut self.mmap.entries.iter_mut(), size);

        if allocd_mem.is_some(){
            let (mem_chunk, range) = allocd_mem.unwrap();
            self.mmap.add_region(MemRegion {
                range,
                region_type
            }).unwrap();
            Some(mem_chunk)
        } else {
            None
        }
    }

}

/// A structure that tells the location of a memory
/// map of E820MemRegions and the number of regions in it
pub struct E820MemMapDescriptor {
    pub mmap_addr: Addr,
    pub mmap_entry_count: u64
}

impl From<E820MemMapDescriptor> for MemMap {
    fn from(e820_mmap_descr: E820MemMapDescriptor) -> MemMap {
        let E820MemMapDescriptor { mmap_addr, mmap_entry_count } = e820_mmap_descr;
        let mmap_start_ptr = mmap_addr.as_u64() as *const E820MemRegion;
        let e820_mmap = unsafe { slice::from_raw_parts(mmap_start_ptr, mmap_entry_count as usize) };
        let mut mmap = MemMap::new();
        for region in e820_mmap {
            if let Err(_) = mmap.add_region(MemRegion::from(*region)) {
                break;
            }
        }
        mmap.sort();
        mmap.remove_usable_region_overlaps();
        mmap
    }
}

/// A structure that tells the location of a memory
/// map of EFIMemRegions and the information needed to parse the map
pub struct EFIMemMapDescriptor {
    pub mmap_ptr: *const EFIMemRegion,
    pub mmap_size: usize,
    pub mmap_entry_size: usize
}

impl From<EFIMemMapDescriptor> for MemMap {
    fn from(mmap_descr: EFIMemMapDescriptor) -> MemMap {
        let efi_mmap_iter = EFIMemMapIter {
            start_ptr: mmap_descr.mmap_ptr as *const u8,
            len: mmap_descr.mmap_size / mmap_descr.mmap_entry_size,
            index: 0,
            entry_size: mmap_descr.mmap_entry_size as isize
        };
        let mut mmap = MemMap::new();
        for region in efi_mmap_iter {
            if let Err(_) = mmap.add_region(MemRegion::from(region.clone())) {
                break;
            }
        }
        mmap.sort();
        mmap.remove_usable_region_overlaps();
        mmap
    }
}

/// An iterator over the UEFI memory map regions
struct EFIMemMapIter {
    /// A pointer to the beginning of the map
    start_ptr: *const u8,
    /// The number of regions of size `descriptor_size` in the map
    len: usize,
    /// The current index of the iteration
    index: isize,
    /// The size of a single entry in the map
    ///
    /// Apparently, this `entry_size` can be bigger than EFIMemDescriptor,
    /// event though each entry is an EFIMemDescriptor
    entry_size: isize
}

impl Iterator for EFIMemMapIter {
    type Item = &'static EFIMemRegion;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index as usize >= self.len {
            None
        } else {
            let curr_ptr = unsafe {
                self.start_ptr.offset(self.index * self.entry_size) as *const EFIMemRegion
            };
            self.index += 1;
            unsafe { Some(&*curr_ptr) }
        }
    }
}

struct Hex<N: Integer>(N);
impl<N: Integer + fmt::Display> fmt::Debug for Hex<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:#}", self.0)
    }
}


