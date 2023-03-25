OUTPUT_ARCH(riscv64gc)

ENTRY(_boot);


SECTIONS {
    . = 0xfffffffa80000000;

    .text : {
        PROVIDE(__text_start = .);
        PROVIDE(KERNEL_START = .);
        *(.init.boot)
        *(.init.rust)
        *(.text .text.*)
        . = ALIGN(4K);
        PROVIDE(__text_end = .);
    }

    .sdata : {
        PROVIDE(__global_pointer = .);
        *(.sdata .sdata.*)       
        . = ALIGN(4K);
        PROVIDE(__data_end = .); 
    }

    .data : {
        PROVIDE(__data_start = .);
        *(.data .data.*)
        . = ALIGN(4K);
    }

    .rodata : {
        PROVIDE(__rodata_start = .);
        *(.rodata .rodata.*)
        . = ALIGN(4k);
    }

    PROVIDE(__bss_start = .);
    .sbss : {
        *(.sbss .sbss.*);
    }

    .bss : {
        *(.bss .bss.*)
    }
    . = ALIGN(4K);
    PROVIDE(__bss_end = .);

    .tdata : {
        . = ALIGN(4K);
        PROVIDE(__tdata_start = .);
        
        *(.tdata .tdata.* .tbss .tbss.*);

        . = ALIGN(4K);
        PROVIDE(__tdata_end = .);
    }

    . = ALIGN(2M);
    PROVIDE(KERNEL_END = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
