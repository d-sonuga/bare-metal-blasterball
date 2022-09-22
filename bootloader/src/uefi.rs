use core::ffi::c_void;
use core::{ptr, mem};
use machine::memory::{Addr, EFIMemMapDescriptor, EFIMemRegion, MemMap, MemAllocator};
use num::Integer;
use sync::mutex::Mutex;
use crate::interrupts;
use crate::gdt;
use crate::setup_memory_and_run_game;

//static mut PRINTER: Option<Printer> = None;
static mut SYS_TABLE: Option<*mut EFISystemTable> = None;

/// The main entry point of a UEFI executable as described in specification version 2.7
///
/// # Arguments
///
/// * image_handle: This is the firmware allocated handle used to identify the UEFI image
/// * system_table: This is a pointer to the EFI System Table
///
/// # References
///
/// * UEFI Spec, version 2.7, page 103, chapter 4: EFI System Table, section 4.1
#[no_mangle]
pub unsafe extern "efiapi" fn efi_main(image_handle: EFIHandle, system_table: *mut EFISystemTable) -> ! {
    init_table(system_table);
    let boot_services = (*system_table).boot_services;
    clear_screen();
    
    init_graphics().unwrap();

    let mut mmap = exit_boot_services(image_handle).expect("Unable to exit boot services");
    let mem_allocator = MemAllocator::new(&mut mmap);
    setup_memory_and_run_game(mem_allocator);
    loop {}
}

/// Initializes the graphics mode to a 640x480 mode
unsafe fn init_graphics() -> Result<(), &'static str> {
    let sys_table = SYS_TABLE.as_mut().unwrap();
    let boot_services = (**sys_table).boot_services;
    // To change the graphics mode
    // The GOP (Graphics Output Protocol) needs to be located
    let mut gop: *mut EFIGraphicsOutputProtocol = ptr::null_mut();
    let locate_gop_status = ((*boot_services).locate_protocol)(
        &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
        ptr::null_mut(),
        &mut gop
    );
    if locate_gop_status != STATUS_SUCCESS {
        return Err("GOP not located");
    }
    let max_mode = ((*(*gop).mode).max_mode);
    let mut mode_size = 0usize;
    let mut mode_info: *mut EFIGraphicsOutputModeInfo = ptr::null_mut();
    let mut i = 0;
    loop {
        if i == max_mode {
            return Err("Couldn't find a mode with the necessary requirements");
        }
        let status = ((*gop).query_mode)(gop, i, &mut mode_size, &mut mode_info);
        if status != STATUS_SUCCESS {
            return Err("Failed to get information about a mode");
        }
        if (*mode_info).vertical_resolution == 480 && (*mode_info).horizontal_resolution == 640 {
            let status = ((*gop).set_mode)(gop, i);
            if status != STATUS_SUCCESS {
                return Err("Failed to set a mode");
            }
            let framebuffer = (*(*gop).mode).frame_buffer_base;
            crate::artist_init::init(Addr::new(framebuffer));
            return Ok(())
        }
        i += 1;
    }
}

/// Exits the UEFI boot services and returns the memory map
unsafe fn exit_boot_services(image_handle: EFIHandle) -> Result<MemMap, &'static str> {
    let sys_table = SYS_TABLE.as_mut().unwrap();
    let boot_services = (**sys_table).boot_services;

    // The map_key is required to exit boot services
    let mut map_key = 0usize;
    let mut descriptor_size = 0usize;
    let mut descriptor_version = 0u32;
    let mut mem_map_size = 0usize;

    // Exit boot services to gain full control of the system
    // Get the size of buffer required to store the map in mem_map_size
    let status = ((*boot_services).get_mem_map)(
        &mut mem_map_size,
        ptr::null_mut(),
        &mut map_key,
        &mut descriptor_size,
        &mut descriptor_version
    );
    let stdout = (**sys_table).stdout;
    if status != STATUS_BUFFER_TOO_SMALL | ERROR_BIT {
        return Err("Not too small for some reason")
    }
    // mem_map_size now contains the size of the buffer needed to store the mem_map
    // The EFI_MEMORY_TYPE as specified by the UEFI spcification
    let pool_type = MEM_TYPE_BOOT_SERVICES_DATA;
    // According to the UEFI spec extra space should be allocated
    let mut map_size = mem_map_size + 500;
    let mut mem_map_buffer: *mut u8 = ptr::null_mut();
    // To get the memory map, space needs to be allocated to retrieve it
    let alloc_status = ((*boot_services).alloc_pool)(
        pool_type,
        map_size,
        &mut mem_map_buffer
    );
    if alloc_status != STATUS_SUCCESS {
        return Err("Unable to allocate memory for the memory map");
    }
    let mut mem_map_buffer = mem_map_buffer.cast::<EFIMemRegion>();
    let mut m = 0;
    loop {
        // Get the memory map
        let status = ((*boot_services).get_mem_map)(
            &mut map_size,
            mem_map_buffer,
            &mut map_key,
            &mut descriptor_size,
            &mut descriptor_version
        );
        let boot_exit_status = ((*boot_services).exit_boot_services)(
            image_handle,
            map_key
        );
        if boot_exit_status == STATUS_SUCCESS {
            let mmap_descr = EFIMemMapDescriptor {
                mmap_ptr: mem_map_buffer,
                mmap_size: map_size,
                mmap_entry_size: descriptor_size
            };
            return Ok(MemMap::from(mmap_descr));
        } else if boot_exit_status == ERROR_BIT | STATUS_INVALID_PARAMETER {
            continue;
        } else {
            return Err("Unexpected boot exit status");
        }
    }
}

