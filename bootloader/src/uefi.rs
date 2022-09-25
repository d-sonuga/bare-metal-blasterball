use core::ffi::c_void;
use core::{ptr, mem};
use core::ops::BitOr;
use machine::keyboard::uefi::{EFIInputKey, EFIKeyData, EFIKeyToggle};
use machine::memory::{Addr, EFIMemMapDescriptor, EFIMemRegion, MemMap, MemAllocator, EFIMemRegionType, MemChunk};
use machine::uefi;
use machine::uefi::{EFIEventType, EFITpl, EFIEvent, EFITimerType, EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID};
use event_hook;
use event_hook::{Event};
use machine::keyboard::uefi::EFIScanCode;
use machine::keyboard::{KeyDirection, KeyCode, KeyModifiers, KeyEvent};
use num::Integer;
use sync::mutex::Mutex;
use sync::once::Once;
use crate::{setup_memory_and_run_game};


static FRAMEBUFFER: Once<Addr> = Once::new();

machine::efi_entry_point!(main);


fn main() -> ! {
    let systable = uefi::get_systable().unwrap();
    let stdout = systable.stdout();
    stdout.clear_screen();

    let framebuffer = init_graphics().unwrap();
    init_framebuffer(framebuffer);
    
    extern "efiapi" fn notify_keypress_fn(event: EFIEvent, context: *mut c_void) {
        let systable = uefi::get_systable().unwrap();
        let mut stdin = systable.stdin();
        let key = stdin.read_key().unwrap();
        if key.is_none() {
            return;
        }
        let key = key.unwrap();
        let key_event = KeyEvent::try_from(key);
        if let Ok(event) = key_event {
            event_hook::send_event(Event::Keyboard(event.keycode, event.direction, event.key_modifiers));
        }
    }

    let boot_services = systable.boot_services();
    let event = boot_services.create_event(
        EFIEventType::Timer | EFIEventType::NotifySignal,
        EFITpl::Notify,
        notify_keypress_fn
    ).unwrap();
    boot_services.set_timer(event, EFITimerType::Periodic, 1).unwrap();
    boot_services.signal_event(event).unwrap();

    let (stack_mem, heap_mem) = alloc_game_mem().unwrap();
    setup_memory_and_run_game(stack_mem, heap_mem);
    loop {}
}

/// Initializes the graphics mode to a 640x480 mode
fn init_graphics() -> Result<Addr, &'static str> {
    let systable = uefi::get_systable();
    if systable.is_none() {
        return Err("System table is not initialized");
    }
    let systable = systable.unwrap();
    let boot_services = systable.boot_services();
    // To change the graphics mode
    // The GOP (Graphics Output Protocol) needs to be located
    let gop = boot_services.locate_protocol(&EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID)?;
    let max_mode = gop.mode().max_mode();
    let mut mode_no = 0;
    loop {
        if mode_no == max_mode {
            return Err("Couldn't find a mode with the necessary requirements");
        }
        let mode_info = gop.query_mode(mode_no)?;
        if mode_info.vertical_resolution() == 480 && mode_info.horizontal_resolution() == 640 {
            gop.set_mode(mode_no)?;
            let framebuffer = Addr::new(gop.mode().frame_buffer_base());
            crate::artist_init::init(framebuffer);
            return Ok(framebuffer)
        }
        mode_no += 1;
    }
}

fn alloc_game_mem() -> Result<(MemChunk, MemChunk), &'static str> {
    use crate::{APP_STACK_SIZE, APP_HEAP_SIZE};
    let systable = uefi::get_systable();
    if systable.is_none() {
        return Err("System table is not intialized");
    }
    let systable = systable.unwrap();
    let boot_services = systable.boot_services();
    let mut stack_mem = boot_services.alloc_mem(EFIMemRegionType::LoaderData, APP_STACK_SIZE as usize)?;
    let mut heap_mem = boot_services.alloc_mem(EFIMemRegionType::LoaderData, APP_HEAP_SIZE as usize)?;
    Ok((stack_mem, heap_mem))
}

fn init_framebuffer(fb: Addr) {
    FRAMEBUFFER.call_once(|| fb);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if FRAMEBUFFER.get().is_some() {
        // The printer can't be used until the
        // FRAMEBUFFER has been initialized
        writeln!(Printer, "{}", info);
    }
    loop {}
}


use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt;
use::core::fmt::Write;
use artist::{FONT_WIDTH, FONT_HEIGHT, X_SCALE, Y_SCALE, SCREEN_WIDTH, SCREEN_HEIGHT, Color};
static X_POS: AtomicUsize = AtomicUsize::new(0);
static Y_POS: AtomicUsize = AtomicUsize::new(0);
use artist::font;

// Can only be used after setting up the graphics mode
// and initializing the framebuffer
pub struct Printer;
impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            Printer.print_char(c);
        }
        Ok(())
    }
}

// Quick and dirty printing
impl Printer {
    pub fn print_char(&mut self, c: u8) {
        let framebuffer = FRAMEBUFFER.get();
        if framebuffer.is_none() {
            return;
        }
        let mut vga = framebuffer.unwrap().as_mut_ptr() as *mut Color;
        let curr_x = X_POS.load(Ordering::Relaxed);
        let curr_y = Y_POS.load(Ordering::Relaxed);
        if c == b'\n' {
            X_POS.store(0, Ordering::Relaxed);
            let old_y = Y_POS.load(Ordering::Relaxed);
            Y_POS.store(old_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
        } else if is_printable_ascii(c) {
            for (y, byte) in font::FONT[c].iter().enumerate() {
                let i = y + 1;
                for yp in y * Y_SCALE..i*Y_SCALE {
                    for x in 0..FONT_WIDTH {
                        let j = x + 1;
                        for xp in x * X_SCALE..j * X_SCALE {
                            unsafe {
                                if byte & (1 << (FONT_WIDTH - x - 1)) == 0 {
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color::new(Color::Blue);
                                } else {
                                    *vga.offset(((curr_y + yp)*SCREEN_WIDTH+xp+curr_x) as isize) = Color::new(Color::Black);
                                }
                            }
                        }
                    }
                }
            }
            X_POS.store(curr_x + FONT_WIDTH * X_SCALE, Ordering::Relaxed);
            if X_POS.load(Ordering::Relaxed) >= SCREEN_WIDTH {
                X_POS.store(0, Ordering::Relaxed);
                Y_POS.store(curr_y + FONT_HEIGHT * Y_SCALE, Ordering::Relaxed);
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
