#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use printer::println;


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap, event_hooker: EventHooker) -> ! {
    println!("Greetings from blasterball");
    event_hook::hook_event(event_hook::Event::Timer, ||{
        println!("-");
    });
    loop {}
}
