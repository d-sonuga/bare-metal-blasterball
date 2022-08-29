//! Abstractions for printing to the screen

#![no_std]

use core::fmt;
use core::fmt::Write;
use lazy_static::lazy_static;
use sync::mutex::Mutex;

mod font;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        x_pos: 0,
        y_pos: 0,
        color_code: ColorCode(Color::Yellow, Color::DarkGray),
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

pub fn clear_screen() {
    use machine::instructions::interrupts;
    interrupts::without_interrupts(||{
        WRITER.lock().clear_screen();
    });
}

/// A color that can be put in a pixel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Color {
    Black       = 0x0,
    Blue        = 0x1,
    Green       = 0x2,
    Cyan        = 0x3,
    Red         = 0x4,
    Magenta     = 0x5,
    Brown       = 0x6,
    LightGray   = 0x7,
    DarkGray    = 0x8,
    LightBlue   = 0x9,
    LightGreen  = 0xa,
    LightCyan   = 0xb,
    LightRed    = 0xc,
    Pink        = 0xd,
    Yellow      = 0xe,
    White       = 0xf
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
            }
        } else {
            panic!("Attempt to print unprintable character");
        }
    }

    fn write_string(&mut self, s: &str) {
        for c in s.bytes() {
            self.write_byte(c);
        }
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
                self.vga_buffer.pixels[row][col] = Color::Black;
            }
        }
        self.x_pos = 0;
        self.y_pos = 0;
        */
    }

    fn draw_rectangle() {  }
}

fn is_printable_ascii(c: u8) -> bool {
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