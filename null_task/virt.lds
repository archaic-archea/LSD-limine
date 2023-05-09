OUTPUT_ARCH(riscv64gc)

ENTRY(_entry);


SECTIONS {
    . = 0x80000000;
    
    PROVIDE(__image_base = .);

    .text : {
        PROVIDE(__text_start = .);
        *(.init.entry)
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
    .got                    : ALIGN(4K) { *(.got) }

    .tdata                   : ALIGN(4K) { 
        PROVIDE(__tdata_start = .);
        *(.tdata .tdata.*) 
    }
    .tbss                    : ALIGN(4K) { 
        *(.tbss .tbss.*) 
        PROVIDE(__tdata_end = .);
    }

    .data.rel.ro            : ALIGN(4K) { *(.data.rel.ro*) }
    .data                   : ALIGN(4K) {
        *(.data .data.*) 
        *(.sdata .sdata.*)
    }

    PROVIDE(__global_pointer = .);
    .bss                    : ALIGN(4K) {
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
    }

    PROVIDE(__image_end = .);
}