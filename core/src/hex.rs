//! Intel HEX loader for PIC16F628A firmware (as produced by MPLAB).
//!
//! Firmware enters the simulator as Intel HEX — the IDE is the toolchain, no
//! assembler is built here. We parse the records into the 2K (2048-word) program
//! image, assembling 14-bit instruction words little-endian (low byte first),
//! and capture the configuration word (0x2007) and any data-EEPROM bytes.
//!
//! Record types handled: 00 (data), 01 (EOF), 02 (extended segment address),
//! 04 (extended linear address). Checksums are verified; a corrupt record is
//! rejected rather than silently loaded.

use std::collections::BTreeMap;

/// Program flash size in 14-bit words (0x000..=0x7FF).
pub const PROG_WORDS: usize = 2048;
/// Erased flash reads as all-ones in 14 bits.
pub const BLANK_WORD: u16 = 0x3FFF;
/// The configuration word lives at program (word) address 0x2007 == byte 0x400E.
pub const CONFIG_BYTE_ADDR: u32 = 0x400E;
/// Data EEPROM is mapped at word 0x2100 == byte 0x4200 in the HEX file.
const EEPROM_BYTE_BASE: u32 = 0x4200;
const EEPROM_BYTES: u32 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexError {
    /// No data record was seen before EOF / end of input.
    NoData,
    /// A non-empty line did not start with ':'.
    MissingColon(usize),
    /// Odd length, too short, or byte count mismatch (line number).
    BadLength(usize),
    /// A non-hex character appeared in a record (line number).
    BadHexDigit(usize),
    /// Record checksum did not validate.
    BadChecksum { line: usize, expected: u8, found: u8 },
    /// A record type we do not handle appeared (line number + type).
    UnsupportedRecord { line: usize, kind: u8 },
}

/// A parsed firmware image.
#[derive(Debug, Clone)]
pub struct HexImage {
    /// Program memory, `PROG_WORDS` words; untouched cells are `BLANK_WORD`.
    pub program: Vec<u16>,
    /// Configuration word at 0x2007, if present in the HEX.
    pub config: Option<u16>,
    /// Data-EEPROM bytes by offset (0..=127), if present.
    pub eeprom: BTreeMap<u8, u8>,
}

/// Parse Intel HEX text into a [`HexImage`].
pub fn parse(text: &str) -> Result<HexImage, HexError> {
    let mut bytes: BTreeMap<u32, u8> = BTreeMap::new();
    let mut base: u32 = 0;
    let mut saw_data = false;

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let line_no = idx + 1;
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(':') {
            return Err(HexError::MissingColon(line_no));
        }
        let body = &line[1..];
        if body.len() < 10 || body.len() % 2 != 0 {
            return Err(HexError::BadLength(line_no));
        }
        let rb = decode_hex(body).map_err(|_| HexError::BadHexDigit(line_no))?;
        let count = rb[0] as usize;
        if rb.len() != count + 5 {
            return Err(HexError::BadLength(line_no));
        }
        // Checksum: the sum of every byte (including the checksum) is 0 mod 256.
        let sum = rb.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        if sum != 0 {
            let found = rb[rb.len() - 1];
            return Err(HexError::BadChecksum {
                line: line_no,
                expected: found.wrapping_sub(sum),
                found,
            });
        }
        let addr = ((rb[1] as u32) << 8) | rb[2] as u32;
        let rtype = rb[3];
        let data = &rb[4..4 + count];
        match rtype {
            0x00 => {
                saw_data = true;
                for (i, &b) in data.iter().enumerate() {
                    bytes.insert(base + addr + i as u32, b);
                }
            }
            0x01 => break, // EOF
            0x02 => {
                if data.len() < 2 {
                    return Err(HexError::BadLength(line_no));
                }
                base = (((data[0] as u32) << 8) | data[1] as u32) << 4;
            }
            0x04 => {
                if data.len() < 2 {
                    return Err(HexError::BadLength(line_no));
                }
                base = (((data[0] as u32) << 8) | data[1] as u32) << 16;
            }
            0x03 | 0x05 => { /* start-address records: irrelevant here, ignore */ }
            other => {
                return Err(HexError::UnsupportedRecord { line: line_no, kind: other });
            }
        }
    }

    if !saw_data {
        return Err(HexError::NoData);
    }

    // Assemble program words (little-endian) from the byte map.
    let mut program = vec![BLANK_WORD; PROG_WORDS];
    for (w, slot) in program.iter_mut().enumerate() {
        let lo_addr = (w as u32) * 2;
        let lo = bytes.get(&lo_addr).copied();
        let hi = bytes.get(&(lo_addr + 1)).copied();
        if lo.is_some() || hi.is_some() {
            let word = (lo.unwrap_or(0xFF) as u16) | ((hi.unwrap_or(0xFF) as u16) << 8);
            *slot = word & 0x3FFF;
        }
    }

    // Configuration word at 0x2007 (bytes 0x400E / 0x400F).
    let clo = bytes.get(&CONFIG_BYTE_ADDR).copied();
    let chi = bytes.get(&(CONFIG_BYTE_ADDR + 1)).copied();
    let config = if clo.is_some() || chi.is_some() {
        Some(((clo.unwrap_or(0xFF) as u16) | ((chi.unwrap_or(0xFF) as u16) << 8)) & 0x3FFF)
    } else {
        None
    };

    // Data EEPROM: each byte is stored in the low byte of a HEX word.
    let mut eeprom: BTreeMap<u8, u8> = BTreeMap::new();
    for (&a, &b) in bytes.iter() {
        if a >= EEPROM_BYTE_BASE && a < EEPROM_BYTE_BASE + EEPROM_BYTES * 2 && a % 2 == 0 {
            eeprom.insert(((a - EEPROM_BYTE_BASE) / 2) as u8, b);
        }
    }

    Ok(HexImage { program, config, eeprom })
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ()> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i + 1 < b.len() {
        out.push((hexval(b[i])? << 4) | hexval(b[i + 1])?);
        i += 2;
    }
    Ok(out)
}

fn hexval(c: u8) -> Result<u8, ()> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The self-bootstrap fixture: increment PORTB forever (5 words).
    const BLINK: &str = ":0A000000831686018312860A032886\n:00000001FF\n";

    #[test]
    fn parses_blink_program() {
        let img = parse(BLINK).unwrap();
        assert_eq!(
            img.program[0..5].to_vec(),
            vec![0x1683u16, 0x0186, 0x1283, 0x0A86, 0x2803]
        );
        assert_eq!(img.program[5], BLANK_WORD); // rest is erased flash
        assert!(img.config.is_none());
        assert!(img.eeprom.is_empty());
    }

    #[test]
    fn rejects_bad_checksum() {
        // Same as BLINK but the final checksum byte is wrong (0x99 vs 0x86).
        let bad = ":0A000000831686018312860A032899\n:00000001FF\n";
        assert!(matches!(parse(bad), Err(HexError::BadChecksum { .. })));
    }

    #[test]
    fn requires_leading_colon() {
        assert!(matches!(parse("0A0000zz\n"), Err(HexError::MissingColon(1))));
    }

    #[test]
    fn eof_only_is_no_data() {
        assert!(matches!(parse(":00000001FF\n"), Err(HexError::NoData)));
    }

    #[test]
    fn parses_config_word() {
        // INHX32: extended-linear-address 0x0000, then config 0x3F21 at 0x400E.
        let hexs = ":020000040000FA\n:02400E00213F50\n:00000001FF\n";
        let img = parse(hexs).unwrap();
        assert_eq!(img.config, Some(0x3F21));
    }
}
