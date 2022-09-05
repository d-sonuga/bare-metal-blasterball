#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use physics::{Rectangle, Point, Object, Velocity};
use artist::{println, print};
use artist::bitmap::Bitmap;
use artist;


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap) -> ! {
    let mut n = 0;
    let mut rect = Rectangle {
        top_left: Point(0, 0),
        width: 200,
        height: 100
    };
    let mut draw = true;
    let rocket_bmp_bytes = include_bytes!("./ball.bmp");
    let rocket_bmp = Bitmap::from(rocket_bmp_bytes, artist::get_artist().lock())
        .expect("Failed to read the bitmap from the given source");
    let mut character = Character {
        object: Object {
            pos: Point(0, 0),
            velocity: Velocity { direction: 270, speed: 1 }
        },
        repr: rocket_bmp
    };
    artist::get_artist().lock().draw_bitmap(character.object.pos, &character.repr);
    event_hook::hook_event(event_hook::Event::Timer, event_hook::box_fn!(|_| {
        character.object.update_pos(1);
        let mut artist = artist::get_artist().lock();
        artist.erase_bitmap(character.object.pos, &character.repr);
        artist.draw_bitmap(character.object.pos, &character.repr);
    }));
    
    event_hook::hook_event(event_hook::Event::keyboard(), event_hook::box_fn!(|event| {
        if let event_hook::Event::Keyboard(keycode, direction, modifiers) = event {
            match keycode {
                KeyCode::ArrowUp => {
                    
                }
                _ => ()
            };
        }
    }));
    loop {}
}

struct Character {
    /// The physical definition of the character?
    object: Object,
    repr: Bitmap
}
