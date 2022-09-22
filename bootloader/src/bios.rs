use core::arch::{global_asm, asm};
use core;

global_asm!(include_str!("asm/stage_1.s"));
global_asm!(include_str!("asm/stage_2.s"));
global_asm!(include_str!("asm/stage_3.s"));

use crate::gdt;
use crate::interrupts;
use crate::setup_memory_and_run_game;

use core::sync::atomic::{Ordering};
use core::slice;
use machine::memory::{Addr, MemRegion, MemRegionType, AddrRange, MemAllocator, MemMap, E820MemMapDescriptor};
use machine::memory;
use artist::{println, clear_screen};
use collections::allocator;
use blasterball;

macro_rules! addr_to_mut_ref {
    ($addr:ident) => {
        &mut *($addr as *const PageTable as *mut PageTable)
    }
}

const VGA_BUFFER_ADDR: Addr = Addr::new(0xa0000);

#[no_mangle]
pub extern "C" fn main() -> ! {
    unsafe {
        asm!("
            push rbx
            mov rbx, 0
            mov ss, rbx
            pop rbx
        ");
    }
    let mmap_addr: u64;
    let mmap_entry_count: u64;
    let app_start: u64;
    let app_end: u64;
    let page_table_start: u64;
    let page_table_end: u64;
    unsafe {
        asm!("
            mov {}, mmap_entry_count
            mov {}, offset _mmap
            mov {}, offset __app_start
            mov {}, offset __app_end
            mov {}, offset __page_table_start
            mov {}, offset __page_table_end",
            out(reg) mmap_entry_count,
            out(reg) mmap_addr,
            out(reg) app_start,
            out(reg) app_end,
            out(reg) page_table_start,
            out(reg) page_table_end
        );
    }
    let mmap_entry_count = mmap_entry_count & 0xff;         // Only lower byte needed
    if mmap_entry_count == 0 {
        panic!("No memory regions found");
    }
    
    let e820_mmap_descr = E820MemMapDescriptor {
        mmap_addr: Addr::new(mmap_addr),
        mmap_entry_count
    };
    let mut mmap = MemMap::from(e820_mmap_descr);

    let mut mem_allocator = MemAllocator::new(&mut mmap);

    let app_start_addr = Addr::new(app_start);
    let app_end_addr = Addr::new(app_end);
    let app_region_range = AddrRange::new(app_start_addr.as_u64(), app_end_addr.as_u64() + 1);
    mem_allocator.mark_alloc_region(MemRegion {
        range: app_region_range,
        region_type: MemRegionType::App
    });

    let page_table_start_addr = Addr::new(page_table_start);
    let page_table_end_addr = Addr::new(page_table_end);
    let page_table_region_range = AddrRange::new(page_table_start_addr.as_u64(), page_table_end_addr.as_u64() + 1);
    mem_allocator.mark_alloc_region(MemRegion {
        range: page_table_region_range,
        region_type: MemRegionType::PageTable
    });

    crate::artist_init::init(VGA_BUFFER_ADDR);

    setup_memory_and_run_game(mem_allocator);

    loop {}
}


#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // A function that allows for printing independently of the artist
    use artist::{is_printable_ascii, font, Color};
    impl PanicWriter {
        fn print_char(&mut self, c: u8) {
            let mut vga_buffer = VGA_BUFFER_ADDR.as_mut_ptr();
            if c == b'\n' {
                self.print_char(b' ');
            } else if is_printable_ascii(c) {
                for (y, byte) in font::FONT[c].iter().enumerate() {
                    for x in 0..8 {
                        let char_y = (y + self.y_pos) as isize;
                        let char_x = (x + self.x_pos) as isize;
                        unsafe {
                            if byte & (1 << (8 - x - 1)) == 0 {
                                *vga_buffer.offset(char_y*320+char_x) = Color::Black;
                            } else {
                                *vga_buffer.offset(char_y*320+char_x) = Color::Yellow;
                            }
                        }
                    }
                }
                self.x_pos += 8;
                if self.x_pos >= 320 {
                    self.y_pos += 8;
                    self.x_pos = 0;
                }
            } else {
                self.print_char(b'?');
            }
        }
    }
    struct PanicWriter { x_pos: usize, y_pos: usize }
    use core::fmt;
    use core::fmt::Write;
    impl fmt::Write for PanicWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for c in s.bytes() {
                self.print_char(c);
            }
            Ok(())
        }
    }
    //println!("Panicked: {}", _info);
    let mut panic_writer = PanicWriter { x_pos: 0, y_pos: 0 };
    panic_writer.write_str("Panicked: ");
    panic_writer.write_fmt(format_args!("{}", _info));
    loop {}
}