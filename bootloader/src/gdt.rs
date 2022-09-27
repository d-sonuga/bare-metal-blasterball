use machine::tss::{TaskStateSegment, load_tss};
use machine::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector, CS, DS, SegmentRegister, SS};
use machine::memory::Addr;
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            // 20Kib
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = Addr::new(unsafe { &STACK as *const _ as u64 });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_seg_selector = gdt.add_entry(Descriptor::code_segment());
        let data_seg_selector = gdt.add_entry(Descriptor::data_segment());
        let tss_seg_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_seg_selector,
                data_seg_selector,
                tss_seg_selector
            }
        )
    };
}

#[derive(Debug)]
struct Selectors {
    code_seg_selector: SegmentSelector,
    data_seg_selector: SegmentSelector,
    tss_seg_selector: SegmentSelector
}

pub fn init() {
    GDT.0.load();
    unsafe {
        CS.set(GDT.1.code_seg_selector);
        DS.set(GDT.1.data_seg_selector);
        SS.set(GDT.1.data_seg_selector);
        load_tss(GDT.1.tss_seg_selector);
    }
}
