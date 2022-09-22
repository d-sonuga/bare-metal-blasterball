use core::cmp::PartialEq;
use crate::Hue;

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
    pub const Purple: u8      = 0x6b;

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