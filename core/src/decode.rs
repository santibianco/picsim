//! Instruction decode + disassembly for the PIC16F628A (PIC14 mid-range core).
//!
//! The mid-range core has exactly **35 instructions** encoded in a 14-bit word.
//! Decode is the load-bearing foundation of the whole simulator: a single wrong
//! opcode bit silently poisons everything downstream (this is the class of bug
//! that made SimulIDE untrustworthy). So the table below is explicit and
//! table-driven, every entry is covered by tests, and the operand extraction is
//! kept dead simple.
//!
//! Encodings follow the Microchip mid-range reference (PIC16F627A/628A/648A data
//! sheet, "Instruction Set Summary"). Field layout within the 14-bit word:
//!   - byte-oriented: `00 oooo dfff ffff`  (d = dest bit 7, f = file bits 6..0)
//!   - bit-oriented:  `01 oobb bfff ffff`  (b = bit 9..7, f = file 6..0)
//!   - CALL/GOTO:     `10 ikkk kkkk kkkk`  (k = 11-bit address)
//!   - literal:       `11 oooo kkkk kkkk`  (k = 8-bit literal)

/// One of the 35 mid-range mnemonics, plus `Unknown` for reserved/illegal words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mnemonic {
    // Byte-oriented file register operations
    Addwf, Andwf, Clrf, Clrw, Comf, Decf, Decfsz, Incf, Incfsz,
    Iorwf, Movf, Movwf, Nop, Rlf, Rrf, Subwf, Swapf, Xorwf,
    // Bit-oriented file register operations
    Bcf, Bsf, Btfsc, Btfss,
    // Literal and control operations
    Addlw, Andlw, Call, Clrwdt, Goto, Iorlw, Movlw, Retfie, Retlw,
    Return, Sleep, Sublw, Xorlw,
    // Not a valid mid-range encoding (reserved hole)
    Unknown,
}

/// Cycle cost. One instruction cycle = 4 oscillator clocks (1 us at 4 MHz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleCost {
    /// Always one instruction cycle.
    One,
    /// Always two cycles: GOTO, CALL, RETURN, RETLW, RETFIE.
    Two,
    /// One cycle normally; two when the conditional skip is taken
    /// (BTFSC, BTFSS, INCFSZ, DECFSZ). Resolved at execute time.
    OneOrTwoIfSkip,
}

/// A fully decoded instruction. Fields not used by a given mnemonic are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// The raw 14-bit opcode word.
    pub word: u16,
    pub mnemonic: Mnemonic,
    /// File register address (7-bit) for byte/bit-oriented ops.
    pub f: u8,
    /// Destination select (`d` bit): false = W, true = file register.
    pub dest_f: bool,
    /// Bit number 0..=7 for bit-oriented ops.
    pub bit: u8,
    /// 8-bit literal, or 11-bit target address for CALL/GOTO.
    pub k: u16,
    pub cycles: CycleCost,
}

#[derive(Debug, Clone, Copy)]
enum Fmt {
    /// d + f (byte-oriented)
    ByteFd,
    /// f only (MOVWF, CLRF)
    FileF,
    /// no operand
    NoArg,
    /// b + f (bit-oriented)
    Bit,
    /// 8-bit literal
    Lit8,
    /// 11-bit address (CALL/GOTO)
    Addr11,
}

struct Entry {
    mask: u16,
    val: u16,
    m: Mnemonic,
    fmt: Fmt,
}

