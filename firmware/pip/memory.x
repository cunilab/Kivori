/* RP2040-Zero: 2 MB W25Q16 flash, 264 KB SRAM. Linked in by build.rs (cortex-m-rt's link.x includes it). */
MEMORY {
    /* The 256-byte second-stage bootloader the ROM copies out of flash first. */
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    /* Striped banks 0-3 (256 KB) plus the two 4 KB banks, one contiguous region: no heap, one stack. */
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}

SECTIONS {
    /* ### Boot loader */
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;
