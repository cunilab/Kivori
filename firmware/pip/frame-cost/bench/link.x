/* Memory map of the ESP32-C3 as the firmware sees it (esp-hal esp32c3 memory.x, TRM chapter 3):
   code in external flash through the cache at IROM 0x4200_0000, read-only data (the asset blob)
   through DROM 0x3C00_0000, data and stack in internal SRAM at 0x3FC8_0000. */
ENTRY(_start)
MEMORY
{
  IROM : ORIGIN = 0x42000000, LENGTH = 4M
  DROM : ORIGIN = 0x3C000000, LENGTH = 4M
  RAM  : ORIGIN = 0x3FC80000, LENGTH = 384K
}
SECTIONS
{
  .text : { *(.text._start) *(.text .text.*) } > IROM
  .rodata : { *(.srodata .srodata.*) *(.rodata .rodata.*) } > DROM
  .data : { *(.sdata .sdata.*) *(.data .data.*) } > RAM
  .bss (NOLOAD) : { *(.sbss .sbss.*) *(.bss .bss.*) *(COMMON) } > RAM
  _stack_top = ORIGIN(RAM) + LENGTH(RAM);
  /DISCARD/ : { *(.eh_frame*) *(.riscv.attributes) *(.comment) }
}
