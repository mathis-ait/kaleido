/* Placement dans les marges de code.bin de Rubis Oméga EUR v1.0 :
 * code  : fin de .text   0x00579604 .. 0x0057A000 (exécutable, 2 556 octets)
 * état  : fin de .bss    0x006AE620 .. 0x006AF000 (inscriptible, hors fichier :
 *         le .bss suit .data sans alignement, 0x0062F9F4 + 0x7EC2C = 0x006AE620 ;
 *         0x006AEFF0.. est réservé aux compteurs de trace) */
MEMORY
{
    CAVE_TEXT (rx) : ORIGIN = 0x00579610, LENGTH = 0x9F0
    CAVE_DATA (rw) : ORIGIN = 0x006AE640, LENGTH = 0x980
}

SECTIONS
{
    .text : {
        *(.text.hooks)
        *(.text.testpad)
        *(.text .text.*)
        *(.rodata .rodata.*)
    } > CAVE_TEXT

    .data (NOLOAD) : {
        *(.state)
        *(.data .data.*)
        *(.bss .bss.* COMMON)
    } > CAVE_DATA

    /DISCARD/ : { *(.ARM.exidx*) *(.ARM.attributes) *(.comment) *(.note*) }
}
