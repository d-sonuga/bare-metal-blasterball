#![no_main]
#![no_std]
#![feature(abi_x86_interrupt, abi_efiapi)]
#![allow(unaligned_references)]


#[cfg(feature="bios")]
mod bios;

#[cfg(not(feature="bios"))]
mod uefi;

//#[cfg(feature="bios")]
mod interrupts;

//#[cfg(feature="bios")]
mod gdt;

mod artist_init;

use core::arch::asm;
use machine::memory::{Addr, MemRegion, MemRegionType, AddrRange, MemAllocator, MemMap, MemChunk};
use machine::memory;
use artist::{println, clear_screen};
use collections::allocator;
use blasterball;


macro_rules! Mem {
    // $n megabytes
    ($n:expr, Mib) => { $n * 2u64.pow(20) };
    ($n:expr, Kib) => { $n * 2u64.pow(10) };
}

const APP_STACK_SIZE: u64 = Mem!(10, Mib);

const APP_HEAP_SIZE: u64 = Mem!(10, Mib);

fn setup_memory_and_run_game(mut mem_allocator: MemAllocator) -> ! {
    let stack_mem = mem_allocator.alloc_mem(MemRegionType::AppStack, APP_STACK_SIZE)
        .expect("Couldn't allocate memory for the stack");
    let heap_mem = mem_allocator.alloc_mem(MemRegionType::Heap, APP_HEAP_SIZE)
        .expect("Couldn't allocate memory for the heap");

    // Saving values on the stack in registers so they can be used later
    unsafe {
        asm!("
            mov rdi, {}
            mov rsp, {}",
            in(reg) &heap_mem as *const _ as u64,
            in(reg) stack_mem.range().end_addr.as_u64() - 1,
        );
    }
    
    let heap_mem_addr: u64;
    unsafe { 
        asm!("
            mov {}, rdi",
            out(reg) heap_mem_addr,
        );
    }
    let heap_mem = unsafe { *(heap_mem_addr as *const MemChunk) };
    // It's important that the GDT is initialized before the interrupts
    // The interrupts make use of the GDT
    #[cfg(feature = "bios")]
    gdt::init();
    // The allocator must be initialized before the interrupts
    // because the event_hooks which handle interrupts make use of
    // the allocator
    allocator::init(heap_mem);
    #[cfg(feature = "bios")]
    interrupts::init();
    //let x = usize::MAX as *mut u8;
    blasterball::game_entry_point();
    loop {}
}
