//! Abstractions for printing to the screen

#![no_std]
#![allow(dead_code)]

use core::fmt;
use core::fmt::Write;
use core::ops::{Index, IndexMut};
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use sync::once::Once;
use physics::Point;
use machine::memory::Addr;
use num::Integer;

pub mod font;
pub mod bitmap;

mod color;
pub use color::{Color, Hue};

use bitmap::{ScaledBitmap, Transparency};

#[cfg(feature = "bios")]
pub const SCREEN_WIDTH: usize = 320;
#[cfg(feature = "bios")]
pub const SCREEN_HEIGHT: usize = 200;
#[cfg(not(feature = "bios"))]
pub const SCREEN_WIDTH: usize = 640;
#[cfg(not(feature = "bios"))]
pub const SCREEN_HEIGHT: usize = 480;

/// Factor by which bitmaps should be scaled horizontally to fit the screen
pub const X_SCALE: usize = SCREEN_WIDTH / 320;
/// Factor by which bitmaps should be scaled vertically to fit the screen
pub const Y_SCALE: usize = SCREEN_HEIGHT / 200;

/// Height of the letters and numbers in the font module
pub const FONT_HEIGHT: usize = 8;
/// Width of the letters and numbers in the font module
pub const FONT_WIDTH: usize = 8;

pub const DOUBLE_BUFFER_SIZE: usize = SCREEN_HEIGHT * SCREEN_WIDTH;
pub static SCREEN_BUFFER_ADDRESS: Once<Addr> = Once::new();

lazy_static! {
    pub static ref ARTIST: Mutex<Artist> = Mutex::new(Artist {
        x_pos: 0,
        y_pos: 0,
        color_code: ColorCode(Color::new(Color::YELLOW), Color::new(Color::BLACK)),
        vga_buffer: {
            let screen_buffer_addr = SCREEN_BUFFER_ADDRESS.get()
                .expect("The screen buffer is not initialized");
            unsafe { &mut *(screen_buffer_addr.as_mut_ptr() as *mut VGABuffer) }
        },
        double_buffer: VGABuffer {
            pixels: [[Color::new(Color::BLACK); SCREEN_WIDTH]; SCREEN_HEIGHT]
        }
    });
}

unsafe impl Send for Artist {}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}


pub fn _print(args: fmt::Arguments) {
    use machine::instructions::interrupts;
    interrupts::without_interrupts(||{
        ARTIST.lock().write_fmt(args).unwrap();
    })
}

pub fn get_artist() -> &'static Mutex<Artist> {
    &ARTIST
}

/// A foreground/background color code for printing characters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColorCode(Color, Color);

impl ColorCode {
    /// Returns the background color of the color code
    fn background(&self) -> Color {
        self.1
    }
    /// Returns the foreground color of the color code
    fn foreground(&self) -> Color {
        self.0
    }
}

/// The VGA buffer to be written to for screen printing
#[repr(transparent)]
struct VGABuffer {
    pixels: [[Color; SCREEN_WIDTH]; SCREEN_HEIGHT]
}

impl Index<usize> for VGABuffer {
    type Output = [Color; SCREEN_WIDTH];
    fn index(&self, idx: usize) -> &[Color; SCREEN_WIDTH] {
        &self.pixels[idx]
    }
}

impl IndexMut<usize> for VGABuffer {
    fn index_mut(&mut self, idx: usize) -> &mut [Color; SCREEN_WIDTH] {
        &mut self.pixels[idx]
    }
}

/// Draws to the VGA buffer
pub struct Artist {
    x_pos: usize,
    y_pos: usize,
    color_code: ColorCode,
    vga_buffer: &'static mut VGABuffer,
    double_buffer: VGABuffer,
    //move_bitmap_in_double_buffer_request_queue: Queue<'static, MoveBitmapInDoubleBufferRequest>
}

impl Artist {

    /// Writes a byte to the VGA buffer
    pub fn write_byte(&mut self, c: u8, write_target: WriteTarget) {
        if c == b'\n' {
            self.newline();
        } else if is_printable_ascii(c) {
            let buffer = match write_target {
                WriteTarget::VGABuffer => &mut self.vga_buffer,
                WriteTarget::DoubleBuffer => &mut self.double_buffer
            };
            for (y, byte) in font::FONT[c].iter().enumerate() {
                let i = y + 1;
                for yp in y * Y_SCALE..i*Y_SCALE {
                    for x in 0..FONT_WIDTH {
                        let j = x + 1;
                        for xp in x * X_SCALE..j * X_SCALE {
                            if byte & (1 << (FONT_WIDTH - x - 1)) == 0 {
                                buffer[self.y_pos + yp][self.x_pos + xp] = self.color_code.background();
                            } else {
                                buffer[self.y_pos + yp][self.x_pos + xp] = self.color_code.foreground();
                            }
                        }
                    }
                }
            }
            self.x_pos += FONT_WIDTH * X_SCALE;
            if self.x_pos >= SCREEN_WIDTH {
                self.newline();
                self.x_pos = 0;
            }
            if self.y_pos >= SCREEN_HEIGHT - FONT_HEIGHT * Y_SCALE {
                self.y_pos = 0;
            }
        } else {
            self.write_byte(b'?', write_target);
        }
    }

