//! Abstractions for printing to the screen

#![no_std]

use core::fmt;
use core::fmt::Write;
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use physics::{Rectangle, Point};
use collections::vec::Vec;
use collections::vec;
use machine::port::Port;
use num::Num;

pub mod font;
pub mod bitmap;

use bitmap::Bitmap;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        x_pos: 0,
        y_pos: 0,
        color_code: ColorCode(Color(Color::Yellow), Color(Color::Black)),
        vga_buffer: unsafe { &mut *(0xa0000 as *mut VGABuffer) }
    });
}

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
        WRITER.lock().write_fmt(args).unwrap();
    })
}

pub fn get_artist() -> &'static Mutex<Writer> {
    &WRITER
}

pub fn clear_screen() {
    use machine::instructions::interrupts;
    interrupts::without_interrupts(||{
        WRITER.lock().clear_screen();
    });
}

/// A color that can be put in a pixel
#[derive(Copy, Clone, PartialEq, Debug, Eq)]
#[repr(transparent)]
pub struct Color(u8);

impl Color {
    pub const Black: u8       = 0x0;
    pub const Blue: u8        = 0x1;
    pub const Green: u8       = 0x2;
    pub const Cyan: u8        = 0x3;
    pub const Red: u8         = 0x4;
    pub const Magenta: u8     = 0x5;
    pub const Brown: u8       = 0x6;
    pub const LightGray: u8   = 0x7;
    pub const DarkGray: u8    = 0x8;
    pub const LightBlue: u8   = 0x9;
    pub const LightGreen: u8  = 0xa;
    pub const LightCyan: u8   = 0xb;
    pub const LightRed: u8    = 0xc;
    pub const Pink: u8        = 0xd;
    pub const Yellow: u8      = 0xe;
    pub const White: u8       = 0xf;

    fn new(color: u8) -> Self {
        Self(color)
    }
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

/// Writes to the VGA buffer
pub struct Writer {
    x_pos: usize,
    y_pos: usize,
    color_code: ColorCode,
    vga_buffer: &'static mut VGABuffer
}

impl Writer {

    /// Writes a byte to the VGA buffer
    fn write_byte(&mut self, c: u8) {
        if c == b'\n' {
            self.newline();
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                for x in 0..8 {
                    if byte & (1 << (8 - x - 1)) == 0 {
                        self.vga_buffer.pixels[self.y_pos + y][self.x_pos + x] = self.color_code.background();
                    } else {
                        self.vga_buffer.pixels[self.y_pos + y][self.x_pos + x] = self.color_code.foreground();
                    }
                }
            }
            self.x_pos += 8;
            if self.x_pos >= SCREEN_WIDTH {
                self.newline();
                self.x_pos = 0;
            }
        } else {
            self.write_byte(b'?');
        }
    }

    fn write_string(&mut self, s: &str) {
        for c in s.bytes() {
            self.write_byte(c);
        }
    }

    fn printint<T: Num>(&mut self, n: T) {
        fn inner_printint<T: Num>(w: &mut Writer, n: T) {
            if n.to_u8() < 10 {
                w.write_byte(n.to_u8() + 48);
            } else {
                let n = n.to_u64();
                let q = n / 10;
                let r = n % 10;
                inner_printint(w, q);
                w.write_byte(r.to_u8() + 48u8);
            }
        }
        inner_printint(self, n);
        self.newline();
    }

    /// Prints a newline in the VGA buffer
    fn newline(&mut self) {
        self.y_pos += 8;
        self.x_pos = 0;
    }

    /// Deletes all characters on a row of the VGA buffer
    fn clear_row(&mut self, row: usize) {
    }

    /// Clears the screen
    fn clear_screen(&mut self) {
        /*
        for row in 0..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                self.vga_buffer.pixels[row][col] = self.color_code.background();
            }
        }
        */
        self.x_pos = 0;
        self.y_pos = 0;
    }

    pub fn draw_rectangle(&mut self, rect: &Rectangle) {
        // top_left -> top_right
        // top_right -> bottom_right
        // bottom_right -> bottom_left
        // bottom_left -> top_left
        self.x_pos = rect.top_left.x();
        self.y_pos = rect.top_left.y();
        for i in 0..rect.width {
            self.vga_buffer.pixels[self.y_pos][self.x_pos + i] = self.color_code.foreground();
            self.vga_buffer.pixels[self.y_pos + rect.height][self.x_pos + i] = self.color_code.foreground();
        }
        for i in 0..rect.height {
            self.vga_buffer.pixels[self.y_pos + i][self.x_pos] = self.color_code.foreground();
            self.vga_buffer.pixels[self.y_pos + i][self.x_pos + rect.width] = self.color_code.foreground();
        }
    }

    pub fn erase_rectangle(&mut self, rect: &Rectangle) {
        // top_left -> top_right
        // top_right -> bottom_right
        // bottom_right -> bottom_left
        // bottom_left -> top_left
        self.x_pos = rect.top_left.x();
        self.y_pos = rect.top_left.y();
        for i in 0..rect.width {
            self.vga_buffer.pixels[self.y_pos][self.x_pos + i] = self.color_code.background();
            self.vga_buffer.pixels[self.y_pos + rect.height][self.x_pos + i] = self.color_code.background();
        }
        for i in 0..rect.height {
            self.vga_buffer.pixels[self.y_pos + i][self.x_pos] = self.color_code.background();
            self.vga_buffer.pixels[self.y_pos + i][self.x_pos + rect.width] = self.color_code.background();
        }
    }

    pub fn draw_bitmap(&mut self, pos: Point, bitmap: &Bitmap) {
        self.y_pos = pos.y();
        self.x_pos = pos.x();
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                let pixel_array_y = bitmap.height() - y - 1;
                self.vga_buffer.pixels[self.y_pos + y][self.x_pos + x] = 
                    Color::new(bitmap.image_data[pixel_array_y*bitmap.width()+x]);
            }
        }
    }

    pub fn erase_bitmap(&mut self, pos: Point, bitmap: &Bitmap) {
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                self.vga_buffer.pixels[pos.y() + y][pos.x() + x] = Color(Color::Black);
            }
        }
    }
}

fn wait_for_retrace() {
    const INPUT_STATUS: u16 = 0x03da;
    const VRETRACE: u8 = 0x08;
    let input_status_port = Port::new(INPUT_STATUS);
    while input_status_port.read() & VRETRACE != 0 {}
    while input_status_port.read() & VRETRACE == 0 {}
}

pub fn is_printable_ascii(c: u8) -> bool {
    match c {
        b' '..=b'~' => true,
        _ => false
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}