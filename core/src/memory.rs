//! PIC16F628A data memory: banking (RP0/RP1), shared SFRs, common RAM,
//! FSR/INDF indirection, and TRIS-aware I/O port reads.
//!
//! This is the address-resolution substrate the CPU reads/writes through. It is
//! exactly where naive emulators go subtly wrong (architecture.md §3), so the
//! banking, the registers that mirror into every bank, indirect addressing, and
//! the input-vs-latch behavior of the I/O ports are modeled here and tested.
//!
//! Port model: a write to PORTx sets the output latch. A read of PORTx returns,
//! per bit, the latch (for pins configured as outputs by TRIS=0) or the
//! externally-driven level (for inputs, TRIS=1). The external levels are set via
//! `set_ext` (driven by buttons through `Core::set_pin`).

/// INDF (indirect) — accessing this address redirects through FSR.
pub const INDF: u8 = 0x00;
pub const TMR0: u8 = 0x01;
pub const PCL: u8 = 0x02;
pub const STATUS: u8 = 0x03;
pub const FSR: u8 = 0x04;
pub const PORTA: u8 = 0x05;
pub const PORTB: u8 = 0x06;
pub const PCLATH: u8 = 0x0A;
pub const INTCON: u8 = 0x0B;

// STATUS register bit positions.
pub const STATUS_C: u8 = 0;
pub const STATUS_DC: u8 = 1;
pub const STATUS_Z: u8 = 2;
pub const STATUS_PD: u8 = 3;
pub const STATUS_TO: u8 = 4;
pub const STATUS_RP0: u8 = 5;
pub const STATUS_RP1: u8 = 6;
pub const STATUS_IRP: u8 = 7;

/// Registers that are physically the same cell in every bank (the mid-range
/// "mapped in all banks" set): PCL, STATUS, FSR, PCLATH, INTCON.
const SHARED: [u8; 5] = [PCL, STATUS, FSR, PCLATH, INTCON];

/// 4 banks x 128 bytes = 512 physical cells (9-bit address space).
const CELLS: usize = 512;

// Physical addresses of the I/O registers we special-case.
const PORTA_PHYS: usize = 0x05;
const PORTB_PHYS: usize = 0x06;
const TRISA_PHYS: usize = 0x85; // bank 1, offset 0x05
const TRISB_PHYS: usize = 0x86; // bank 1, offset 0x06

/// Banked data memory with shared-register/common-RAM aliasing and TRIS-aware
/// I/O ports.
#[derive(Debug, Clone)]
pub struct DataMem {
    cells: [u8; CELLS],
    /// Externally-driven pin levels (buttons), one bit per pin.
    ext_a: u8,
    ext_b: u8,
}

impl DataMem {
    pub fn new() -> Self {
        DataMem {
            cells: [0; CELLS],
            ext_a: 0,
            ext_b: 0,
        }
    }

    /// Current bank (0..=3) from STATUS RP1:RP0 for *direct* addressing.
    pub fn bank(&self) -> u8 {
        let s = self.cells[STATUS as usize];
        ((s >> STATUS_RP0) & 1) | (((s >> STATUS_RP1) & 1) << 1)
    }

    /// Resolve a 7-bit offset + bank to a physical cell index, honoring shared
    /// registers and common RAM (0x70..0x7F), both of which alias to bank 0.
    fn phys(&self, off: u8, bank: u8) -> usize {
        let off = off & 0x7F;
        if SHARED.contains(&off) {
            return off as usize;
        }
        if off >= 0x70 {
            return off as usize; // common RAM, same cell in every bank
        }
        (bank as usize) * 0x80 + off as usize
    }

    /// Effective PORTA value: latch for output bits, external level for inputs.
    pub fn port_a(&self) -> u8 {
        let tris = self.cells[TRISA_PHYS];
        (self.cells[PORTA_PHYS] & !tris) | (self.ext_a & tris)
    }

    /// Effective PORTB value: latch for output bits, external level for inputs.
    pub fn port_b(&self) -> u8 {
        let tris = self.cells[TRISB_PHYS];
        (self.cells[PORTB_PHYS] & !tris) | (self.ext_b & tris)
    }

    fn read_phys(&self, p: usize) -> u8 {
        match p {
            PORTA_PHYS => self.port_a(),
            PORTB_PHYS => self.port_b(),
            _ => self.cells[p],
        }
    }

    /// Direct read using the current bank. INDF redirects through FSR; reads of
    /// PORTA/PORTB return the effective (TRIS-aware) pin value.
    pub fn read(&self, off: u8) -> u8 {
        if off & 0x7F == INDF {
            return self.read_indirect();
        }
        self.read_phys(self.phys(off, self.bank()))
    }

