//! **From QEMU's PVH (para-virtualized hardware) entry to Rust**: the note that names the entry
//! point, then the 32-bit stub that zeroes the uninitialized data, identity-maps the first four
//! gibibytes with 2-mebibyte pages, enters long mode and calls `smoke_main`.
//!
//! The PVH boot protocol (Xen's `XEN_ELFNOTE_PHYS32_ENTRY`, which QEMU's `-kernel` honors in an
//! Executable and Linkable Format (ELF) image) enters in 32-bit protected mode with paging off and
//! interrupts disabled. The firmware has already enumerated the Peripheral Component Interconnect
//! (PCI) bus and assigned every base address register by then. The top gibibyte, where the
//! firmware places those registers, is mapped uncached.

core::arch::global_asm!(
    r#"
    .section .note.pvh, "a", @note
    .align 4
    .long 4
    .long 8
    .long 18
    .asciz "Xen"
    .align 4
    .quad pvh_start
    .align 4

    .section .bss.boot, "aw", @nobits
    .align 4096
boot_level_4_table:
    .skip 4096
boot_level_3_table:
    .skip 4096
boot_level_2_tables:
    .skip 4 * 4096
boot_stack:
    .skip 64 * 1024
boot_stack_top:

    .section .rodata.boot, "a"
    .align 16
boot_descriptor_table:
    .quad 0
    .quad 0x00af9a000000ffff
    .quad 0x00cf92000000ffff
boot_descriptor_table_end:
boot_descriptor_table_pointer:
    .word boot_descriptor_table_end - boot_descriptor_table - 1
    .long boot_descriptor_table

    .section .text.boot, "ax"
    .code32
    .global pvh_start
pvh_start:
    cli
    cld
    mov edi, offset __bss_start
    mov ecx, offset __bss_end
    sub ecx, edi
    shr ecx, 2
    xor eax, eax
    rep stosd
    mov esp, offset boot_stack_top

    mov eax, offset boot_level_3_table
    or eax, 3
    mov dword ptr [boot_level_4_table], eax

    xor ecx, ecx
.Lboot_level_3_loop:
    mov eax, ecx
    shl eax, 12
    add eax, offset boot_level_2_tables
    or eax, 3
    mov dword ptr [boot_level_3_table + ecx * 8], eax
    inc ecx
    cmp ecx, 4
    jne .Lboot_level_3_loop

    xor ecx, ecx
.Lboot_level_2_loop:
    mov eax, ecx
    shl eax, 21
    or eax, 0x83
    cmp ecx, 1536
    jb .Lboot_cached
    or eax, 0x18
.Lboot_cached:
    mov dword ptr [boot_level_2_tables + ecx * 8], eax
    inc ecx
    cmp ecx, 2048
    jne .Lboot_level_2_loop

    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax
    mov eax, offset boot_level_4_table
    mov cr3, eax
    mov ecx, 0xc0000080
    rdmsr
    or eax, 1 << 8
    wrmsr
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    lgdt [boot_descriptor_table_pointer]
    mov eax, 0x08
    push eax
    mov eax, offset boot_long_mode
    push eax
    retf

    .code64
boot_long_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax
    mov rsp, offset boot_stack_top
    call smoke_main
.Lboot_halt:
    hlt
    jmp .Lboot_halt

    .text
"#
);