type Status = usize;
type Uintn = u32;

/// This bit is set in all error status codes
const ERROR_BIT: usize = 1 << (core::mem::size_of::<usize>() * 8 - 1);

/// Status codes
const STATUS_SUCCESS: usize = 0;
const STATUS_BUFFER_TOO_SMALL: Status = 5;
const STATUS_INVALID_PARAMETER: Status = 2;
const STATUS_DEVICE_ERROR: Status = 7;

/// Memory types
const MEM_TYPE_BOOT_SERVICES_DATA: u32 = 4;

/// A firmware allocated handle that is used to identify the UEFI image
/// on various functions.
/// The handle also supports one or more protocols that the image can use
type EFIHandle = *const core::ffi::c_void;

/// A UEFI table which contains pointer to runtime and boot services
///
/// # References
///
/// * The UEFI spec, version 2.7, chapter 4, section 3
#[repr(C)]
pub struct EFISystemTable {
    /// The table header of the EFI System Table
    header: EFITableHeader,
    /// A string that identifies the system firmware for the platform
    firmware_vendor: *const u16,
    /// A firmware vendor specific value that identifies the
    /// revision of the system firmware for the platform
    firmware_revision: u32,
    /// The handle for the active console input device
    stdin_handle: EFIHandle,
    /// A pointer to the EFISimpleTextInputProtocol
    /// interface that is associated with `console_in_handle`
    stdin: *mut [u8; 24],
    /// The handle for the active console output device
    stdout_handle: EFIHandle,
    /// A pointer to the EFISimpleTextOutputProtocol
    /// interface that is associated with `console_out_handle`
    stdout: *mut EFISimpleTextOutputProtocol,
    /// The handle for the active standard error console device
    std_error_handle: EFIHandle,
    /// A pointer to the EFISimpleTextOutputProtocol
    /// interface that is associated with `std_error_handle`
    std_err: *mut EFISimpleTextOutputProtocol,
    /// A pointer to the EFIRuntimeServicesTable
    runtime_services: *mut [u8; 136],
    /// A pointer to the EFIBootServicesTable
    boot_services: *mut EFIBootServices,
    /// Number of system configuration tables in the
    /// EFIConfigurationTable pointed to by `configuration_table`
    no_of_table_entries: usize,
    /// A pointer to the system configuration tables
    configuration_table: *mut EFIConfigurationTableEntry
}

/// A structure that precedes all UEFI table structures
#[repr(C)]
struct EFITableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32
}

/// A UEFI protocol used to control text-based output devices
#[repr(C)]
struct EFISimpleTextOutputProtocol {
    /// Reset the console out device
    reset: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, extended_verification: bool),
    /// Displays a null terminated string on the device at the current cursor location
    output_string: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, string: *const u16),
    /// Tests to see of the console output device supports the given null terminated string
    test_string: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, *const u16),
    /// Queries information concerning the output device's supported text mode
    query_mode: extern "efiapi" fn(
        this: *mut EFISimpleTextOutputProtocol, 
        mode_number: usize,
        columns: *mut usize,
        rows: *mut usize
    ),
    /// Sets the current mode of the output device
    set_mode: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, mode_number: usize),
    /// Sets the foreground and background colors of the text that is outputted
    set_attribute: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, attribute: usize),
    /// Clears the screen with the currently set background color
    clear_screen: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol),
    /// Sets the current cursor position
    set_cursor_position: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, column: usize, row: usize),
    /// Toggles the visibility of the cursor
    enable_cursor: extern "efiapi" fn(this: *mut EFISimpleTextOutputProtocol, visible: bool),
    /// Pointer to the SimpleTextOutputMode
    mode: *mut SimpleTextOutputMode
}

