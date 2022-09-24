//! Abstractions for printing to the screen

#![no_std]

use core::fmt;
use core::fmt::Write;
use core::ops::{Index, IndexMut};
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use sync::once::Once;
use physics::{Rectangle, Point};
use collections::vec::Vec;
use collections::queue::Queue;
use collections::vec;
use collections::queue;
use machine::port::Port;
use machine::memory::Addr;
use num::Integer;

pub mod font;
pub mod bitmap;

mod color;
pub use color::{Color, Hue};

use bitmap::{Bitmap, Transparency};

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
        color_code: ColorCode(Color::new(Color::Yellow), Color::new(Color::Black)),
        vga_buffer: {
            let screen_buffer_addr = SCREEN_BUFFER_ADDRESS.get()
                .expect("The screen buffer is not initialized");
            unsafe { &mut *(screen_buffer_addr.as_mut_ptr() as *mut VGABuffer) }
        },
        double_buffer: VGABuffer {
            pixels: [[Color::new(Color::Black); SCREEN_WIDTH]; SCREEN_HEIGHT]
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

pub fn clear_screen() {
    use machine::instructions::interrupts;
    interrupts::without_interrupts(||{
        ARTIST.lock().clear_screen();
    });
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
            let mut buffer = match write_target {
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
            if n.to_u8() < 10 {
                w.write_byte(n.to_u8() + 48, WriteTarget::VGABuffer);
            } else {
                let n = n.to_u64();
                let q = n / 10;
                let r = n % 10;
                inner_printint(w, q);
                w.write_byte(r.to_u8() + 48u8, WriteTarget::VGABuffer);
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
        for y in 0..bitmap.height() {
            let i = y + 1;
            for yp in y * Y_SCALE..i * Y_SCALE {
                for x in 0..bitmap.width() {
                    let j = x + 1;
                    for xp in x * X_SCALE..j * X_SCALE {
                        let pixel_array_y = bitmap.height() - y - 1;
                        if pos_is_within_screen_bounds(pos, xp, yp) {
                            let raw_color = bitmap.image_data[pixel_array_y*bitmap.width()+x];
                            let color = Color::from_bitmap_data(raw_color);
                            if bitmap.transparency == Transparency::Black && color == Color::Black {
                                continue;
                            }
                            self.double_buffer[pos.y().to_usize() + yp][pos.x().to_usize() + xp] = color;
                        }
                    }
                }
            }
        }
    }

    /*fn move_bitmap_in_double_buffer(&mut self, old_pos: Point, new_pos: Point, bitmap: Bitmap, bottom_repr: Bitmap, bottom_repr_pos: Point) {
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
    }*/

    /*pub fn redraw_region_in_double_buffer(&mut self, top_left_corner: Point, top_right_corner: Point, bottom_left_corner: Point, bottom_right_corner: Point, bitmaps_to_draw: Vec<(Point, Bitmap)>) {
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
    }*/

    /*pub fn service_all_double_buffer_requests(&mut self) {
        while let Some(request) = self.move_bitmap_in_double_buffer_request_queue.dequeue() {
            self.move_bitmap_in_double_buffer(request.old_pos, request.new_pos, request.repr, request.bottom_repr, request.bottom_repr_pos);
        }
    }*/
    
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

    pub fn draw_on_screen_from_double_buffer(&mut self) {
        /*
        fn draw() {
            for y in 0..SCREEN_HEIGHT {
                for x in 0..SCREEN_WIDTH {
                    self.vga_buffer[y][x] = self.double_buffer[y][x];
                }
            }
        }
        */
        use core::arch::asm;
        ///*
        unsafe {
            asm!("
                # Move 4 bytes at a time from esi to edi, ecx times
                rep movsd",
                in("esi") self.double_buffer.pixels.as_slice().as_ptr(),
                in("edi") self.vga_buffer.pixels.as_slice().as_ptr(),
                in("ecx") DOUBLE_BUFFER_SIZE
            );
        }
        //*/
    }
    /*
    pub fn request_to_move_bitmap_in_double_buffer(&mut self, request: MoveBitmapInDoubleBufferRequest) {
        self.move_bitmap_in_double_buffer_request_queue.enqueue(request);
    }
    */
}

/*
pub fn wait_for_retrace() {
    const INPUT_STATUS: u16 = 0x03da;
    const VRETRACE: u8 = 0x08;
    let input_status_port = Port::new(INPUT_STATUS);
    while input_status_port.read() & VRETRACE != 0 {}
    while input_status_port.read() & VRETRACE == 0 {}
}
*/

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


/*
use core::sync::atomic::{AtomicUsize, Ordering};


fn print_str(s: &str) {
    for c in s.bytes() {
        print_char(c);
    }
}

pub fn print_char(c: u8) {
        let mut vga = 0x80000000 as *mut Color;
        let width = 640;
        let height = 480;
        let curr_x = X_POS.load(Ordering::Relaxed);
        let curr_y = Y_POS.load(Ordering::Relaxed);
        if c == b'\n' {
            
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                for x in 0..8 {
                    unsafe {
                        if byte & (1 << (8 - x - 1)) == 0 {
                            *vga.offset(((curr_y + y)*width+x+curr_x) as isize) = Color::new(Color::Yellow);
                        } else {
                            *vga.offset(((curr_y + y)*width+x) as isize) = Color::new(Color::Black);
                        }
                    }
                }
            }
            if curr_x + 8 >= width {
                X_POS.store(0, Ordering::Relaxed);
                Y_POS.store(curr_y + 8, Ordering::Relaxed);
            } else {
                X_POS.store(curr_x + 8, Ordering::Relaxed);
            }
        } else {
            print_char(b'?');
        }
    }

static X_POS: AtomicUsize = AtomicUsize::new(0);
static Y_POS: AtomicUsize = AtomicUsize::new(0);
*/