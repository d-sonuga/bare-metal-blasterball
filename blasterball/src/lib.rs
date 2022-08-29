#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use printer::{println, print};


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap) -> ! {
    println!("Greetings from blasterball");
    event_hook::hook_event(event_hook::Event::Timer, |_| {
        print!("-");
    });
    event_hook::hook_event(event_hook::Event::keyboard(), |event| {
        print!("Keyboard");
    });
    loop {}
}
