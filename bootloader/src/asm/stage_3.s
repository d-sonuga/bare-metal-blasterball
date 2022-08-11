.section .boot.stage3, "awx"
.code32


restart_protected_mode:
    mov bx, 0x10
    mov ds, bx
    mov es, bx
    mov ss, bx


check_cpuid:
    # Check if CPUID is supported by attempting to flip the ID bit (the 22nd bit)
    # in the FLAGS register. If it can be flipped, CPUID is available
    # Necessary to make the switch to long mode

    pushfd              # Put value of flags register into eax
    pop eax
    mov ecx, eax        # For comparing later
    xor eax, (1 << 21)  # Flip the ID bit
    push eax            # Put value in eax into flags register
    popfd
    pushfd              # Copy value into eax again, to make comparisons
    pop eax
    push ecx            # Restore original value to flags register, which is in ecx
    popfd
    cmp eax, ecx        # If they're not equal, then CPUID isn't supported
    je no_cpuid_err

check_long_mode:
    # Check if extended CPUID functions are available
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb no_long_mode_err
    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29)           # Test if LM-bit, 30th bit, is set
    jz no_long_mode_err         # If they aren't, no long mode
    cli
    mov ecx, 43234
    lidt zero_idt               # So Non Maskable Interrupts can cause a triple fault
    mov ecx, 234

check_cpu:

    cli                   # disable interrupts

    lidt zero_idt 

setup_page_tables:
    # Zero out page tables
    mov eax, 0
    mov ecx, offset __page_table_end
    sub ecx, offset __page_table_start
    shr ecx, 2
    mov edi, offset __page_table_start
    rep stosd

    mov eax, offset _pdpt1                       # The first PDPT
    or eax, 0b11                                 # Present, writable and huge
    mov [_pml4t], eax

    mov eax, offset _pdpt2
    or eax, 0b11                                 # Present, writable and huge
    mov [_pml4t + 8], eax

    mov eax, offset _pdpt3
    or eax, 0b11
    mov [_pml4t + 8 * 2], eax

    # Map the PDP tables
    mov edi, 0
    or edi, (1 << 7) | 0b11
    mov ecx, 0
    map_pdpt1:
    mov [_pdpt1 + 8 * ecx], edi
    add edi, 0x40000000
    inc ecx
    cmp ecx, 512
    jne map_pdpt1

    mov ecx, 0
    map_pdpt2:
    mov [_pdpt2 + 8 * ecx], edi
    add edi, 0x40000000
    inc ecx
    cmp ecx, 512
    jne map_pdpt2

    mov ecx, 0
    map_pdpt3:
    mov [_pdpt3 + 8 * ecx], edi
    add edi, 0x40000000
    inc ecx
    cmp ecx, 512
    jne map_pdpt3

set_pml4_addr:
    mov eax, offset _pml4t
    mov cr3, eax

enable_pae:
    mov eax, cr4                # Setting the PAE-enabled bit in cr4, the 6th bit
    or eax, 1 << 5
    mov cr4, eax

switch_to_long_mode:
    mov ecx, 0xc0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

enable_paging:
    mov eax, cr0                # Setting the paging bit
    or eax, (1 << 31)
    mov cr0, eax

load_64bit_gdt:
    lgdt gdt64_descriptor

jmp_to_long_mode:
    push 0x8
    mov eax, offset main
    push eax
    retf

.equ VIDEO_MEMORY, 0xb8000
.equ WHITE_ON_BLACK, 0x0f
# Prints a null-terminated string in 32-bit mode
#
# Expected
# --------
# edx contains the address of the string
#
# Result
# ------
# Doesn't return anything or affect any registers
print_string32:
    mov eax, 0                              # Index of the next character to print in the string
    mov esi, 0                              # Index of the next position in video memory
    xor ebx, ebx
    jmp print_string32_print_loop

print_string32_print_loop:
    mov bl, [edx + eax]                     # Character at index eax
    cmp bl, 0
    je print_string32_end
    mov [esi + VIDEO_MEMORY], bl            # Calculate the location in video memory to put next character
    mov byte ptr [esi + VIDEO_MEMORY + 1], 0x0f
    inc eax                                 # Increment the indexes
    add esi, 2
    jmp print_string32_print_loop

print_string32_end:
    ret

no_cpuid_err:
    mov edx, offset no_cpuid_err_msg
    call print_string32
    jmp halt

no_long_mode_err:
    mov edx, offset no_long_mode_err_msg
    call print_string32
    jmp halt

halt2:
    jmp halt2



#check_cpuid:
    # Check if CPUID is supported by attempting to flip the ID bit (bit 21)
    # in the FLAGS register. If we can flip it, CPUID is available.

    # Copy FLAGS in to EAX via stack
#    pushfd
 #   pop eax

    # Copy to ECX as well for comparing later on
#    mov ecx, eax

    # Flip the ID bit
#    xor eax, (1 << 21)

    # Copy EAX to FLAGS via the stack
#    push eax
 #   popfd

    # Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
#    pushfd
 #   pop eax

    # Restore FLAGS from the old version stored in ECX (i.e. flipping the
    # ID bit back if it was ever flipped).
#    push ecx
 #   popfd

    # Compare EAX and ECX. If they are equal then that means the bit
    # wasn't flipped, and CPUID isn't supported.
#    cmp eax, ecx
 #   je no_cpuid
  #  ret
#no_cpuid:
 #   mov esi, offset no_cpuid_err_msg
  #  call print_string32
#no_cpuid_spin:
 #   hlt
  #  jmp no_cpuid_spin

#check_long_mode:
    # test if extended processor info in available
 #   mov eax, 0x80000000    # implicit argument for cpuid
  #  cpuid                  # get highest supported argument
   # cmp eax, 0x80000001    # it needs to be at least 0x80000001
#    jb no_long_mode        # if it's less, the CPU is too old for long mode

    # use extended info to test if long mode is available
#    mov eax, 0x80000001    # argument for extended processor info
 #   cpuid                  # returns various feature bits in ecx and edx
  #  test edx, (1 << 29)    # test if the LM-bit is set in the D-register
   # jz no_long_mode        # If it's not set, there is no long mode
    #ret
#no_long_mode:
 #   jmp halt2

/*
.align 4
zero_idt:
    .word 0
    .byte 0

gdt64:
    .quad 0x0000000000000000          # Null Descriptor - should be present.
    .quad 0x00209A0000000000          # 64-bit code descriptor (exec/read).
    .quad 0x0000920000000000          # 64-bit data descriptor (read/write).

.align 4
    .word 0                              # Padding to make the "address of the GDT" field aligned on a 4-byte boundary

gdt64_descriptor:
    .word gdt64_descriptor - gdt64 - 1    # 16-bit Size (Limit) of GDT.
    .long gdt64                            # 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)

*/
.align 4
zero_idt:
    .word 0
    .byte 0

# The only difference between the 64 bit and 32 bit gdt is that
# the long mode bit is set in the two segements
gdt64:
    .quad 0x0000000000000000          # Null Descriptor - should be present
    .quad 0x00209a0000000000          # 64-bit code descriptor (exec/read)
    .quad 0x0000920000000000          # 64-bit data descriptor (read/write)

.align 4
    .word 0             # Align 
gdt64_end:

gdt64_descriptor:
    .word gdt64_end - gdt64 - 1
    .long gdt64

no_cpuid_err_msg:                   .asciz "CPUID not supported"
no_long_mode_err_msg:               .asciz "Long mode not supported"