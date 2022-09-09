//! Abstractions for printing to the screen

#![no_std]

use core::fmt;
use core::fmt::Write;
use core::ops::{Index, IndexMut};
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use physics::{Rectangle, Point};
use collections::vec::Vec;
use collections::queue::Queue;
use collections::vec;
use collections::queue;
use machine::port::Port;
use num::Integer;

pub mod font;
pub mod bitmap;

use bitmap::Bitmap;

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;
pub const DOUBLE_BUFFER_SIZE: usize = SCREEN_HEIGHT * SCREEN_WIDTH;

lazy_static! {
    pub static ref ARTIST: Mutex<Artist> = Mutex::new(Artist {
        x_pos: 0,
        y_pos: 0,
        color_code: ColorCode(Color(Color::Yellow), Color(Color::Black)),
        vga_buffer: unsafe { &mut *(0xa0000 as *mut VGABuffer) },
        double_buffer: VGABuffer {
            pixels: [[Color(Color::Black); SCREEN_WIDTH]; SCREEN_HEIGHT]
        },
        move_bitmap_in_double_buffer_request_queue: queue!(
            item_type => MoveBitmapInDoubleBufferRequest,
            capacity => 10
        ),
        transparency: Transparency::Black
    });
}

#[derive(Clone)]
pub struct MoveBitmapInDoubleBufferRequest {
    pub old_pos: Point,
    pub new_pos: Point,
    pub repr: Bitmap,
    pub bottom_repr: Bitmap,
    pub bottom_repr_pos: Point
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

pub fn clear_screen() {
    use machine::instructions::interrupts;
    interrupts::without_interrupts(||{
        ARTIST.lock().clear_screen();
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
    move_bitmap_in_double_buffer_request_queue: Queue<'static, MoveBitmapInDoubleBufferRequest>,
    transparency: Transparency
}

impl Artist {

    /// Writes a byte to the VGA buffer
    fn write_byte(&mut self, c: u8) {
        if c == b'\n' {
            self.newline();
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                for x in 0..8 {
                    if byte & (1 << (8 - x - 1)) == 0 {
                        self.double_buffer[self.y_pos + y][self.x_pos + x] = self.color_code.background();
                    } else {
                        self.double_buffer[self.y_pos + y][self.x_pos + x] = self.color_code.foreground();
                    }
                }
            }
            self.x_pos += 8;
            if self.x_pos >= SCREEN_WIDTH {
                self.newline();
                self.x_pos = 0;
            }
            if self.y_pos >= SCREEN_HEIGHT - 8 {
                self.y_pos = 0;
            }
            self.redraw_on_screen_from_double_buffer();
        } else {
            self.write_byte(b'?');
        }
    }

    fn write_string(&mut self, s: &str) {
        for c in s.bytes() {
            self.write_byte(c);
        }
    }

    fn printint<T: Integer>(&mut self, n: T) {
        fn inner_printint<T: Integer>(w: &mut Artist, n: T) {
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
/*
    pub fn draw_rectangle(&mut self, rect: &Rectangle) {
        // top_left -> top_right
        // top_right -> bottom_right
        // bottom_right -> bottom_left
        // bottom_left -> top_left
        self.x_pos = rect.top_left.x();
        self.y_pos = rect.top_left.y();
        for i in 0..rect.width {
            self.vga_buffer[self.y_pos][self.x_pos + i] = self.color_code.foreground();
            self.vga_buffer[self.y_pos + rect.height][self.x_pos + i] = self.color_code.foreground();
        }
        for i in 0..rect.height {
            self.vga_buffer[self.y_pos + i][self.x_pos] = self.color_code.foreground();
            self.vga_buffer[self.y_pos + i][self.x_pos + rect.width] = self.color_code.foreground();
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
            self.vga_buffer[self.y_pos][self.x_pos + i] = self.color_code.background();
            self.vga_buffer[self.y_pos + rect.height][self.x_pos + i] = self.color_code.background();
        }
        for i in 0..rect.height {
            self.vga_buffer[self.y_pos + i][self.x_pos] = self.color_code.background();
            self.vga_buffer[self.y_pos + i][self.x_pos + rect.width] = self.color_code.background();
        }
    }
*/

    pub fn draw_bitmap_in_double_buffer(&mut self, pos: Point, bitmap: &Bitmap) {
        /*for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                let pixel_array_y = bitmap.height() - y - 1;
                if pos_is_within_screen_bounds(pos, x, y) {
                    let color = bitmap.image_data[pixel_array_y*bitmap.width()+x];
                    if self.transparency == Transparency::Black && color == Color::Black {
                        continue;
                    }
                    self.double_buffer[pos.y().to_usize() + y][pos.x().to_usize() + x] = Color::new(color);
                }
            }
        }
        */
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                let pixel_array_y = bitmap.height() - y - 1;
                if pos_is_within_screen_bounds(pos, x, y) {
                    let color = bitmap.image_data[pixel_array_y*bitmap.width()+x];
                    if self.transparency == Transparency::Black && color == Color::Black {
                        continue;
                    }
                    self.double_buffer[pos.y().to_usize() + y][pos.x().to_usize() + x] = Color::new(color);
                }
            }
        }
    }

    fn move_bitmap_in_double_buffer(&mut self, old_pos: Point, new_pos: Point, bitmap: Bitmap, bottom_repr: Bitmap, bottom_repr_pos: Point) {
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                let pixel_array_y = bitmap.height() - y - 1;
                let bottom_repr_array_y = bottom_repr.height() - y - 1;
                if pos_is_within_screen_bounds(old_pos, x, y) {
                    self.double_buffer[old_pos.y().to_usize() + y][old_pos.x().to_usize() + x] =
                        Color::new(bottom_repr.image_data[(bottom_repr_array_y)*bottom_repr.width()+x]);
                }
            }
        }
        for y in 0..bitmap.height() {
            for x in 0..bitmap.width() {
                let pixel_array_y = bitmap.height() - y - 1;
                if pos_is_within_screen_bounds(new_pos, x, y) {
                    self.double_buffer[new_pos.y().to_usize() + y][new_pos.x().to_usize() + x] = 
                        Color::new(bitmap.image_data[pixel_array_y*bitmap.width()+x]);
                }
            }
        }
    }

