use machine::memory::Addr;
use artist::SCREEN_BUFFER_ADDRESS;

pub fn init(graphics_buffer_address: Addr) {
    SCREEN_BUFFER_ADDRESS.call_once(|| graphics_buffer_address);
}