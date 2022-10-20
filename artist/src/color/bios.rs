use core::cmp::PartialEq;
use crate::Hue;

/// A color that can be put in a pixel
#[derive(Copy, Clone, PartialEq, Debug, Eq)]
#[repr(transparent)]
pub struct Color(u8);

impl Color {
    pub const BLACK: u8       = 0x0;
    pub const BLUE: u8        = 0x1;
    pub const GREEN: u8       = 0x2;
    pub const CYAN: u8        = 0x3;
    pub const RED: u8         = 0x4;
    pub const MAGENTA: u8     = 0x5;
    pub const BROWN: u8       = 0x6;
    pub const LIGHT_GRAY: u8   = 0x7;
    pub const DARK_GRAY: u8    = 0x8;
    pub const LIGHT_BLUE: u8   = 0x9;
    pub const LIGHT_GREEN: u8  = 0xa;
    pub const LIGHT_CYAN: u8   = 0xb;
    pub const LIGHT_RED: u8    = 0xc;
    pub const PINK: u8        = 0xd;
    pub const YELLOW: u8      = 0xe;
    pub const WHITE: u8       = 0xf;
    pub const PURPLE: u8      = 0x6b;

    /// Creates a new instance of the color
    pub fn new(color: u8) -> Self {
        Self(color)
    }
}

impl Hue for Color {
    /// Converts a byte in the color indexed bitmap pixel array to
    /// a color
    ///
    /// No actual conversion is needed here since it already corresponds
    /// to what the VGA is expecting
    ///
    /// This has to be here because of the UEFI version of color which does not
    /// correspond with any VGA
    fn from_bitmap_data(raw_color: u8) -> Self {
        Self(raw_color)
    }

    /// Returns a color into its numerical representation
    ///
    /// Has to return a u32 to remain compatible with the UEFI color
    fn to_num(&self) -> u32 {
        u32::from_be_bytes([self.0, self.0, self.0, self.0])
    }
}

impl PartialEq<u8> for Color {
    fn eq(&self, rhs: &u8) -> bool {
        self.0 == *rhs
    }
}