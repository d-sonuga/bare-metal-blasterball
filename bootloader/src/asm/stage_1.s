.section .boot.stage1, "awx"
.code16
.intel_syntax noprefix
.global boot

boot:
    xor ax, ax          # Zero out the segment registers
    mov ds, ax
    mov gs, ax
    mov es, ax
    mov fs, ax
    mov ss, ax

    cld

    mov sp, 0x7c00      # Initialize stack pointer

    mov [BOOT_DRIVE], dl

    call bios_clear_screen
    mov bx, offset loading_msg
    call print_string16

enable_a20:
    in al, 0x92       # Enable A20-Line with port 92
    test al, 2
    jnz enter_protected_mode
    or al, 2
    and al, 0xfe
    out 0x92, al

enter_protected_mode:
    cli
    push ds
    push es
    lgdt [gdt32_descriptor]
    mov eax, cr0
    or al, 1
    mov cr0, eax
    jmp start_protected_mode

start_protected_mode:
    mov ax, 0x10                            # Data segment's index into gdt
    mov ds, ax
    mov es, ax

enter_unreal_mode:
    mov eax, cr0
    and al, 0xfe
    mov cr0, eax
    pop es
    pop ds
    sti

check_for_bios_int13h_extensions:
    mov ah, 0x41
    mov bx, 0x55aa
    mov dl, [BOOT_DRIVE]
    int 0x13
    jc no_bios_int13h_ext_err
    
# Loads the first sector into buffer
# If the number of sectors left is not 0, load the next
# If the number of sectors left is 0, coninue to next block
load_rest_of_app:
    mov word ptr [dap_buffer_segment], 0
    mov ax, offset _app_buffer                     # Using this as a temporary buffer to make loading easier
    mov word ptr [dap_offset_to_buffer], ax

    mov eax, offset _rest_of_app_start_addr
    mov ebx, offset _rest_of_app_end_addr
    sub ebx, eax
    shr ebx, 9                                      # ebx now contains the total number of sectors to load

    mov word ptr [dap_no_of_sectors], 1
    mov word ptr [dap_lba_start], 1
    mov edi, offset _rest_of_app_start_addr  # Initial address where the buffered sector should be stored

load_rest_of_app_loop:
    mov si, offset dap
    mov ah, 0x42
    mov dl, [BOOT_DRIVE]
    int 0x13
    jc load_rest_of_app_err
    
    mov ecx, 512 / 4                    # To move 512 bytes 4 bytes at a time
    mov esi, offset _app_buffer
    rep movsd [edi], [esi]
    dec ebx                             # Number of sectors left to load
    inc word ptr [dap_lba_start]

    cmp ebx, 0
    jne load_rest_of_app_loop
    jmp stage_2

load_rest_of_app_err:
    mov bx, offset load_rest_of_app_err_msg
    call print_string16
    jmp halt

no_bios_int13h_ext_err:
    mov bx, offset no_bios_int13h_ext_err_msg
    call print_string16
    jmp halt

halt:
    hlt
    jmp halt

# Prints a string on screen
#
# Expected
# --------
# bx contains the address of the null-terminated string to print
#
# Result
# ------
# Doesn't return anything or affect any registers
#
# Roles
# -----
# ah - BIOS number to print on screen
# si - Index of the next character to print in the string
# al - Character at index si is loaded here before being printed
.equ BIOS_PRINT, 0x0e
.equ CPU_INT, 0x10
print_string16:
    mov si, 0                     # Index of string to print initialized to 0
    jmp print_string16_loop

print_string16_loop:
    mov al, [bx + si]             # Load the ascii value of the character at index %si
    cmp al, 0                     # Is the character the null byte
    je print_string16_loop_end     # If it is, the string has been printed
    call print_char16
    inc si                         # Increase the index of the string
    jmp print_string16_loop        # Keep printing

print_string16_loop_end:
    ret

# Prints a character
#
# Expected
# --------
# al contains the ascii value of the character to be printed
#
# Result
# ------
# Doesn't return anything or affect any registers
print_char16:
    push ax
    mov ah, 0x0e            # The BIOS number for printing
    int 0x10                # Interrupt the CPU to print
    pop ax
    ret

bios_clear_screen:
    push ax
    mov ah, 0
    int 0x10
    pop ax
    ret


start_message_16bit:                .asciz "Successfully started in 16-bit real mode"
load_app_err_msg:                   .asciz "Failed to load app"
load_rest_of_app_err_msg:           .asciz "Failed to load the rest of the app"
no_bios_int13h_ext_err_msg:         .asciz "No BIOS int13h extensions"
loading_msg:                        .asciz "Loading..."

BOOT_DRIVE:                 .byte 0

gdt32:
    .quad 0             # Null descriptor
gdt32_code_descr:
    .word 0xffff        # Lower 2 bytes of limit
    .word 0x0           # Lower 2 bytes of base address
    .byte 0x0           # Next byte of base address
    .byte 0b10011010    # Access byte
    .byte 0b11001111    # Flags and next 4 bytes of limit
    .byte 0x0           # Last byte of base address
gdt32_data_descr:
    .word 0xffff        # Lower 2 bytes of limit
    .word 0x0           # Lower 2 bytes of base address
    .byte 0x0           # Next byte of base address
    .byte 0b10010010    # Access byte
    .byte 0b11001111    # Flags and next 4 bytes of limit
    .byte 0x0           # Last byte of base address
gdt32_end:

gdt32_descriptor:
    .word gdt32_end - gdt32 - 1
    .long gdt32


# Disk Address Packet
dap:
    .byte 0x10      # dap size (16 bytes)
    .byte 0x0       # Unsused
dap_no_of_sectors:
    .word 0
dap_offset_to_buffer:
    .word 0
dap_buffer_segment:
    .word 0
dap_lba_start:
    .quad 0

. = boot + 510
.word 0xaa55
