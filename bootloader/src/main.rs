#![no_main]
#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

use core::arch::{global_asm, asm};
use core;

global_asm!(include_str!("asm/stage_1.s"));
global_asm!(include_str!("asm/stage_2.s"));
global_asm!(include_str!("asm/stage_3.s"));

mod interrupts;
mod gdt;
mod allocator;
extern crate alloc;

use core::sync::atomic::{Ordering};
use core::slice;
use machine::memory::{Addr, MemRegion, MemRegionType, AddrRange, MemAllocator};
use machine::memory;
use printer::{println, clear_screen};
use blasterball;

macro_rules! addr_to_mut_ref {
    ($addr:ident) => {
        &mut *($addr as *const PageTable as *mut PageTable)
    }
}

// 2 Mib
const APP_STACK_SIZE: u64 = 2u64.pow(20);

// 100 Kib
const APP_HEAP_SIZE: u64 = 100 * 2u64.pow(10);

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
    let long_mode_switch_success_msg = "Successfully switched to long mode";
    clear_screen();
    println!("{}", long_mode_switch_success_msg);
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

    let mut mmap = memory::create_mmap(Addr::new(mmap_addr), mmap_entry_count);
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

    let stack_mem = mem_allocator.alloc_mem(MemRegionType::AppStack, APP_STACK_SIZE)
        .expect("Couldn't allocate memory for the stack");
    let heap_mem = mem_allocator.alloc_mem(MemRegionType::Heap, APP_HEAP_SIZE)
        .expect("Couldn't allocate memory for the heap");
    
    let mmap_addr = Addr::new(&mmap as *const _ as u64);
    // It's important that the GDT is initialized before the interrupts
    unsafe {
        asm!("mov rsp, {}",
            in(reg) stack_mem.range().end_addr.as_u64() - 1,
        );
    }
    gdt::init();
    interrupts::init();
    use collections::allocator;
    allocator::init(heap_mem);
    
    unsafe {
        asm!(
    //        "mov rsp, {}
            "call {}",
            //in(reg) stack_mem.range().end_addr.as_u64() - 1,
            in(reg) blasterball::entry_point,
            in("rdi") mmap_addr.as_u64()
        )
    };
    loop {}
    //blasterball::entry_point(mmap);
}

fn v(){}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("Panicked: {}", _info);
    loop {}
}


#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Attempt to allocate: {:?}", layout);
}
