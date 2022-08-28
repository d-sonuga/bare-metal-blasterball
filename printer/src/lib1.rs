//! Abstractions for printing to the screen

#![no_std]

use core::fmt;
use core::fmt::Write;
use lazy_static::lazy_static;
use sync::mutex::Mutex;

const VGA_BUFFER_HEIGHT: usize = 25;
const VGA_BUFFER_WIDTH: usize = 80;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_pos: 0,
        color_code: ColorCode::new(Color::White, Color::Black),
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

/// The color of either the background or foreground of a VGA buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The full color attributes of a character in the VGA buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    /// Creates a new color code with colors foreground and background
    fn new(foreground: Color, background: Color) -> Self {
        Self(foreground as u8 | ((background as u8) << 4))
    }
}

/// A character that can be written to the VGA buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct VGAScreenChar {
    ascii_code: u8,
    color_code: ColorCode
}

/// The VGA buffer to be written to for screen printing
#[repr(transparent)]
struct VGABuffer {
    chars: [[VGAScreenChar; VGA_BUFFER_WIDTH]; VGA_BUFFER_HEIGHT]
}

/// Writes to the VGA buffer
pub struct Writer {
    column_pos: usize,
    color_code: ColorCode,
    vga_buffer: &'static mut VGABuffer
}

impl Writer {

    /// Writes a byte to the VGA buffer
    fn write_byte(&mut self, c: u8) {
        if c == b'\n' {
            self.newline();
        } else {
            if self.column_pos >= VGA_BUFFER_WIDTH {
                self.newline();
            }
            let row = VGA_BUFFER_HEIGHT - 1;
            self.vga_buffer.chars[row][self.column_pos] = VGAScreenChar {
                ascii_code: c,
                color_code: self.color_code
            };
            self.column_pos += 1;
        }
    }

    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            if is_printable(byte) {
                self.write_byte(byte);
            } else {
                self.write_byte(0xfe);
            }
        }
    }

    /// Prints a newline in the VGA buffer
    fn newline(&mut self) {
        for row in 1..VGA_BUFFER_HEIGHT {
            for col in 0..VGA_BUFFER_WIDTH {
                self.vga_buffer.chars[row - 1][col] = self.vga_buffer.chars[row][col];
            }
        }
        self.column_pos = 0;
        self.clear_row(VGA_BUFFER_HEIGHT - 1);
    }

    /// Deletes all characters on a row of the VGA buffer
    fn clear_row(&mut self, row: usize) {
        for col in 0..VGA_BUFFER_WIDTH {
            self.vga_buffer.chars[row][col] = VGAScreenChar {
                ascii_code: b' ',
                color_code: self.color_code
            }
        }
    }

    /// Clears the screen
    fn clear_screen(&mut self) {
        for row in 0..VGA_BUFFER_HEIGHT {
            for col in 0..VGA_BUFFER_WIDTH {
                self.vga_buffer.chars[row][col] = VGAScreenChar {
                    ascii_code: b' ',
                    color_code: self.color_code
                }
            }
        }
    }
}

/// Determines whether byte is in the printable ASCII range
fn is_printable(byte: u8) -> bool {
    match byte {
        0x20..=0x7e | b'\n' => true,
        _ => false
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}