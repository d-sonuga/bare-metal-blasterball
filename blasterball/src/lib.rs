#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use printer::println;


#[no_mangle]
pub fn entry_point(mmap: MemMap) -> ! {
    println!("Greetings from blasterball");
    loop {}
}
