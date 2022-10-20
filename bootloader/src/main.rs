#![no_main]
#![no_std]
#![feature(abi_x86_interrupt, abi_efiapi)]
#![allow(unaligned_references)]


#[cfg(feature="bios")]
mod bios;

#[cfg(not(feature="bios"))]
mod uefi;

mod interrupts;

mod gdt;

mod artist_init;

use core::arch::asm;
use machine::memory::MemChunk;
use collections::allocator;
use sound;
use blasterball;


macro_rules! Mem {
    // $n megabytes
    ($n:expr, Mib) => { $n * 2u64.pow(20) };
    ($n:expr, Kib) => { $n * 2u64.pow(10) };
}

const APP_STACK_SIZE: u64 = Mem!(10, Mib);

const APP_HEAP_SIZE: u64 = Mem!(10, Mib);

fn setup_memory_and_run_game(stack_mem: MemChunk, heap_mem: MemChunk) -> ! {
    
    // Changing the stack pointer
    // Need to save heap_mem so it can be used later
    unsafe {
        asm!("
            mov rdi, {}
            mov rsp, {}",
            in(reg) &heap_mem as *const _ as u64,
            //in(reg) &mmap as *const _ as u64,
            in(reg) stack_mem.range().end_addr.as_u64() - 1,
        );
    }
    
    let heap_mem_addr: u64;
    //let mmap: u64;
    unsafe { 
        asm!("
            mov {}, rdi
            ",
            out(reg) heap_mem_addr,
            //out(reg) mmap
        );
    }
    let heap_mem = unsafe { *(heap_mem_addr as *const MemChunk) };
    //let mmap = unsafe { &*(mmap as *const MemMap) };
    // It's important that the GDT is initialized before the interrupts
    // The interrupts make use of the GDT
    gdt::init();
    // The allocator must be initialized before the interrupts
    // because the interrupts use the event hooker which in turn uses
    // the allocator
    allocator::init(heap_mem);
    interrupts::init();
    event_hook::init();
    sound::init().unwrap();

    blasterball::game_entry_point();
}
