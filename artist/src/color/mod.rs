mod bios;
mod uefi;

use num::Integer;
use bios::Color as BIOSColor;
use uefi::Color as UEFIColor;

#[cfg(feature = "bios")]
pub type Color = BIOSColor;

#[cfg(not(feature = "bios"))]
pub type Color = UEFIColor;

pub trait Hue {
    /// Converts a byte in the color indexed bitmap pixel array to
    /// a color
    fn from_bitmap_data(raw_color: u8) -> Self;

    /// Returns a color into its numerical representation
    fn to_num(&self) -> u32;
}