// Scanned top-to-bottom; first match wins. Ordering matters: the most specific
// patterns (exact no-arg words, then 7-bit opcodes) come before the broader
// 6-bit byte ops, bit ops, control, and literals.
const TABLE: &[Entry] = &[
    // --- exact, no operand ---
    Entry { mask: 0x3FFF, val: 0x0008, m: Mnemonic::Return, fmt: Fmt::NoArg },
    Entry { mask: 0x3FFF, val: 0x0009, m: Mnemonic::Retfie, fmt: Fmt::NoArg },
    Entry { mask: 0x3FFF, val: 0x0063, m: Mnemonic::Sleep,  fmt: Fmt::NoArg },
    Entry { mask: 0x3FFF, val: 0x0064, m: Mnemonic::Clrwdt, fmt: Fmt::NoArg },
    // NOP: 00 0000 0xx0 0000  -> 0x0000/0x0020/0x0040/0x0060
    Entry { mask: 0x3F9F, val: 0x0000, m: Mnemonic::Nop,    fmt: Fmt::NoArg },
    // --- 7-bit opcode ---
    Entry { mask: 0x3F80, val: 0x0100, m: Mnemonic::Clrw,  fmt: Fmt::NoArg },
    Entry { mask: 0x3F80, val: 0x0080, m: Mnemonic::Movwf, fmt: Fmt::FileF },
    Entry { mask: 0x3F80, val: 0x0180, m: Mnemonic::Clrf,  fmt: Fmt::FileF },
    // --- 6-bit opcode, byte-oriented (d + f) ---
    Entry { mask: 0x3F00, val: 0x0700, m: Mnemonic::Addwf,  fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0500, m: Mnemonic::Andwf,  fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0900, m: Mnemonic::Comf,   fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0300, m: Mnemonic::Decf,   fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0B00, m: Mnemonic::Decfsz, fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0A00, m: Mnemonic::Incf,   fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0F00, m: Mnemonic::Incfsz, fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0400, m: Mnemonic::Iorwf,  fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0800, m: Mnemonic::Movf,   fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0D00, m: Mnemonic::Rlf,    fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0C00, m: Mnemonic::Rrf,    fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0200, m: Mnemonic::Subwf,  fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0E00, m: Mnemonic::Swapf,  fmt: Fmt::ByteFd },
    Entry { mask: 0x3F00, val: 0x0600, m: Mnemonic::Xorwf,  fmt: Fmt::ByteFd },
    // --- bit-oriented (b + f) ---
    Entry { mask: 0x3C00, val: 0x1000, m: Mnemonic::Bcf,   fmt: Fmt::Bit },
    Entry { mask: 0x3C00, val: 0x1400, m: Mnemonic::Bsf,   fmt: Fmt::Bit },
    Entry { mask: 0x3C00, val: 0x1800, m: Mnemonic::Btfsc, fmt: Fmt::Bit },
    Entry { mask: 0x3C00, val: 0x1C00, m: Mnemonic::Btfss, fmt: Fmt::Bit },
    // --- control with 11-bit address ---
    Entry { mask: 0x3800, val: 0x2000, m: Mnemonic::Call, fmt: Fmt::Addr11 },
    Entry { mask: 0x3800, val: 0x2800, m: Mnemonic::Goto, fmt: Fmt::Addr11 },
    // --- literal (8-bit k) ---
    Entry { mask: 0x3C00, val: 0x3000, m: Mnemonic::Movlw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3C00, val: 0x3400, m: Mnemonic::Retlw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3F00, val: 0x3800, m: Mnemonic::Iorlw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3F00, val: 0x3900, m: Mnemonic::Andlw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3F00, val: 0x3A00, m: Mnemonic::Xorlw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3E00, val: 0x3C00, m: Mnemonic::Sublw, fmt: Fmt::Lit8 },
    Entry { mask: 0x3E00, val: 0x3E00, m: Mnemonic::Addlw, fmt: Fmt::Lit8 },
];

