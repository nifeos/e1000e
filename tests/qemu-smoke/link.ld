/* Loaded by QEMU's -kernel at the physical addresses below, and entered at the address the
   .note.pvh section names (the PVH boot protocol: 32-bit protected mode, paging off). */
ENTRY(pvh_start)

PHDRS
{
    image PT_LOAD FLAGS(7);
    note PT_NOTE FLAGS(4);
}

SECTIONS
{
    . = 1M;
    .note.pvh : { KEEP(*(.note.pvh)) } :image :note
    .text : { *(.text.boot) *(.text .text.*) } :image
    .rodata : { *(.rodata .rodata.*) } :image
    .data : { *(.data .data.*) } :image
    . = ALIGN(4096);
    __bss_start = .;
    .bss : { *(.bss .bss.*) *(COMMON) } :image
    . = ALIGN(16);
    __bss_end = .;
    /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note.GNU-stack) }
}