#[repr(C)]
struct SimpleTextOutputMode {
    max_mode: i32,
    mode: i32,
    attribute: i32,
    cursor_column: i32,
    cursor_row: i32,
    cursor_visible: bool
}

/// An entry in the EFIConfigurationTable
#[repr(C)]
struct EFIConfigurationTableEntry {
    /// The 128-bit GUID value that uniquely identifies the system
    /// configuration table
    vendor_guid: u128,
    /// A pointer to the table associated with vendor GUID
    vendor_table: *const core::ffi::c_void
}

/// The boot services in the EFISystemTable
#[repr(C)]
struct EFIBootServices {
    /// The table header
    header: EFITableHeader,
    /// These fields are not needed in this project
    unneeded0: [u8; 4*8],
    /// Returns the current memory map
    ///
    /// # Arguments
    ///
    /// * mem_map_size: A pointer to the size, in bytes, of the MemoryMap buffer.
    ///     On input, this is the size of the buffer allocated by the
    ///     caller. On output, it is the size of the buffer returned by the
    ///     firmware if the buffer was large enough, or the size of the
    ///     buffer needed to contain the map if the buffer was too
    ///     small
    /// * mem_map: A pointer to the buffer in which firmware places the
    ///     current memory map. The map is an array of EFIMemDescriptors
    /// * map_key: A pointer to the location in which firmware returns the key
    ///     for the current memory map
    /// * descriptor_size: A pointer to the location in which firmware returns the
    ///     size, in bytes, of an individual EFIMemDescriptor
    /// * descriptor_version: A pointer to the location in which firmware returns the
    ///     version number associated with the EFIMemDescriptor
    get_mem_map: unsafe extern "efiapi" fn(
        mem_map_size: &mut usize,
        mem_map: *mut EFIMemRegion,
        map_key: &mut usize,
        descriptor_size: &mut usize,
        descriptor_version: &mut u32
    ) -> Status,
    /// Allocates pool memory from the UEFI firmware
    ///
    /// # Arguments
    ///
    /// * pool_type: the type of pool to allocate
    /// * size: the number of bytes tp allocate from the pool
    /// * buffer: a pointer to a pointer to the allocated buffer if the call succeeds
    alloc_pool: unsafe extern "efiapi" fn(
        pool_type: u32,
        size: usize,
        buffer: &mut *mut u8
    ) -> Status,
    /// These fields are not needed in this project
    unneeded1: [u8; 20*8],
    /// Releases all firmware provided boot services and hands control over to
    /// the OS
    exit_boot_services: unsafe extern "efiapi" fn(image_handle: EFIHandle, map_key: usize) -> Status,
    /// These fields are not needed in this project
    unneeded2: [u8; 10*8],
    /// A UEFI protocol for finding the location of a protocol with Guid `protocol_guid`
    ///
    /// # Arguments
    ///
    /// * protocol_guid: Provides the protocol to search for
    /// * registration: Nullable optional registration key
    /// * out_protocol: On return, a pointer to the first interface that matches protocol
    ///   and registration
    locate_protocol: extern "efiapi" fn(
        protocol_guid: &Guid,
        registration: *mut c_void,
        out_protocol: &mut *mut EFIGraphicsOutputProtocol
    ) -> Status,
    /// These fields are not needed in this project
    unneeded3: [u8; 6*8]
}

/// A unique 64-bit aligned 128-bit value used to identify protocols
#[derive(Debug)]
#[repr(C)]
struct Guid {
    first: u32,
    second: u16,
    third: u16,
    fourth: [u8; 8]
}

const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: Guid = Guid {
    first: 0x9042a9de,
    second: 0x23dc,
    third: 0x4a38,
    fourth: [0x96,0xfb,0x7a,0xde,0xd0,0x80,0x51,0x6a]
};

