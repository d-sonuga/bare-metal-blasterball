use core::{mem, slice};
use collections::vec::Vec;
use collections::vec;

/// The number of colors in the default VGA palette.
/// All bitmaps used are assumed to have this number of colors in their color tables
const COLOR_TABLE_SIZE: usize = 254;

/// A bitmap file with a BITMAPV5HEADER.
/// The bitmap is assumed to be 8bpp (bits per pixel) and it's palette is assumed
/// to correspond to the default VGA palette
///
/// For information on the bitmap file format: <https://en.wikipedia.org/wiki/BMP_file_format>
///
/// I used arrays of u8s instead of the corresponding u32 or u16 in all the related
/// bitmap structures because integer values in the bitmap structure are stored in
/// little-endian format
pub struct Bitmap {
    /// The start of the file used for identification
    file_header: &'static BitmapFileHeader,
    /// The Bitmap v5 header
    dib_header: &'static BitmapDIBHeader,
    /// The palette for the image
    ///
    /// This structure assumes it always corresponds with the default VGA palette
    /// so there is no need to change the VGA palette to draw the bitmap
    color_table: &'static [u8],
    /// The actual bit array which gets drawn on the screen
    pub image_data: &'static [u8]
}

/// The start of the bitmap file which is used for identification
#[repr(C, packed)]
struct BitmapFileHeader {
    // Always "BM" in ascii
    bmp_id: [u8; 2],
    /// The size of the bitmap file in bytes
    image_size: [u8; 4],
    reserved: u32,
    /// The offset of the bitmap image data into the file
    image_data_offset: [u8; 4]
}

/// The BITMAPV5HEADER as described in 
/// <https://docs.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header>
#[repr(C, packed)]
struct BitmapDIBHeader {
    /// Size of this DIB header
    header_size: u32,
    /// Width of the bitmap in pixels. Again, the value starts from the first byte
    image_width: [u8; 4],
    /// Height of the bitmap in pixels.
    image_height: [u8; 4],
    /// Number of planes for the target device. Always 1
    planes: [u8; 2],
    /// The number of bits that define each pixel and the maximum number of colors in the bitmap
    ///
    /// If 0, then the number of bits per pixel is specified by the jpg or png format.
    /// If 1, then it's a monochrome
    /// If 4, 8, 16, 24, 32 then the bitmap has a max of 2^24 colors
    /// This bitmap representation assumes this field to always be 8
    bits_per_pixel: [u8; 2],
    /// Specifies the compression used in the bitmap
    ///
    /// This bitmap representation assumes this field to never be compressed, that is always set to 0 (BI_RGB)
    compression_method: [u8; 4],
    /// Size of the image in bytes. May be set to 0 if no compression is used
    size_image: [u8; 4],
    horizontal_resolution: [u8; 4],
    vertical_resolution: [u8; 4],
    /// No of color indexes in the color table used by the bitmap
    ///
    /// Assumed to be 256 (the maximum value for an 8bpp bitmap) for this representation
    /// of the bitmap
    no_of_colors_used: [u8; 4],
    /// No of color indexes required for displaying the bitmap
    no_of_important_colors: [u8; 4],
    /// Color mask that specifies the red component of each pixel.
    /// Valid only if the compression method is BI_BITFIELDS
    red_mask: [u8; 4],
    /// Color mask that specifies the green component of each pixel.
    /// Valid only if the compression method is BI_BITFIELDS
    green_mask: [u8; 4],
    /// Color mask that specifies the blue component of each pixel.
    /// Valid only if the compression method is BI_BITFIELDS
    blue_mask: [u8; 4],
    /// Color mask that specifies the alpha component of each pixel.
    /// Valid only if the compression method is BI_BITFIELDS
    alpha_mask: [u8; 4],
    /// Specifies the color space of the DIB
    cs_type: [u8; 4],
    /// Specifies the xyz coordinates of 3 colors that correspond to red, green, blue endpoints
    /// for the logical color space associated with the bitmap
    ///
    /// Not relevant for the purposes of this bitmap representation
    endpoints: [u8; 36],
    /// Toned response curve for red
    ///
    /// Not relevant for the purposes of this bitmap representation
    gamma_red: [u8; 4],
    /// Toned response curve for green
    ///
    /// Not relevant for the purposes of this bitmap representation
    gamma_green: [u8; 4],
    /// Toned response curve for blue
    ///
    /// Not relevant for the purposes of this bitmap representation
    gamma_blue: [u8; 4],
    /// Rendering intent for the bitmap
    ///
    /// Not relevant for the purposes of this bitmap representation
    intent: [u8; 4],
    /// Offset in bytes from the DIB header beginning to the start of the profile data
    ///
    /// Not relevant for the purposes of this bitmap representation
    profile_data: [u8; 4],
    /// Size of the embedded profile data
    profile_size: [u8; 4],
    reserved: [u8; 4]
}

impl Bitmap {
    /// Creates a representation of a bitmap in memory from the raw bytes `raw_bytes`
    pub fn from(raw_bytes: &[u8]) -> Result<Self, &'static str> {
        if !is_valid_bitmap(raw_bytes) {
            return Err("Bitmap is not valid");
        }
        unsafe {
            const FILE_HEADER_SIZE: isize = core::mem::size_of::<BitmapFileHeader>() as isize;
            const DIB_HEADER_SIZE: isize = core::mem::size_of::<BitmapDIBHeader>() as isize;
            let file_header = &(*(raw_bytes.as_ptr() as *const BitmapFileHeader));
            let dib_header = &(*(raw_bytes.as_ptr().offset(FILE_HEADER_SIZE) as *const BitmapDIBHeader));
            let color_table = slice::from_raw_parts(raw_bytes.as_ptr().offset(FILE_HEADER_SIZE + DIB_HEADER_SIZE), COLOR_TABLE_SIZE);
            let image_data_offset = u32::from_le_bytes(file_header.image_data_offset) as isize;
            let image_width = u32::from_le_bytes(dib_header.image_width) as usize;
            let image_height = u32::from_le_bytes(dib_header.image_height) as usize;
            let image_data = slice::from_raw_parts(raw_bytes.as_ptr().offset(image_data_offset), image_width * image_height);
            Ok(Bitmap {
                file_header,
                dib_header,
                color_table,
                image_data
            })
        }
    }

    /// Returns the width of the image in the bitmap
    pub fn width(&self) -> usize {
        u32::from_le_bytes(self.dib_header.image_width) as usize
    }

    /// Returns the height of the image in the bitmap
    pub fn height(&self) -> usize {
        u32::from_le_bytes(self.dib_header.image_height) as usize
    }
}

fn is_valid_bitmap(raw_bytes: &[u8]) -> bool {
    raw_bytes.len() > 2 && raw_bytes[0] == b'B' && raw_bytes[1] == b'M'
}