    pub fn redraw_region_in_double_buffer(&mut self, top_left_corner: Point, top_right_corner: Point, bottom_left_corner: Point, bottom_right_corner: Point, bitmaps_to_draw: Vec<(Point, Bitmap)>) {
        for (point, bitmap) in bitmaps_to_draw.iter() {
            for y in top_left_corner.y().to_usize()..=bottom_left_corner.y().to_usize() {
                for x in top_left_corner.x().to_usize()..=top_right_corner.x().to_usize() {
                    if pos_is_within_screen_bounds(Point(x.to_i16(), y.to_i16()), 0, 0) {
                        // If the current point being drawn falls within the current bitmap's
                        // range, draw the bitmap, else continue
                        if x >= point.x().to_usize() && x < point.x().to_usize() + bitmap.width()
                            && y >= point.y().to_usize() && y < point.y().to_usize() + bitmap.height() {
                                let (bitmap_x, bitmap_y) = (x - point.x().to_usize(), y - point.y().to_usize());
                                let pixel_array_y = bitmap.height() - bitmap_y - 1;
                                let color = bitmap.image_data[pixel_array_y*bitmap.width()+bitmap_x];
                                if self.transparency == Transparency::Black && color == Color::Black {
                                    continue;
                                }
                                self.double_buffer[y][x] = Color::new(color);
                        }
                    }
                }
            }
        }
    }

    pub fn service_all_double_buffer_requests(&mut self) {
        while let Some(request) = self.move_bitmap_in_double_buffer_request_queue.dequeue() {
            self.move_bitmap_in_double_buffer(request.old_pos, request.new_pos, request.repr, request.bottom_repr, request.bottom_repr_pos);
        }
    }
    
    pub fn draw_background_in_double_buffer(&mut self, background: &Bitmap) {
        // Rust was too slow for this.
        // Had to use assembly
        use core::arch::asm;
        unsafe {
            asm!("
                # Move 4 bytes at a time from esi to edi, ecx times
                rep movsd",
                in("esi") background.image_data.as_ptr(),
                in("edi") self.double_buffer.pixels.as_slice().as_ptr(),
                in("ecx") DOUBLE_BUFFER_SIZE / 4
            );
        }
    }

    pub fn redraw_on_screen_from_double_buffer(&mut self) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                self.vga_buffer[y][x] = self.double_buffer[y][x];
            }
        }
    }

    pub fn request_to_move_bitmap_in_double_buffer(&mut self, request: MoveBitmapInDoubleBufferRequest) {
        self.move_bitmap_in_double_buffer_request_queue.enqueue(request);
    }
}

/// An indicator of what color should be regarded as transparent when drawing
/// a bitmap
#[derive(PartialEq)]
pub enum Transparency {
    /// When black is encountered, don't draw it
    Black,
    /// Draw everything, don't exclude any color
    None
}

pub fn wait_for_retrace() {
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

#[inline]
pub fn pos_is_within_screen_bounds(pos: Point, dx: usize, dy: usize) -> bool {
    pos.y() >= 0 && pos.x() >= 0 
        && pos.y().to_usize() + dy < SCREEN_HEIGHT
        && pos.x().to_usize() + dx < SCREEN_WIDTH
}

impl fmt::Write for Artist {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
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