/// Provides basic abstractions to set video modes and interact with
/// the graphics controller's frame buffer
#[repr(C)]
struct EFIGraphicsOutputProtocol {
    /// Returns information for an available graphics mode that
    /// the graphics device and the set of active video output
    /// devices supports.
    ///
    /// # Arguments
    ///
    /// * this: The EFIGraphicsOutputProtocol instance
    /// * mode_no: The mode number to return information on
    /// * size_of_info: A pointer to the size in bytes of the info buffer
    /// * info: A pointer to a callee allocated buffer that returns information about mode_no
    query_mode: unsafe extern "efiapi" fn(
        this: *mut EFIGraphicsOutputProtocol,
        mode_no: u32,
        size_of_info: &mut usize,
        info: &mut *mut EFIGraphicsOutputModeInfo
    ) -> Status,
    /// Set the video device into the specified mode and clears
    /// the visible portions of the output display to black
    ///
    /// # Arguments
    ///
    /// * this: The EFIGraphicsOutputProtocol instance
    /// * mode_no: Abstraction that defines the current video mode
    set_mode: unsafe extern "efiapi" fn(
        this: *mut EFIGraphicsOutputProtocol,
        mode_no: u32
    ) -> Status,
    /// Software abstraction to draw on the video device’s frame
    /// buffer
    ///
    /// # Arguments
    ///
    /// * this: The EFIGrahicsOutputProtocol instance
    /// * blt_buffer: The data to transfer to the graphics screen
    /// * blt_op: The operation to perform when copying blt_buffer to the graphics screen
    /// * source_x: The x coordinate of the source of the blt_op
    /// * source_y: The y coordinate of the source of the blt_op
    /// * dest_x: The x coordinate of the destination of the blt_op
    /// * dest_y: The y coordinate of the destination of the blt_op
    /// * width: The width of a rectangle in the blt rectangle in pixels
    /// * height: The height of a rectangle in the blt rectangle in pixels
    /// * delta: To be 0 if the entire blt_buffer is used, else the number of bytes to be used
    ///   in a row of the blt_buffer
    blt: unsafe extern "efiapi" fn(
        this: *mut EFIGraphicsOutputProtocol,
        blt_buffer: *mut EFIGraphicsOutputBltPixel,
        blt_op: EFIGraphicsOutputBltOp,
        source_x: usize,
        source_y: usize,
        dest_x: usize,
        dest_y: usize,
        width: usize,
        height: usize,
        delta: usize
    ) -> Status,
    /// A pointer to the read-only EFIGraphicsOutputProtocolMode
    mode: &'static EFIGraphicsOutputProtocolMode,
}

#[derive(Debug)]
#[repr(C)]
struct EFIGraphicsOutputBltPixel {
    blue: u8,
    green: u8,
    red: u8,
    reserved: u8
}

/// Operations that can be performed when copying a
/// buffer to the graphics screen with EFIGraphicsOutputProtocol.blt
#[derive(Debug)]
#[repr(u32)]
enum EFIGraphicsOutputBltOp {
    /// Write data from a buffer directly to every pixel
    /// of the video display rectangle 
    BltVideoFill = 0,
    /// Read data from a video display rectangle and place it
    /// in the buffer
    BltVideoToBltBuffer = 1,
    /// Write data from the blt directly to the video display rectangle
    BltBufferToVideo = 2,
    /// Copy from video display rectangle to video display rectangle
    BltVideoToVideo = 3,
    /// No valid EFIGraphicsOutputBltOp has a value up to this
    GraphicsOutputBltOpMax = 4
}

/// A read-only structure that describes the current graphics mode.
/// The values can only be changed with the appropriate interface functions
/// in EFIGraphicsOutputProtocol
#[derive(Debug)]
#[repr(C)]
struct EFIGraphicsOutputProtocolMode {
    /// The number of modes supported by query_mode and set_mode
    max_mode: u32,
    /// Current mode of the graphics device
    ///
    /// Valid mode numbers are 0 to `max_mode` - 1
    mode: u32,
    /// Pointer to a read-only EFIGraphicsOutputModeInfo
    info: &'static EFIGraphicsOutputModeInfo,
    /// Size of `info` in bytes
    size_of_info: usize,
    /// Base address of graphics linear frame buffer
    frame_buffer_base: u64,
    /// Amount of frame buffer needed to support the active mode
    frame_buffer_size: usize
}

#[derive(Debug)]
#[repr(C)]
struct EFIGraphicsOutputModeInfo {
    /// The version of this data structure
    version: u32,
    /// The size of video screen in pixels in the x dimension
    horizontal_resolution: u32,
    /// The size of the video screen in pixels in the y dimension
    vertical_resolution: u32,
    /// 
    pixel_format: EFIGraphicsPixelFormat,
    /// A bitmask which is valid only if pixel_format
    /// is set to EFIGraphicsPixelFormat::PixelBitmask
    pixel_info: EFIPixelBitmask,
    /// The number of pixel elements per video memory line,
    /// which may be padded to an amount of memory alignment
    pixels_per_scan_line: u32
}

