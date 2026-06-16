//! Cross-check the decoder against Santiago's real MPLAB lab programs.
//!
//! Each `.hex` here was built by MPLAB from the `.asm` in the same `examples/`
//! folder. We parse the `.hex` and assert the disassembly matches the source —
//! the cheap, durable validation that the instruction table is correct on real
//! firmware (BANKSEL expansion, EQU/CBLOCK addresses, literals, jump tables,
//! 7-seg RETLW patterns, interrupts, EEPROM, delays).
//!
//! These reference files live outside the crate (`../../examples/...`); if you
//! move them, update the paths below.

use new_proteus_core::decode::disassemble;
use new_proteus_core::hex;

fn program(hex_text: &str) -> Vec<u16> {
    hex::parse(hex_text).expect("example .hex should parse").program
}

fn config(hex_text: &str) -> Option<u16> {
    hex::parse(hex_text).expect("example .hex should parse").config
}

// --- examples/Simple one/Punto A.asm: write 0xFF to PORTB, halt ---
#[test]
fn simple_punto_a_matches_source() {
    let hexs = include_str!("../../examples/Simple one/Punto A.hex");
    let p = program(hexs);
    assert_eq!(disassemble(p[0]), "GOTO 0x004"); // GOTO INICIO (ORG 0x04)
    assert_eq!(disassemble(p[4]), "BSF 0x03, 5"); // BANKSEL TRISB -> BSF STATUS,RP0
    assert_eq!(disassemble(p[6]), "CLRF 0x06"); // CLRF TRISB
    assert_eq!(disassemble(p[9]), "MOVLW 0xFF"); // MOVLW 0xFF
    assert_eq!(disassemble(p[10]), "MOVWF 0x06"); // MOVWF PORTB
    assert_eq!(disassemble(p[11]), "GOTO 0x00B"); // GOTO $
    assert_eq!(config(hexs), Some(0x3F10)); // __CONFIG 3F10
}

// --- examples/Multiplication/Multiplicacion.asm: 7 * 10 by repeated add ---
#[test]
fn multiplication_matches_source() {
    let p = program(include_str!("../../examples/Multiplication/mult.hex"));
    assert_eq!(disassemble(p[0]), "CLRF 0x22"); // CLRF MULT
    assert_eq!(disassemble(p[1]), "MOVLW 0x07"); // MOVLW .7
    assert_eq!(disassemble(p[2]), "MOVWF 0x20"); // MOVWF N1
    assert_eq!(disassemble(p[3]), "MOVWF 0x23"); // MOVWF CONT
    assert_eq!(disassemble(p[4]), "MOVLW 0x0A"); // MOVLW .10
    assert_eq!(disassemble(p[5]), "MOVWF 0x21"); // MOVWF N2
    assert_eq!(disassemble(p[6]), "ADDWF 0x22, F"); // SUMA: ADDWF MULT,F
    assert_eq!(disassemble(p[7]), "DECFSZ 0x23, F"); // DECFSZ CONT,F
    assert_eq!(disassemble(p[8]), "GOTO 0x006"); // GOTO SUMA
    assert_eq!(disassemble(p[9]), "GOTO 0x000"); // GOTO INICIO
}

// --- examples/Larger one/TP-turnos-solucion.asm: EEPROM + 7-seg, interrupts ---
#[test]
fn turnos_isr_and_display_table_match_source() {
    let p = program(include_str!("../../examples/Larger one/Solucion.hex"));
    assert_eq!(disassemble(p[0]), "GOTO 0x031"); // GOTO conf
    // Interrupt routine (ORG 0x04): save context.
    assert_eq!(disassemble(p[4]), "BCF 0x0B, 7"); // BCF INTCON,GIE
    assert_eq!(disassemble(p[5]), "BCF 0x0B, 1"); // BCF INTCON,INTF
    assert_eq!(disassemble(p[6]), "MOVWF 0x24"); // MOVWF AUXW
    assert_eq!(disassemble(p[7]), "MOVF 0x03, W"); // MOVF STATUS,W
    // tabla_display: computed jump then the 7-seg patterns.
    assert_eq!(disassemble(p[0x58]), "ADDWF 0x02, F"); // ADDWF PCL,1
    assert_eq!(disassemble(p[0x59]), "RETLW 0x7E"); // b'01111110'
    assert_eq!(disassemble(p[0x5A]), "RETLW 0x0C"); // b'00001100'
}

// --- examples/Another large/TP 2022 - Ejemplo.{asm,hex}: MISMATCHED PAIR ---
//
// The decoder is fine — every word is a valid instruction — but this `.hex`
// does NOT correspond to its `.asm`. Evidence locked in below:
//   * config word in the .hex is 0x3FD0, the .asm says __CONFIG 3F10
//   * .hex ISR (0x04) is `BCF INTCON,INTF / COMF 0x22,F / RETFIE`,
//     the .asm ISR begins `BCF INTCON,T0IF / BCF INTCON,GIE / MOVWF AUX_W`.
// Action: re-export the matching .hex from MPLAB; then update these asserts.
#[test]
fn another_large_pair_is_mismatched_hex_differs_from_asm() {
    let hexs = include_str!("../../examples/Another large/TP 2022 - Ejemplo.hex");
    let p = program(hexs);
    assert_eq!(config(hexs), Some(0x3FD0)); // != asm's 0x3F10
    assert!(disassemble(p[0]).starts_with("GOTO")); // GOTO <config routine>
    assert_eq!(disassemble(p[4]), "BCF 0x0B, 1"); // INTF, not T0IF as the asm has
    assert_eq!(disassemble(p[5]), "COMF 0x22, F"); // asm has MOVWF AUX_W here
    assert_eq!(disassemble(p[6]), "RETFIE");
}
