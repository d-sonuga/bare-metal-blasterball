//! Abstractions for dealing with memory

use core::ops::{Add, Sub, BitAnd, BitOr, Index, IndexMut, AddAssign};
use core::{slice, fmt};
use num::Num;

const MAX_MEM_MAP_SIZE: usize = 64;

/// A wrapper around a u64 to ensure it always remains a valid
/// virtual address, that is, the 49th bit upwards is sign extended
/// because only the lower 48 bits are used as a valid virtual address
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct Addr(u64);

impl Addr {
    #[inline]
    pub fn new(n: u64) -> Addr {
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

/*
    /// A wrapper around a u64 to ensure it always remains a valid
    /// physical address, that is, the higher 12 bits are zero, because
    /// only the lower 52 bits are used for the physical address on x86-64
    #[derive(Copy, Clone)]
    pub struct PAddr(u64);

    impl PAddr {
        pub fn new(n: u64) -> PAddr {
            // Upper 12 bits must be 0
            if n & 0xfff0000000000000 != 0 {
                panic!("Address too big to be a valid physical address");
            }
            PAddr(n)
        }

        fn new_trunc(n: u64) -> PAddr {
            PAddr::new(n & 0x000fffffffffffff)
        }

        pub fn as_u64(&self) -> u64 {
            self.0
        }
    }

    impl Add<u64> for PAddr {
        type Output = PAddr;

        fn add(self, rhs: u64) -> Self::Output {
            PAddr::new(self.0 + rhs)
        }
    }

    impl Sub for PAddr {
        type Output = PAddr;

        fn sub(self, rhs: PAddr) -> PAddr {
            PAddr::new(self.as_u64() - rhs.as_u64())
        }
    }

    impl Sub<u64> for PAddr {
        type Output = PAddr;

        fn sub(self, rhs: u64) -> PAddr {
            PAddr::new(self.as_u64() - rhs)
        }
    }

    impl BitAnd<u64> for PAddr {
        type Output = PAddr;

        fn bitand(self, rhs: u64) -> PAddr {
            PAddr::new(self.0 & rhs)
        }
    }
*/
/*
    #[derive(Copy, Clone)]
    struct Page {
        start_addr: VAddr
    }

    impl Page {
        fn containing_address(addr: VAddr) -> Page {
            let raw_page_start_addr = addr & !(PAGE_SIZE - 1);
            let start_addr = VAddr::new_trunc(raw_page_start_addr.as_u64());
            Page {
                start_addr
            }
        }
    }

    impl Add<u64> for Page {
        type Output = Self;

        fn add(self, rhs: u64) -> Page {
            Page::containing_address(self.start_addr + rhs * PAGE_SIZE)
        }
    }

    #[derive(Copy, Clone)]
    pub struct Frame {
        start_addr: PAddr
    }

    impl Frame {
        pub fn containing_address(addr: PAddr) -> Frame {
            let raw_frame_start_addr = addr & !(PAGE_SIZE - 1);
            let start_addr = PAddr::new_trunc(raw_frame_start_addr.as_u64());
            Frame {
                start_addr
            }
        }

        fn for_each(start_frame: Frame, end_frame: Frame) -> FrameIterator {
            FrameIterator {
                start_frame,
                end_frame,
                curr_index: 0
            }
        }
    }

    macro_rules! impl_frame_add_num {
        ($($num_type:ty)+) => {$(
            impl Add<$num_type> for Frame {
                type Output = Frame;

                fn add(self, rhs: $num_type) -> Frame {
                    Frame::containing_address(self.start_addr + rhs as u64 * PAGE_SIZE)
                }
            }
        )+}
    }

    impl_frame_add_num!(u64 usize);

    impl Sub<Frame> for Frame {
        type Output = u64;

        fn sub(self, rhs: Frame) -> u64 {
            (self.start_addr - rhs.start_addr).as_u64() / PAGE_SIZE
        }
    }

    impl Sub<u64> for Frame {
        type Output = Frame;

        fn sub(self, rhs: u64) -> Frame {
            Frame::containing_address(self.start_addr - rhs)
        }
    }

    struct FrameIterator {
        start_frame: Frame,
        end_frame: Frame,
        curr_index: usize
    }

    impl Iterator for FrameIterator {
        type Item = Frame;

        fn next(&mut self) -> Option<Self::Item> {
            let n = self.end_frame - self.start_frame;
            if self.curr_index > n as usize {
                None
            } else {
                self.curr_index += 1;
                Some(self.start_frame + self.curr_index - 1)
            }
        }
    }

    #[derive(Clone, Copy)]
    struct PageTableFlags(u64);

    impl PageTableFlags {
        const PRESENT: u64 = 0b1;
        const WRITABLE: u64 = 0b10;
        const NO_EXECUTE: u64 = 1 << 63;
        const HUGE_PAGE: u64 = 1 << 7;
        const NO_HUGE_PAGE: u64 = 0 << 7;

        fn new() -> PageTableFlags {
            PageTableFlags(0)
        }

        fn is_present(self, present: bool) -> PageTableFlags {
            if present {
                PageTableFlags(self.0 | PageTableFlags::PRESENT)
            } else {
                self
            }
        }

        fn can_exec(self, can_exec: bool) -> PageTableFlags {
            if can_exec {
                self
            } else {
                PageTableFlags(self.0 | PageTableFlags::NO_EXECUTE)
            }
        }

        fn can_write(self, writable: bool) -> PageTableFlags {
            if writable {
                PageTableFlags(self.0 | PageTableFlags::WRITABLE)
            } else {
                self
            }
        }
    }

    impl BitOr<u64> for PageTableFlags {
        type Output = PageTableFlags;

        fn bitor(self, rhs: u64) -> PageTableFlags {
            PageTableFlags(self.0 | rhs)
        }
    }

    impl BitOr<PageTableFlags> for u64 {
        type Output = u64;

        fn bitor(self, rhs: PageTableFlags) -> u64 {
            self | rhs.0
        }
    }

    /// A wrapper around u16 to hold a value that will always be a valid 9 bit page table index
    struct PageTableIndex(u16);

    impl PageTableIndex {
        fn new(n: u64) -> PageTableIndex {
            // Can only be 9 bits
            if n >> 9 != 0 {
                panic!("Value too big to be a page table index");
            }
            PageTableIndex(n as u16)
        }
    }


    #[repr(transparent)]
    struct PageTableEntry(u64);

    impl PageTableEntry {
        fn new(addr: PAddr) -> PageTableEntry {
            PageTableEntry(addr.0)
        }
        
        /// A builder method to set flags on a page table entry
        fn flags(&self, flags: PageTableFlags) -> PageTableEntry {
            PageTableEntry(self.0 | flags)
        }
    }

    #[repr(C)]
    #[repr(align(4096))]
    pub struct PageTable {
        entries: [PageTableEntry; PAGE_SIZE as usize]
    }

    impl Index<PageTableIndex> for PageTable {
        type Output = PageTableEntry;

        fn index(&self, index: PageTableIndex) -> &PageTableEntry {
            &self.entries[index.0 as usize]
        }
    }

    impl IndexMut<PageTableIndex> for PageTable {
        fn index_mut(&mut self, index: PageTableIndex) -> &mut PageTableEntry {
            &mut self.entries[index.0 as usize]
        }
    }
*/
#[repr(C)]
pub struct MemMap {
    pub entries: [MemRegion; MAX_MEM_MAP_SIZE],
    next_entry_index: u64
}

impl MemMap {
    #[inline]
    pub fn new() -> MemMap {
        MemMap {
            entries: [MemRegion::empty(); MAX_MEM_MAP_SIZE],
            next_entry_index: 0
        }
    }

    #[inline]
    pub fn add_region(&mut self, region: MemRegion){
        if self.next_entry_index > MAX_MEM_MAP_SIZE as u64 {
            panic!("Too many regions in mem map");
        }
        self.entries[self.next_entry_index as usize] = region;
        self.next_entry_index += 1;
        self.sort();
    }

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
}

impl fmt::Debug for MemMap {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("MemMap {\n\t");
        f.write_str("entries: ");
        f.debug_list().entries(self.entries.iter()).finish();
        f.write_str("\n\t");
        f.write_str("next_entry_index: ");
        f.write_fmt(format_args!("{:?}", self.next_entry_index));
        f.write_str("\n}");
        Ok(())
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MemRegion {
    pub range: AddrRange,
    pub region_type: MemRegionType
}

impl MemRegion {
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

/// A range of the form [start_number, end_number), that is, end_number is not included in the range
#[derive(Copy, Clone)]
#[repr(C)]
pub struct AddrRange {
    pub start_addr: Addr,
    pub end_addr: Addr
}

impl AddrRange {
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



#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MemRegionType {
    Usable,
    InUse,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMem,
    App,
    AppStack,
    PageTable,
    Bootloader,
    FrameZero,
    Empty,
    BootInfo,
    Package
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

/// A chunk of allocated memory
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MemChunk {
    pub start_addr: Addr,
    pub size: u64
}

impl MemChunk {
    #[inline]
    pub fn range(&self) -> AddrRange {
        AddrRange {
            start_addr: self.start_addr,
            end_addr: self.start_addr + self.size
        }
    }
}

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
                            self.mmap.add_region(region);
                        } else {
                            *r = region;
                        }
                    } else if region.range.start_addr > r.range.start_addr {
                        if region.range.end_addr < r.range.end_addr {
                            let mut left_r = r.clone();
                            left_r.range.end_addr = region.range.start_addr;
                            r.range.start_addr = region.range.end_addr;
                            self.mmap.add_region(left_r);
                            self.mmap.add_region(region);
                        } else {
                            r.range.end_addr = region.range.start_addr;
                            self.mmap.add_region(region);
                        }
                    } else {
                        r.range.start_addr = region.range.end_addr;
                        self.mmap.add_region(region);
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
            });
            Some(mem_chunk)
        } else {
            None
        }
    }

}