/// Decode a 14-bit program word into an [`Instruction`]. Always succeeds;
/// reserved encodings come back as [`Mnemonic::Unknown`].
pub fn decode(word: u16) -> Instruction {
    let w = word & 0x3FFF;
    for e in TABLE {
        if w & e.mask == e.val {
            let mut f = 0u8;
            let mut dest_f = false;
            let mut bit = 0u8;
            let mut k = 0u16;
            match e.fmt {
                Fmt::ByteFd => {
                    f = (w & 0x7F) as u8;
                    dest_f = (w & 0x80) != 0;
                }
                Fmt::FileF => {
                    f = (w & 0x7F) as u8;
                    dest_f = true; // MOVWF/CLRF write the file register
                }
                Fmt::NoArg => {}
                Fmt::Bit => {
                    f = (w & 0x7F) as u8;
                    bit = ((w >> 7) & 0x07) as u8;
                }
                Fmt::Lit8 => k = w & 0x00FF,
                Fmt::Addr11 => k = w & 0x07FF,
            }
            return Instruction { word, mnemonic: e.m, f, dest_f, bit, k, cycles: cost(e.m) };
        }
    }
    Instruction {
        word,
        mnemonic: Mnemonic::Unknown,
        f: 0,
        dest_f: false,
        bit: 0,
        k: 0,
        cycles: CycleCost::One,
    }
}

fn cost(m: Mnemonic) -> CycleCost {
    use Mnemonic::*;
    match m {
        Goto | Call | Return | Retlw | Retfie => CycleCost::Two,
        Btfsc | Btfss | Incfsz | Decfsz => CycleCost::OneOrTwoIfSkip,
        _ => CycleCost::One,
    }
}

/// Uppercase mnemonic string (`"ADDWF"`, `"BSF"`, ...). `Unknown` -> `"???"`.
pub fn mnemonic_str(m: Mnemonic) -> &'static str {
    use Mnemonic::*;
    match m {
        Addwf => "ADDWF", Andwf => "ANDWF", Clrf => "CLRF", Clrw => "CLRW",
        Comf => "COMF", Decf => "DECF", Decfsz => "DECFSZ", Incf => "INCF",
        Incfsz => "INCFSZ", Iorwf => "IORWF", Movf => "MOVF", Movwf => "MOVWF",
        Nop => "NOP", Rlf => "RLF", Rrf => "RRF", Subwf => "SUBWF",
        Swapf => "SWAPF", Xorwf => "XORWF",
        Bcf => "BCF", Bsf => "BSF", Btfsc => "BTFSC", Btfss => "BTFSS",
        Addlw => "ADDLW", Andlw => "ANDLW", Call => "CALL", Clrwdt => "CLRWDT",
        Goto => "GOTO", Iorlw => "IORLW", Movlw => "MOVLW", Retfie => "RETFIE",
        Retlw => "RETLW", Return => "RETURN", Sleep => "SLEEP", Sublw => "SUBLW",
        Xorlw => "XORLW",
        Unknown => "???",
    }
}

