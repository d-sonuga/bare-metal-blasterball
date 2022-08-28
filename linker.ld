ENTRY(boot)
SECTIONS {
	. = 0x500;
	_app_buffer = .;
	. += 512;
	. = ALIGN(0x1000);
	__page_table_start = .;
	_pml4t = .;
	. += 0x1000;
	_pdpt1 = .;
	. += 0x1000;
	_pdpt2 = .;
	. += 0x1000;
	_pdpt3 = .;
	. += 0x1000;
	__page_table_end = .;
	__app_start = .;
	_mmap = .;
	. += 0x1000;
	. = 0x7c00;
	.blasterball : {
		*(.boot.stage1)
		_rest_of_app_start_addr = .;
		*(.boot.stage2)
		*(.boot.stage3)
		*(.text .text.*)
		*(.rodata .rodata.*)
		*(.data .data.*)
		*(.eh_frame_hdr)
		*(.eh_frame)
		*(.got)
		. = ALIGN(512);
		_rest_of_app_end_addr = .;
	}
	__app_end = .;
}
