use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt; use machine::power::FRAMEBUFFER;
use::core::fmt::Write;
const FONT_WIDTH: usize = 8;
const FONT_HEIGHT: usize = 8;
const X_SCALE: usize = 2;
const Y_SCALE: usize = 2;
const SCREEN_HEIGHT: usize = 480;
const SCREEN_WIDTH: usize = 640;
#[repr(C)]
struct Color {
    blue: u8,
    green: u8,
    red: u8,
    _r: u8
}

/*
#[repr(transparent)]
struct Color(u8);
*/

static X_POS: AtomicUsize = AtomicUsize::new(0);
static Y_POS: AtomicUsize = AtomicUsize::new(0);
use crate::font;

// Can only be used after setting up the graphics mode
// and initializing the framebuffer
pub struct Printer;
impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            Printer.print_char(c);
        }
        Ok(())
    }
}
use machine::pic8259::is_printable_ascii;
// Quick and dirty printing
impl Printer {
    pub fn print_char(&mut self, c: u8) {
        let framebuffer = FRAMEBUFFER.get().unwrap().as_mut_ptr();
        //let framebuffer = 0xa0000;
        let mut vga = framebuffer as *mut Color;
        let curr_x = X_POS.load(Ordering::Relaxed);
        let curr_y = Y_POS.load(Ordering::Relaxed);
        if c == b'\n' {
            X_POS.store(0, Ordering::Relaxed);
            let old_y = Y_POS.load(Ordering::Relaxed);
            Y_POS.store(old_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                let i = y + 1;
                for yp in y * Y_SCALE..i*Y_SCALE {
                    for x in 0..FONT_WIDTH {
                        let j = x + 1;
                        for xp in x * X_SCALE..j * X_SCALE {
                            unsafe {
                                if byte & (1 << (FONT_WIDTH - x - 1)) == 0 {
                                    //*vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color(0);
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color { blue: 0, green: 255, red: 255, _r: 0};
                                } else {
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color { blue: 0, green: 0, red: 0, _r: 0};
                                    //*vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color(0xf);
                                }
                            }
                        }
                    }
                }
            }
            X_POS.store(curr_x + FONT_WIDTH * X_SCALE, Ordering::Relaxed);
            if X_POS.load(Ordering::Relaxed) >= SCREEN_WIDTH {
                X_POS.store(0, Ordering::Relaxed);
                Y_POS.store(curr_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
            }
        } else {
            self.print_char(b'?');
        }
    }
}