/// An enumeration that defines the pixel format of the pixel in a graphics mode
#[derive(Debug)]
#[repr(u32)]
enum EFIGraphicsPixelFormat {
    /// A pixel is 32 bits and bytes 0, 1, 2 and 3 represent
    /// red, green, blue and none (reserved) respectively
    ///
    /// The byte values for the red, green and blue components
    /// represent color intensity in the range 0..=255
    PixelRGBReserved8BPC = 0,
    /// The same as `PixelRGBReserved8BPC` expect that bytes 0, 1, and 2
    /// represent blue, green and red respectively
    PixelBGRReserved8BPC = 1,
    /// The pixel definition of the physical frame buffer defined by EFIPixelBitmask
    PixelBitmask = 2,
    /// The graphics mode does not support a physical frame buffer
    PixelBltOnly = 3,
    /// Valid EFIGraphicsPixelFormat are less than this
    PixelFormatMax = 4
}

// The bits in the mask must not overlap positions
#[repr(C)]
struct EFIPixelBitmask {
    /// The bits set here represents the red component of the pixel
    red_mask: u32,
    /// The bits set here represents the green component of the pixel
    green_mask: u32,
    /// The bits set here represents the blue component of the pixel
    blue_mask: u32,
    reserved_mask: u32
}

impl fmt::Debug for EFIPixelBitmask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EFIPixelBitmask")
            .field("red_mask", &Hex(self.red_mask))
            .field("green_mask", &Hex(self.green_mask))
            .field("blue_mask", &Hex(self.blue_mask))
            .field("reserved_mask", &Hex(self.reserved_mask))
            .finish()
    }
}

unsafe fn printint(n: usize) {
    if n >= 10 {
        let q = n / 10;
        let r = n % 10;
        printint(q);
        printdigit(r);
    } else {
        printdigit(n);
    }
}

unsafe fn printdigit(n: usize) {
    assert!(n >= 0 && n < 10);
    let n = n as u32;
    PreExitPrinter.write_char(char::from_u32(n + 48).unwrap());
}

use core::fmt;
use core::fmt::Write;

/// A printer that can be used before exiting boot services
struct PreExitPrinter;

impl fmt::Write for PreExitPrinter {
    fn write_char(&mut self, c: char) -> fmt::Result {
        let sys_table = unsafe { SYS_TABLE.as_mut().unwrap() };
        let mut int_utf16: [u16; 2] = [c as u16, 0u16];
        unsafe {
            let stdout = (**sys_table).stdout;
            ((*stdout).output_string)(stdout, int_utf16.as_mut_slice().as_ptr());
        }
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

/// A printer that can be used after exiting boot services
/// and before setting up memory to initialize the artist
pub struct PostExitPrinter;
impl fmt::Write for PostExitPrinter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            PostExitPrinter.print_char(c);
        }
        Ok(())
    }
}

fn print_str(s: &str) {
    PreExitPrinter.write_str(s);
}

fn print_fmt(f: fmt::Arguments) {
    PreExitPrinter.write_fmt(f);
}

fn init_table(sys_table: *mut EFISystemTable) {
    unsafe {
        SYS_TABLE = Some(sys_table)
    }
}

pub fn _print(args: fmt::Arguments) {
    PreExitPrinter.write_fmt(args).unwrap();
}

fn clear_screen() {
    unsafe {
        let system_table = *SYS_TABLE.as_mut().unwrap();
        let stdout = system_table.read().stdout;
        (stdout.read().clear_screen)(stdout);
    }
}

use core::sync::atomic::{AtomicUsize, Ordering};


static X_POS: AtomicUsize = AtomicUsize::new(0);
static Y_POS: AtomicUsize = AtomicUsize::new(0);


use artist::font;
impl PostExitPrinter {
    pub fn print_char(&mut self, c: u8) {
        let mut vga = 0x80000000 as *mut EFIGraphicsOutputBltPixel;
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
                            *vga.offset(((curr_y + y)*width+x+curr_x) as isize) = EFIGraphicsOutputBltPixel {
                                blue: 255,
                                green: 0,
                                red: 0,
                                reserved: 0
                            };
                        } else {
                            *vga.offset(((curr_y + y)*width+x) as isize) = EFIGraphicsOutputBltPixel {
                                blue: 0,
                                green: 255,
                                red: 0,
                                reserved: 0
                            };
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
            self.print_char(b'?');
        }
    }
}

pub fn is_printable_ascii(c: u8) -> bool {
    match c {
        b' '..=b'~' => true,
        _ => false
    }
}

struct Hex<N: Integer>(N);
impl<N: Integer + fmt::Display> fmt::Debug for Hex<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:#}", self.0)
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    writeln!(PostExitPrinter, "{}", info);
    loop {}
}