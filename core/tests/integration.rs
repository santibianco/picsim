//! Integration tests: load + run real firmware end to end.
//!
//! The decode cross-check stays here; now that execute exists we also *run*
//! firmware and assert observable results and exact cycle counts — the latter
//! being the property you can confirm against MPLAB's stopwatch.

use new_proteus_core::{decode, hex, Core};

const BLINK_HEX: &str = include_str!("fixtures/blink.hex");
const SEG_COUNTER_HEX: &str = include_str!("fixtures/seg_counter.hex");

#[test]
fn loads_and_disassembles_blink_fixture() {
    let img = hex::parse(BLINK_HEX).expect("blink.hex should parse");
    let expected = [
        (0x1683u16, "BSF 0x03, 5"), // BSF STATUS, RP0  -> bank 1
        (0x0186, "CLRF 0x06"),      // CLRF TRISB       -> PORTB all outputs
        (0x1283, "BCF 0x03, 5"),    // BCF STATUS, RP0  -> bank 0
        (0x0A86, "INCF 0x06, F"),   // INCF PORTB, F    -> count
        (0x2803, "GOTO 0x003"),     // GOTO loop
    ];
    for (i, (word, text)) in expected.iter().enumerate() {
        assert_eq!(img.program[i], *word, "program word {} mismatch", i);
        assert_eq!(decode::disassemble(*word).as_str(), *text);
    }
}

#[test]
fn blink_runs_and_increments_portb() {
    let mut core = Core::new();
    core.load_hex(BLINK_HEX).unwrap();
    // Setup: BSF RP0 ; CLRF TRISB ; BCF RP0  (3 cycles); PORTB still 0.
    core.run_cycles(3).unwrap();
    assert_eq!(core.read_pins().1, 0);
    // One INCF PORTB,F (1 cycle) -> PORTB = 1.
    core.run_cycles(1).unwrap();
    assert_eq!(core.read_pins().1, 1);
    // GOTO (2) + INCF (1) -> PORTB = 2.
    core.run_cycles(3).unwrap();
    assert_eq!(core.read_pins().1, 2);
}

// examples/Multiplication: 7 * 10 by repeated addition, result in MULT (0x22).
#[test]
fn multiplication_program_computes_7_times_10() {
    let mut core = Core::new();
    core.load_hex(include_str!("../../examples/Multiplication/mult.hex"))
        .unwrap();
    let mut guard = 0;
    while core.cpu().mem.phys_read(0x22) != 70 && guard < 10_000 {
        core.cpu_mut().step().unwrap();
        guard += 1;
    }
    assert_eq!(core.cpu().mem.phys_read(0x22), 70, "MULT should be 7*10");
    assert!(guard < 10_000, "MULT never reached 70");
}

#[test]
fn multiplication_reaches_70_in_exactly_31_cycles() {
    // 6 (setup) + 6 full loops * 4 + final ADDWF (1) = 31 cycles.
    let mut core = Core::new();
    core.load_hex(include_str!("../../examples/Multiplication/mult.hex"))
        .unwrap();
    let ran = core.run_cycles(31).unwrap();
    assert_eq!(ran, 31);
    assert_eq!(core.cpu().mem.phys_read(0x22), 70);
}

// fixtures/seg_counter.hex: drive a 7-seg on RB0..RB6 with digit patterns from a
// RETLW lookup table (ADDWF PCL computed jump), counting 0..9 with a delay loop.
#[test]
fn seg_counter_disassembles_correctly() {
    let img = hex::parse(SEG_COUNTER_HEX).expect("seg_counter.hex parses");
    assert_eq!(decode::disassemble(img.program[13]).as_str(), "GOTO 0x004"); // main loop
    assert_eq!(decode::disassemble(img.program[14]).as_str(), "ADDWF 0x02, F"); // ADDWF PCL,F
    assert_eq!(decode::disassemble(img.program[15]).as_str(), "RETLW 0x3F"); // digit 0
    assert_eq!(decode::disassemble(img.program[24]).as_str(), "RETLW 0x6F"); // digit 9
    assert_eq!(decode::disassemble(img.program[33]).as_str(), "RETURN"); // end of delay
}

#[test]
fn seg_counter_first_digit_is_zero_pattern() {
    let mut core = Core::new();
    core.load_hex(SEG_COUNTER_HEX).unwrap();
    // Run until the firmware first drives PORTB; that's the digit-0 pattern.
    let mut guard = 0;
    while core.cpu().mem.phys_read(0x06) == 0 && guard < 1000 {
        core.cpu_mut().step().unwrap();
        guard += 1;
    }
    assert_eq!(core.cpu().mem.phys_read(0x06), 0x3F); // segments a..f -> "0"
}

const MUX_COUNTER_HEX: &str = include_str!("fixtures/mux_counter.hex");

// fixtures/mux_counter.hex: two-digit multiplexed counter (00-99). Segments on
// RB0..RB6; digit selects RA0 (units) / RA1 (tens); main-loop multiplexing.
#[test]
fn mux_counter_disassembles() {
    let img = hex::parse(MUX_COUNTER_HEX).expect("mux_counter.hex parses");
    assert_eq!(decode::disassemble(img.program[0x2e]).as_str(), "ADDWF 0x02, F"); // table
    assert_eq!(decode::disassemble(img.program[0x2f]).as_str(), "RETLW 0x3F"); // "0"
    assert_eq!(decode::disassemble(img.program[0x11]).as_str(), "BSF 0x05, 0"); // units select RA0
    assert_eq!(decode::disassemble(img.program[0x17]).as_str(), "BSF 0x05, 1"); // tens select RA1
}

#[test]
fn mux_counter_drives_digit_zero_pattern() {
    let mut core = Core::new();
    core.load_hex(MUX_COUNTER_HEX).unwrap();
    let mut g = 0;
    while core.cpu().mem.phys_read(0x06) == 0 && g < 2000 {
        core.cpu_mut().step().unwrap();
        g += 1;
    }
    assert_eq!(core.cpu().mem.phys_read(0x06), 0x3F); // first digit shown is 0
}

#[test]
fn mux_counter_alternates_both_digit_selects() {
    let mut core = Core::new();
    core.load_hex(MUX_COUNTER_HEX).unwrap();
    core.reset_frame_accumulators();
    core.run_cycles(3000).unwrap(); // several multiplex passes
    assert!(core.pin_high_cycles(0) > 0, "RA0 (units) never selected");
    assert!(core.pin_high_cycles(1) > 0, "RA1 (tens) never selected");
}

const EEPROM_COUNTER_HEX: &str = include_str!("fixtures/eeprom_counter.hex");

// fixtures/eeprom_counter.hex: on startup reads a count from EEPROM[0], bumps it
// (mod 10), writes it back, and displays it — so each power-cycle (reset, which
// preserves EEPROM) advances the number. This is the persistence demonstration.
#[test]
fn eeprom_counter_increments_each_power_cycle() {
    let mut core = Core::new();
    core.load_hex(EEPROM_COUNTER_HEX).unwrap();
    core.run_cycles(300).unwrap(); // startup: read / increment / write
    let first = core.cpu().eeprom_byte(0);
    core.reset(); // power-cycle: EEPROM survives, RAM does not
    core.run_cycles(300).unwrap();
    let second = core.cpu().eeprom_byte(0);
    assert_eq!(second, first + 1, "the count must persist and advance");
}