    /// Direct write using the current bank. INDF redirects through FSR. Writes to
    /// PORTA/PORTB land in the output latch (the cell).
    pub fn write(&mut self, off: u8, val: u8) {
        if off & 0x7F == INDF {
            self.write_indirect(val);
            return;
        }
        let p = self.phys(off, self.bank());
        self.cells[p] = val;
    }

    fn indirect_phys(&self) -> Option<usize> {
        let fsr = self.cells[FSR as usize];
        if fsr & 0x7F == 0 {
            return None;
        }
        let irp = (self.cells[STATUS as usize] >> STATUS_IRP) & 1;
        let bank = (irp << 1) | ((fsr >> 7) & 1);
        Some(self.phys(fsr & 0x7F, bank))
    }

    fn read_indirect(&self) -> u8 {
        match self.indirect_phys() {
            Some(p) => self.read_phys(p),
            None => 0,
        }
    }

    fn write_indirect(&mut self, val: u8) {
        if let Some(p) = self.indirect_phys() {
            self.cells[p] = val;
        }
    }

    // --- external input levels (buttons) ---
    pub fn set_ext(&mut self, port_b: bool, bit: u8, level: bool) {
        let reg = if port_b { &mut self.ext_b } else { &mut self.ext_a };
        if level {
            *reg |= 1u8 << bit;
        } else {
            *reg &= !(1u8 << bit);
        }
    }
    pub fn ext_level(&self, port_b: bool, bit: u8) -> bool {
        let reg = if port_b { self.ext_b } else { self.ext_a };
        (reg >> bit) & 1 == 1
    }

    // --- raw access for the CPU and tests (bypasses banking and port logic) ---
    pub fn phys_read(&self, p: usize) -> u8 {
        self.cells[p]
    }
    pub fn phys_write(&mut self, p: usize, v: u8) {
        self.cells[p] = v;
    }
    pub fn status(&self) -> u8 {
        self.cells[STATUS as usize]
    }
    pub fn set_status(&mut self, v: u8) {
        self.cells[STATUS as usize] = v;
    }

    /// Read STATUS bit `bit` (use the `STATUS_*` constants).
    pub fn flag(&self, bit: u8) -> bool {
        (self.status() >> bit) & 1 != 0
    }

    /// Set or clear STATUS bit `bit`.
    pub fn set_flag(&mut self, bit: u8, v: bool) {
        let mut s = self.status();
        if v {
            s |= 1u8 << bit;
        } else {
            s &= !(1u8 << bit);
        }
        self.set_status(s);
    }
}

impl Default for DataMem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_switching_isolates_gpr() {
        let mut m = DataMem::new();
        m.write(0x20, 0xAA); // bank 0 GPR cell
        m.set_status(1 << STATUS_RP0); // select bank 1
        m.write(0x20, 0xBB); // bank 1 GPR is a *different* cell
        assert_eq!(m.read(0x20), 0xBB);
        m.set_status(0); // back to bank 0
        assert_eq!(m.read(0x20), 0xAA);
    }

    #[test]
    fn shared_register_visible_in_all_banks() {
        let mut m = DataMem::new();
        m.write(PCLATH, 0x12); // bank 0
        m.set_status(1 << STATUS_RP0); // bank 1
        assert_eq!(m.read(PCLATH), 0x12); // PCLATH is shared across banks
    }

    #[test]
    fn common_ram_aliases_across_banks() {
        let mut m = DataMem::new();
        m.write(0x70, 0x55); // bank 0
        m.set_status(1 << STATUS_RP1); // bank 2
        assert_eq!(m.read(0x70), 0x55); // 0x70..0x7F is common RAM
    }

    #[test]
    fn indf_reads_and_writes_through_fsr() {
        let mut m = DataMem::new();
        m.write(FSR, 0x20); // point FSR at 0x20
        m.write(INDF, 0x99); // indirect write
        assert_eq!(m.read(0x20), 0x99); // landed at 0x20
        assert_eq!(m.read(INDF), 0x99); // INDF reads it back
    }

    #[test]
    fn flag_set_and_clear() {
        let mut m = DataMem::new();
        m.set_flag(STATUS_C, true);
        assert!(m.flag(STATUS_C));
        m.set_flag(STATUS_C, false);
        assert!(!m.flag(STATUS_C));
    }

    #[test]
    fn input_pin_reads_external_output_pin_reads_latch() {
        let mut m = DataMem::new();
        // RB0 input: reads the externally-driven level, not the latch.
        m.phys_write(TRISB_PHYS, 0x01);
        m.set_ext(true, 0, true);
        m.write(PORTB, 0x00); // latch low
        assert_eq!(m.read(PORTB) & 1, 1); // pin reads external high

        // RB0 output: reads the latch, ignores the external level.
        m.phys_write(TRISB_PHYS, 0x00);
        m.write(PORTB, 0x00);
        m.set_ext(true, 0, true);
        assert_eq!(m.read(PORTB) & 1, 0); // pin reads latch low
    }
}
