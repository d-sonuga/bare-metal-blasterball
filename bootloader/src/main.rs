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
use core::fmt::Write;


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
    //#[cfg(feature = "bios")]
    use uefi::Printer;
    writeln!(Printer, "pregdt");
    //loop {}
    gdt::init();
    // The allocator must be initialized before the interrupts
    // because the event_hooks which handle interrupts make use of
    // the allocator
    writeln!(Printer, "prealloc");
    allocator::init(heap_mem);
    //#[cfg(feature = "bios")]
    writeln!(Printer, "preinterrupt");
    //loop {}
    interrupts::init();
/*
    use uefi::Printer;
    use core::fmt::Write;
    unsafe {
        use machine::acpi;
        use machine::acpi::SDTTable;
        let rsdp = acpi::detect_rsdp().unwrap();
        let rsdt = &*rsdp.rsdt_ptr();
        let madt = rsdt.find_madt().unwrap();
        assert!(madt.is_valid(), "MADT is invalid");
        writeln!(Printer, "Found the MADT");
        writeln!(Printer, "{:?}", madt.flags().pc_at_compatible());

        use machine::apic;
        apic::setup_apic(madt);
        machine::instructions::interrupts::enable();
    }

    writeln!(Printer, "Im here");*/
//    unsafe { asm!("int3") };
  //  loop {}
    //let x = usize::MAX as *mut u8;
    //unsafe { asm!("int3") };
    //loop {}
    //use crate::uefi::Printer;
    //writeln!(Printer, "here1");
    //loop {}
    blasterball::game_entry_point();
    loop {}
}
