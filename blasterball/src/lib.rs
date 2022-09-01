#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use physics::{Rectangle, Point};
use printer::{println, print};
use printer;


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap) -> ! {
    let mut n = 0;
    let mut rect = Rectangle {
        top_left: Point(0, 0),
        width: 200,
        height: 100
    };
    event_hook::hook_event(event_hook::Event::Timer, event_hook::box_fn!(|_| {
        printer::get_artist().lock().draw_rectangle(&rect);
        n += 1;
        rect.top_left.0 = n;
    }));
    event_hook::hook_event(event_hook::Event::keyboard(), event_hook::box_fn!(|event| {
        print!("Keyboard");
    }));
    loop {}
}
