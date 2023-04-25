OUTPUT_ARCH(riscv64gc)

ENTRY(_boot);


SECTIONS {
    . = 0xfffffffa80000000;
    
    PROVIDE(__image_base = .);

    .text : {
        PROVIDE(__text_start = .);
        *(.init.boot)
        *(.init.rust)
        *(.text .text.*)
        . = ALIGN(4K);
        PROVIDE(__text_end = .);
    }

    .rodata                 : { *(.rodata .rodata.*) }
    .eh_frame_hdr           : { KEEP(*(.eh_frame_hdr)) }
    PROVIDE(__eh_frame = .);
    .eh_frame               : { KEEP(*(.eh_frame)) }
    .dynsym                 : { *(.dynsym) }
    .dynstr                 : { *(.dynstr) }
    .rela                   : { *(.rela*) }
    .dynamic                : { *(.dynamic) }
    .got                    : { *(.got) }
    .data.rel.ro            : { *(.data.rel.ro*) }
    .data                   : ALIGN(4K) { *(.data .data.*) }
    PROVIDE(__global_pointer = .);
    .bss                    : ALIGN(4K) {
        *(.dynbss)
        *(.bss .bss.*)
        *(COMMON)
        . += 0x100000;
        . = ALIGN(128);
        PROVIDE(__stack_top = .);
    }

    PROVIDE(__tdata_start = .);
    .tdata                   : ALIGN(4K) { *(.tdata .tdata.*) }
    .tbss                    : ALIGN(4K) { *(.tbss .tbss.*) }
    PROVIDE(__tdata_end = .);

    PROVIDE(__image_end = .);
}