    fn write_string(&mut self, s: &str, write_target: WriteTarget) {
        for c in s.bytes() {
            self.write_byte(c, write_target);
        }
    }

    pub fn write_string_in_double_buffer(&mut self, s: &str) {
        self.write_string(s, WriteTarget::DoubleBuffer);
    }

    fn printint<T: Integer>(&mut self, n: T) {
        fn inner_printint<T: Integer>(w: &mut Artist, n: T) {
            if n.as_u8() < 10 {
                w.write_byte(n.as_u8() + 48, WriteTarget::VGABuffer);
            } else {
                let n = n.as_u64();
                let q = n / 10;
                let r = n % 10;
                inner_printint(w, q);
                w.write_byte(r.as_u8() + 48u8, WriteTarget::VGABuffer);
            }
        }
        inner_printint(self, n);
        self.newline();
    }

    pub fn reset_writing_pos(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;
    }

    /// Prints a newline in the VGA buffer
    pub fn newline(&mut self) {
        self.y_pos += FONT_HEIGHT * Y_SCALE;
        self.x_pos = 0;
    }
    
    pub fn draw_background_in_double_buffer(&mut self, color: &Color) {
        // Rust was too slow for this.
        // Had to use assembly
        use core::arch::asm;
        #[cfg(feature = "bios")]
        // Divided by 4 because stosd stores 4 bytes at a time
        // and a color is 1 byte in BIOS's VGA 320x200 mode
        let no_of_movements = DOUBLE_BUFFER_SIZE / 4; 
        #[cfg(not(feature = "bios"))]
        // A color is 4 bytes because of the UEFI setup and 4 bytes
        // are moved at a time, so this has to be the full buffer
        let no_of_movements = DOUBLE_BUFFER_SIZE;
        unsafe {
            asm!("
                # Move the value in eax into edi, ecx times
                rep stosd",
                in("eax") color.to_num(),
                in("edi") self.double_buffer.pixels.as_slice().as_ptr(),
                in("ecx") no_of_movements 
            );
        }
    }

    pub fn move_scaled_bitmap_in_double_buffer(&mut self, bitmap: &ScaledBitmap, old_pos: Point, new_pos: Point, background: &Color) {
        self.erase_scaled_bitmap_from_double_buffer(bitmap, old_pos, background);
        self.draw_scaled_bitmap_in_double_buffer(new_pos, bitmap);
    }

    pub fn draw_scaled_bitmap_in_double_buffer(&mut self, pos: Point, bitmap: &ScaledBitmap) {
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                if pos_is_within_screen_bounds(pos, x, y) {
                    let pixel_array_y = bitmap.height() - y - 1;
                    let color = bitmap.image_data[pixel_array_y*bitmap.width()+x];
                    if bitmap.transparency == Transparency::Black && color == Color::BLACK {
                        continue;
                    }
                    self.double_buffer[pos.y().as_usize() + y][pos.x().as_usize() + x] = color;
                }
            }
        }
    }

    pub fn erase_scaled_bitmap_from_double_buffer(&mut self, bitmap: &ScaledBitmap, pos: Point, background: &Color) {
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                if pos_is_within_screen_bounds(pos, x, y) {
                    let pixel_array_y = bitmap.height() - y - 1;
                    let color = bitmap.image_data[pixel_array_y*bitmap.width()+x];
                    if bitmap.transparency == Transparency::Black && color == Color::BLACK {
                        continue;
                    }
                    self.double_buffer[pos.y().as_usize() + y][pos.x().as_usize() + x] = *background;
                }
            }
        }
    }

    pub fn draw_on_screen_from_double_buffer(&mut self) {
        
        unsafe {
            use core::arch::asm;
            asm!("
                # Move 4 bytes at a time from esi to edi, ecx times
                rep movsd",
                in("esi") self.double_buffer.pixels.as_slice().as_ptr(),
                in("edi") self.vga_buffer.pixels.as_slice().as_ptr(),
                in("ecx") DOUBLE_BUFFER_SIZE
            );
        }
    }
}

pub fn is_printable_ascii(c: u8) -> bool {
    match c {
        b' '..=b'~' => true,
        _ => false
    }
}

#[inline]
pub fn pos_is_within_screen_bounds(pos: Point, dx: usize, dy: usize) -> bool {
    pos.y() >= 0 && pos.x() >= 0 
        && pos.y().as_usize() + dy < SCREEN_HEIGHT
        && pos.x().as_usize() + dx < SCREEN_WIDTH
}

impl fmt::Write for Artist {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s, WriteTarget::VGABuffer);
        Ok(())
    }
}

/// Tells the artist where to write text to
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WriteTarget {
    VGABuffer,
    DoubleBuffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pos_is_within_screen_bounds() {
        let pos = Point(0, 0);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(is_within_bounds);

        let pos = Point(320, 200);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(!is_within_bounds);

        let pos = Point(200, 220);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(!is_within_bounds);

        let pos = Point(321, 200);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(!is_within_bounds);

        let pos = Point(322, 221);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(!is_within_bounds);

        let pos = Point(-1, 12);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 2, 43);
        assert!(!is_within_bounds);

        let pos = Point(100, 201);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(!is_within_bounds);

        let pos = Point(319, 199);
        let is_within_bounds = pos_is_within_screen_bounds(pos, 0, 0);
        assert!(is_within_bounds);
    }
}

