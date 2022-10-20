mod bios;
mod uefi;

#[cfg(feature = "bios")]
pub use bios::Color;

#[cfg(not(feature = "bios"))]
pub use uefi::Color;

pub trait Hue {
    /// Converts a byte in the color indexed bitmap pixel array to
    /// a color
    fn from_bitmap_data(raw_color: u8) -> Self;

    /// Returns a color into its numerical representation
    fn to_num(&self) -> u32;
}