/// Disassemble a word to a readable string, e.g. `"BSF 0x03, 5"`,
/// `"MOVLW 0x0A"`, `"GOTO 0x123"`. Useful for eyeballing a parsed `.hex`
/// against its `.asm` source. Reserved words render as `"DW 0x....".`
pub fn disassemble(word: u16) -> String {
    use Mnemonic::*;
    let i = decode(word);
    let name = mnemonic_str(i.mnemonic);
    match i.mnemonic {
        Nop | Clrw | Clrwdt | Return | Retfie | Sleep => name.to_string(),
        Unknown => format!("DW 0x{:04X}", word & 0x3FFF),
        Addwf | Andwf | Comf | Decf | Decfsz | Incf | Incfsz | Iorwf | Movf
        | Rlf | Rrf | Subwf | Swapf | Xorwf => {
            format!("{} 0x{:02X}, {}", name, i.f, if i.dest_f { "F" } else { "W" })
        }
        Movwf | Clrf => format!("{} 0x{:02X}", name, i.f),
        Bcf | Bsf | Btfsc | Btfss => format!("{} 0x{:02X}, {}", name, i.f, i.bit),
        Movlw | Retlw | Iorlw | Andlw | Xorlw | Sublw | Addlw => {
            format!("{} 0x{:02X}", name, i.k)
        }
        Call | Goto => format!("{} 0x{:03X}", name, i.k),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(w: u16) -> Mnemonic {
        decode(w).mnemonic
    }

    #[test]
    fn bsf_bcf_and_bank_select_decode() {
        // The exact instructions SimulIDE got wrong, plus the bank-select idiom.
        let bsf = decode(0x1683);
        assert_eq!(bsf.mnemonic, Mnemonic::Bsf);
        assert_eq!(bsf.f, 0x03); // STATUS
        assert_eq!(bsf.bit, 5); // RP0
        let bcf = decode(0x1283);
        assert_eq!(bcf.mnemonic, Mnemonic::Bcf);
        assert_eq!(bcf.bit, 5);
        assert_eq!(disassemble(0x1683), "BSF 0x03, 5");
        assert_eq!(disassemble(0x1283), "BCF 0x03, 5");
    }

    #[test]
    fn byte_oriented_d_and_f() {
        let f_dest = decode(0x0780); // ADDWF 0x00, F
        assert_eq!(f_dest.mnemonic, Mnemonic::Addwf);
        assert_eq!(f_dest.f, 0x00);
        assert!(f_dest.dest_f);
        let w_dest = decode(0x0700); // ADDWF 0x00, W
        assert!(!w_dest.dest_f);
        assert_eq!(disassemble(0x0A86), "INCF 0x06, F");
    }

    #[test]
    fn literal_and_control() {
        assert_eq!(decode(0x300A).mnemonic, Mnemonic::Movlw);
        assert_eq!(decode(0x300A).k, 0x0A);
        assert_eq!(decode(0x2803).mnemonic, Mnemonic::Goto);
        assert_eq!(decode(0x2803).k, 0x003);
        assert_eq!(decode(0x2000).mnemonic, Mnemonic::Call);
        assert_eq!(disassemble(0x2803), "GOTO 0x003");
        assert_eq!(disassemble(0x300A), "MOVLW 0x0A");
    }

    #[test]
    fn special_no_arg_ops() {
        assert_eq!(m(0x0000), Mnemonic::Nop);
        assert_eq!(m(0x0060), Mnemonic::Nop);
        assert_eq!(m(0x0008), Mnemonic::Return);
        assert_eq!(m(0x0009), Mnemonic::Retfie);
        assert_eq!(m(0x0063), Mnemonic::Sleep);
        assert_eq!(m(0x0064), Mnemonic::Clrwdt);
        assert_eq!(m(0x0100), Mnemonic::Clrw);
        assert_eq!(m(0x0086), Mnemonic::Movwf);
        assert_eq!(m(0x0186), Mnemonic::Clrf);
    }

    #[test]
    fn cycle_costs() {
        assert_eq!(decode(0x2803).cycles, CycleCost::Two); // GOTO
        assert_eq!(decode(0x2000).cycles, CycleCost::Two); // CALL
        assert_eq!(decode(0x0008).cycles, CycleCost::Two); // RETURN
        assert_eq!(decode(0x340A).cycles, CycleCost::Two); // RETLW
        assert_eq!(decode(0x1800).cycles, CycleCost::OneOrTwoIfSkip); // BTFSC
        assert_eq!(decode(0x0B00).cycles, CycleCost::OneOrTwoIfSkip); // DECFSZ
        assert_eq!(decode(0x0700).cycles, CycleCost::One); // ADDWF
    }

    #[test]
    fn table_holds_all_35_distinct_mnemonics() {
        let mut set = std::collections::HashSet::new();
        for e in TABLE {
            set.insert(e.m);
        }
        assert_eq!(set.len(), 35);
    }

    #[test]
    fn reserved_word_is_unknown() {
        assert_eq!(m(0x3B00), Mnemonic::Unknown); // 11 1011 .... reserved hole
        assert_eq!(disassemble(0x3B00), "DW 0x3B00");
    }
}