pub fn create_mmap(mmap_addr: Addr, mmap_entry_count: u64) -> MemMap {
    let mmap_start_ptr = mmap_addr.as_u64() as *const E820MemRegion;
    let e820_mmap = unsafe { slice::from_raw_parts(mmap_start_ptr, mmap_entry_count as usize) };
    let mut mmap = MemMap::new();
    for region in e820_mmap {
        mmap.add_region(MemRegion::from(*region));
    }
    mmap.sort();
    let mut mmap_iter = mmap.entries.iter_mut().peekable();
    while let Some(region) = mmap_iter.next(){
        if let Some(next) = mmap_iter.peek(){
            if (region.range.end_addr > next.range.start_addr
                && region.region_type == MemRegionType::Usable) {
                region.range.end_addr = next.range.start_addr;
            }
        }
    }
    mmap
}

// Maps program segments as described by the program headers.
/*
    pub fn map_segments(app_load_addr: PAddr, app: &ElfFile){
        for segment in app.prog_header_iter() {
            match segment.p_type.get_type() {
                ProgHeaderTypeName::Load => {
                    map_segment(&segment, app_load_addr);
                },
                ProgHeaderTypeName::Other => ()
            }
        }
        let stack_start = stack_start + 1;
        let stack_end = stack_start + stack_size;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let region_type = MemRegionType::AppStack;
        for page in Page::range(stack_start, stack_end) {
            let frame = frame_allocator.allocate_frame()
                .expect("Frame allocation failed");
            map_page(page, frame, flags);
        }
    }
    */
    /*
    fn map_segment(segment: &ProgHeader, app_load_addr: PAddr){
        let mem_size = segment.mem_size;
        let file_size = segment.file_size;
        let virt_addr = VAddr::new(segment.virtual_addr);
        let phys_addr = app_load_addr + segment.offset;
        let start_page = Page::containing_address(virt_addr);
        let start_frame = Frame::containing_address(phys_addr);
        let end_frame = Frame::containing_address(phys_addr + file_size - 1);
        let mut flags = PageTableFlags::new()
            .is_present(true)
            .can_exec(segment.flags.can_exec())
            .can_write(segment.flags.can_write());
        for frame in Frame::for_each(start_frame, end_frame){
            let offset = frame - start_frame;
            let page = start_page + offset;
            map_page(page, frame, flags);
        }
    }

    fn map_page(page: Page, frame: Frame, flags: PageTableFlags){
        let pdpt = create_page_table(&mut pml4t[page.pml4t_index()], flags, allocator);
        let pdt = create_page_table(&mut pdpt[page.pdpt_index()], flags, allocator);
        let pt = create_page_table(&mut pdt[page.pdt_index()], flags, allocator);
        if !pt[page.pt_index()].is_unused() {
            panic!("Attempt to map a used page");
        }
        pt[page.pt_index()].set_frame(frame, flags);
    }

    fn create_page_table(
        higher_entry: &mut PageTableEntry,
        flags: PageTableFlags,
        allocator: FrameAllocator
    ) -> &mut PageTable {
        let mut created_table = false;
        if entry.is_unused(){
            let frame = allocator.allocate_frame().expect("Frame allocation failed");
            created_table = true;
            higher_entry.set_frame(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | flags);
        } else {
            higher_entry.set_flags(higher_entry.flags() | flags);
        }
        if higher_entry.flags().contains(PageTableFlags::HUGE_PAGE){
            panic!("Attempt to map in a 2Mib page");
        }
        let page_table_ptr = higher_entry.addr().as_mut_ptr();
        let page_table: &mut PageTable = unsafe { &mut *page_table_ptr };
        if created_table {
            page_table.zero_out();
        }
        page_table
    }
*/