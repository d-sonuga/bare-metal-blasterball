.code16
.section .boot.stage2, "awx"
.global mmap_entry_count

stage_2:

set_target_op_mode:
    pushf
    mov ax, 0xec00
    mov bl, 0x2
    int 0x15
    popf

build_mem_map:
    lea di, es:[_mmap]
    call map_memory

switch_to_graphics_mode:
    mov ah, 0
    mov al, 0x13
    int 0x10

enter_protected_mode_again:
    cli
    lgdt [gdt32_descriptor]
    mov eax, cr0
    or eax, 0x1
    mov cr0, eax
    push 0x8
    mov eax, offset restart_protected_mode
    push eax
    retf

map_memory:
    xor ebx, ebx
    xor bp, bp
    mov edx, 0x0534d4150
    mov eax, 0xe820
    mov dword ptr es:[di + 20], 1
    mov ecx, 24
    int 0x15
    jc map_memory_fail
    mov edx, 0x0534d4150
    cmp eax, edx
    jne map_memory_fail
    test ebx, ebx
    je map_memory_fail
    jmp jmp_in
jmp_in:
    jcxz skip_entry
    cmp cl, 20
    jbe no_text
    test byte ptr es:[di + 20], 1
    je skip_entry
no_text:
    mov ecx, es:[di + 8]
    or ecx, es:[di + 12]
    jz skip_entry
    inc bp
    add di, 24
skip_entry:
    test ebx, ebx
    jne e820lp
    jmp e820f
e820lp:
    mov eax, 0xe820
    mov dword ptr es:[di + 20], 1
    mov ecx, 24
    int 0x15
    jc e820f
    mov edx, 0x0534d4150
    jmp jmp_in
e820f:
    mov [mmap_entry_count], bp
    clc
    ret
map_memory_fail:
    stc
    ret


load_app_fail_err_msg:              .asciz "Failed to load app"
mmap_entry_count:                   .word 0
