//! CPU: state, power-on reset, fetch, full instruction execute, the TMR0 tick
//! and interrupt check after every instruction, per-pin on-time sampling, and
//! external input-pin drive (`set_pin`).
//!
//! Execute implements all 35 mid-range instructions: W/file writes, the STATUS
//! flags (Z, plus the C/DC borrow semantics on subtraction that trip up naive
//! emulators), the 8-level hardware stack, computed jumps via PCL/PCLATH,
//! GOTO/CALL paging, and the conditional-skip extra cycle. After each
//! instruction TMR0 advances (architecture.md §2 step 5), interrupts are checked
//! (step 6), and per-pin on-time is integrated for persistence of vision (§4).
//! Cycle counts are exact — 1 per instruction; 2 for branches, taken skips, and
//! PCL writes.

use crate::decode::{decode, Instruction, Mnemonic};
use crate::hex::{HexImage, BLANK_WORD, PROG_WORDS};
use crate::interrupts;
use crate::memory::{
    DataMem, INTCON, PCL, PCLATH, STATUS_C, STATUS_DC, STATUS_PD, STATUS_TO, STATUS_Z, TMR0,
};
use crate::sampler::PinSampler;
use crate::timer::Timer0;

const PC_MASK: u16 = 0x1FFF; // 13-bit program counter
const TRISA_PHYS: usize = 0x85;
const TRISB_PHYS: usize = 0x86;
const OPTION_PHYS: usize = 0x81;
const EEDATA_PHYS: usize = 0x9A;
const EEADR_PHYS: usize = 0x9B;
const EECON1_PHYS: usize = 0x9C;
const EECON2_PHYS: usize = 0x9D;
const EECON1_OFF: u8 = 0x1C; // bank-1 offset of EECON1
const EECON2_OFF: u8 = 0x1D; // bank-1 offset of EECON2
const EE_RD: u8 = 0;
const EE_WR: u8 = 1;
const EE_WREN: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    /// PC left valid program memory (16F628A has 2K words, 0x000..=0x7FF).
    PcOutOfRange(u16),
}

pub struct Cpu {
    /// Working register (not memory-mapped).
    pub w: u8,
    /// 13-bit program counter.
    pub pc: u16,
    /// Instruction cycles elapsed since reset (the master clock).
    pub cycles: u64,
    /// Program flash (`PROG_WORDS` words).
    pub program: Vec<u16>,
    /// Data memory (banked).
    pub mem: DataMem,
    /// Configuration word from the loaded HEX, if any.
    pub config: Option<u16>,
    /// 8-level hardware return stack.
    stack: [u16; 8],
    sp: u8,
    /// TMR0 + prescaler.
    timer: Timer0,
    /// Per-pin on-time accumulators (persistence of vision).
    sampler: PinSampler,
    /// Non-volatile 128-byte data EEPROM (survives reset / power cycle).
    eeprom: [u8; 128],
    /// EECON2 write-unlock progress: 0 idle, 1 saw 0x55, 2 armed (0x55 then 0xAA).
    ee_unlock: u8,
    /// Debugger breakpoints — one flag per program word (set via the runtime).
    breaks: Vec<bool>,
    /// Set by the scheduler when a run stopped at a breakpoint (cleared each run).
    pub break_hit: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            w: 0,
            pc: 0,
            cycles: 0,
            program: vec![BLANK_WORD; PROG_WORDS],
            mem: DataMem::new(),
            config: None,
            stack: [0; 8],
            sp: 0,
            timer: Timer0::new(),
            sampler: PinSampler::new(),
            eeprom: [0xFF; 128],
            ee_unlock: 0,
            breaks: vec![false; PROG_WORDS],
            break_hit: false,
        }
    }

    /// Install a parsed firmware image and power-on reset.
    pub fn load_image(&mut self, img: HexImage) {
        self.program = img.program;
        self.config = img.config;
        self.eeprom = [0xFF; 128];
        for (&addr, &val) in img.eeprom.iter() {
            self.eeprom[(addr & 0x7F) as usize] = val;
        }
        self.reset();
    }

    /// Power-on reset: PC=0, W cleared, SFRs to POR defaults, stack/timer/sampler
    /// cleared. TRISA/TRISB default to all-inputs (0xFF), as on real silicon.
    pub fn reset(&mut self) {
        self.pc = 0;
        self.w = 0;
        self.cycles = 0;
        self.mem = DataMem::new();
        self.mem.set_status(0x18); // POR: TO=1, PD=1
        self.mem.phys_write(TRISA_PHYS, 0xFF);
        self.mem.phys_write(TRISB_PHYS, 0xFF);
        self.stack = [0; 8];
        self.sp = 0;
        self.timer.reset();
        self.sampler.reset_frame();
        self.ee_unlock = 0; // EEPROM array persists (non-volatile) across reset
        self.break_hit = false; // breakpoints themselves persist across reset
    }

    /// Fetch the instruction word at PC (PC wraps within program flash).
    pub fn fetch(&self) -> u16 {
        self.program[(self.pc as usize) & (PROG_WORDS - 1)]
    }

    /// Decode the instruction currently at PC without executing it.
    pub fn decode_at_pc(&self) -> Instruction {
        decode(self.fetch())
    }

    /// Reset the per-frame on-time accumulators (call at the start of each
    /// rendered frame; architecture.md §5 `resetFrameAccumulators`).
    pub fn reset_frame(&mut self) {
        self.sampler.reset_frame();
    }

    /// The per-pin on-time sampler (architecture.md §4 persistence of vision).
    pub fn sampler(&self) -> &PinSampler {
        &self.sampler
    }

    /// Read a data-EEPROM byte (0..=127) — for inspection / the runtime / tests.
    pub fn eeprom_byte(&self, addr: u8) -> u8 {
        self.eeprom[(addr & 0x7F) as usize]
    }

    // ---- debugger breakpoints ----
    pub fn set_break(&mut self, addr: u16) { let a = addr as usize; if a < self.breaks.len() { self.breaks[a] = true; } }
    pub fn clear_break(&mut self, addr: u16) { let a = addr as usize; if a < self.breaks.len() { self.breaks[a] = false; } }
    pub fn clear_breaks(&mut self) { for b in self.breaks.iter_mut() { *b = false; } }
    pub fn is_break(&self, pc: u16) -> bool { let a = pc as usize; a < self.breaks.len() && self.breaks[a] }

    /// A write touched EECON1: service an EEPROM read (RD), or — if write-enabled
    /// and unlocked — an EEPROM write (WR). Both control bits clear when done.
    fn eecon1_write(&mut self) {
        let mut eecon1 = self.mem.phys_read(EECON1_PHYS);
        if eecon1 & (1 << EE_RD) != 0 {
            let addr = (self.mem.phys_read(EEADR_PHYS) & 0x7F) as usize;
            let data = self.eeprom[addr];
            self.mem.phys_write(EEDATA_PHYS, data);
            eecon1 &= !(1 << EE_RD);
            self.mem.phys_write(EECON1_PHYS, eecon1);
        }
        if eecon1 & (1 << EE_WR) != 0 {
            if eecon1 & (1 << EE_WREN) != 0 && self.ee_unlock == 2 {
                let addr = (self.mem.phys_read(EEADR_PHYS) & 0x7F) as usize;
                self.eeprom[addr] = self.mem.phys_read(EEDATA_PHYS);
            }
            eecon1 &= !(1 << EE_WR); // write completes immediately in the sim
            self.mem.phys_write(EECON1_PHYS, eecon1);
            self.ee_unlock = 0;
        }
    }

    /// A write touched EECON2: advance the 0x55 -> 0xAA write-unlock sequence.
    fn eecon2_write(&mut self) {
        match (self.ee_unlock, self.mem.phys_read(EECON2_PHYS)) {
            (0, 0x55) => self.ee_unlock = 1,
            (1, 0xAA) => self.ee_unlock = 2,
            _ => self.ee_unlock = 0,
        }
    }

    /// `setPin`: drive an external input pin (a button). Updates the pin's
    /// external level and, for input PORTB pins, raises the matching interrupt
    /// flag on the active edge (RB0/INT -> INTF per OPTION INTEDG; RB4..7 -> RBIF).
    pub fn set_pin(&mut self, pin: usize, level: bool) {
        if pin >= 16 {
            return;
        }
        let port_b = pin >= 8;
        let bit = (pin & 0x07) as u8;
        let prev = self.mem.ext_level(port_b, bit);
        self.mem.set_ext(port_b, bit, level);
        // Only a real change on an input PORTB pin can raise an interrupt flag.
        if prev == level || !port_b || (self.mem.phys_read(TRISB_PHYS) >> bit) & 1 == 0 {
            return;
        }
        if bit == 0 {
            // RB0/INT external interrupt, edge selected by OPTION INTEDG (bit 6).
            let rising = (self.mem.phys_read(OPTION_PHYS) >> 6) & 1 == 1;
            if level == rising {
                let v = self.mem.phys_read(INTCON as usize) | (1u8 << interrupts::INTF);
                self.mem.phys_write(INTCON as usize, v);
            }
        } else if (4..=7).contains(&bit) {
            // PORTB interrupt-on-change (RB4..RB7).
            let v = self.mem.phys_read(INTCON as usize) | (1u8 << interrupts::RBIF);
            self.mem.phys_write(INTCON as usize, v);
        }
    }

    // ---- helpers ----
    fn read_file(&self, f: u8) -> u8 {
        self.mem.read(f)
    }
    fn write_file(&mut self, f: u8, val: u8) {
        self.mem.write(f, val);
    }
    fn sync_pcl(&mut self) {
        self.mem.phys_write(PCL as usize, (self.pc & 0xFF) as u8);
    }
    fn push(&mut self, addr: u16) {
        self.stack[self.sp as usize] = addr & PC_MASK;
        self.sp = (self.sp + 1) & 0x07;
    }
    fn pop(&mut self) -> u16 {
        self.sp = self.sp.wrapping_sub(1) & 0x07;
        self.stack[self.sp as usize]
    }
    /// Hardware-stack depth (the stack pointer, 0..=7): how many return addresses
    /// are pushed. 0 = empty; wraps on overflow like the real 8-level stack.
    pub fn stack_depth(&self) -> u8 {
        self.sp
    }
    /// Return address held at hardware-stack level `i` (0..=7).
    pub fn stack_at(&self, i: usize) -> u16 {
        self.stack[i & 0x07]
    }
    fn set_z(&mut self, r: u8) {
        self.mem.set_flag(STATUS_Z, r == 0);
    }

    /// Write a byte-op result to W or the file register per the `d` bit. Returns
    /// true if the write targeted PCL (so the caller bills the extra cycle).
    fn write_dest(&mut self, instr: &Instruction, val: u8) -> bool {
        if instr.dest_f {
            self.write_file(instr.f, val);
            self.maybe_pcl_jump(instr.f)
        } else {
            self.w = val;
            false
        }
    }

    /// If `f` is PCL, reload PC from PCLATH<4:0>:PCL (a computed jump) and report
    /// it so the caller can bill the second cycle.
    fn maybe_pcl_jump(&mut self, f: u8) -> bool {
        if f & 0x7F == PCL {
            let lo = self.mem.phys_read(PCL as usize) as u16;
            let high = (self.mem.phys_read(PCLATH as usize) & 0x1F) as u16;
            self.pc = ((high << 8) | lo) & PC_MASK;
            true
        } else {
            false
        }
    }

    /// GOTO/CALL target: PC<10:0> = k; PC<12:11> = PCLATH<4:3>.
    fn branch_target(&self, k: u16) -> u16 {
        let high = ((self.mem.phys_read(PCLATH as usize) >> 3) & 0x03) as u16;
        ((high << 11) | (k & 0x07FF)) & PC_MASK
    }

    /// Acknowledge a pending interrupt: push PC, clear GIE, vector to 0x0004.
    /// Returns the entry-cycle cost (0 if none pending).
    fn service_interrupts(&mut self) -> u64 {
        let intcon = self.mem.phys_read(INTCON as usize);
        if !interrupts::pending(intcon) {
            return 0;
        }
        let ret = self.pc;
        self.push(ret);
        self.mem
            .phys_write(INTCON as usize, intcon & !(1u8 << interrupts::GIE));
        self.pc = interrupts::INT_VECTOR;
        interrupts::INTERRUPT_ENTRY_CYCLES
    }

    /// Execute one instruction; returns the decoded instruction it ran.
    pub fn step(&mut self) -> Result<Instruction, StepError> {
        if self.pc as usize >= PROG_WORDS {
            return Err(StepError::PcOutOfRange(self.pc));
        }
        let instr = self.decode_at_pc();
        // Advance past the fetched word first; PCL now reads as PC (= A + 1).
        self.pc = (self.pc + 1) & PC_MASK;
        self.sync_pcl();
        let bank = self.mem.bank(); // current bank (TMR0 / EEPROM write detection)
        // Pins held their pre-execution value during this instruction's cycles.
        let porta_pre = self.mem.port_a();
        let portb_pre = self.mem.port_b();
        let mut cyc: u64 = 1;

        use Mnemonic::*;
        match instr.mnemonic {
            // ---------- byte-oriented: arithmetic (Z, C, DC) ----------
            Addwf => {
                let (r, c, dc) = add8(self.read_file(instr.f), self.w);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
                self.mem.set_flag(STATUS_C, c);
                self.mem.set_flag(STATUS_DC, dc);
            }
            Subwf => {
                let (r, c, dc) = sub8(self.read_file(instr.f), self.w); // f - W
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
                self.mem.set_flag(STATUS_C, c);
                self.mem.set_flag(STATUS_DC, dc);
            }

            // ---------- byte-oriented: logic / move (Z only) ----------
            Andwf => {
                let r = self.read_file(instr.f) & self.w;
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Iorwf => {
                let r = self.read_file(instr.f) | self.w;
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Xorwf => {
                let r = self.read_file(instr.f) ^ self.w;
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Comf => {
                let r = !self.read_file(instr.f);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Movf => {
                let r = self.read_file(instr.f);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Incf => {
                let r = self.read_file(instr.f).wrapping_add(1);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Decf => {
                let r = self.read_file(instr.f).wrapping_sub(1);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.set_z(r);
            }
            Swapf => {
                let v = self.read_file(instr.f);
                let r = (v << 4) | (v >> 4);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
            }

            // ---------- byte-oriented: rotate through carry (C only) ----------
            Rlf => {
                let v = self.read_file(instr.f);
                let oldc = self.mem.flag(STATUS_C) as u8;
                let r = (v << 1) | oldc;
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.mem.set_flag(STATUS_C, (v >> 7) & 1 != 0);
            }
            Rrf => {
                let v = self.read_file(instr.f);
                let oldc = self.mem.flag(STATUS_C) as u8;
                let r = (v >> 1) | (oldc << 7);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                self.mem.set_flag(STATUS_C, v & 1 != 0);
            }

            // ---------- byte-oriented: skip-if-zero (no flags) ----------
            Incfsz => {
                let r = self.read_file(instr.f).wrapping_add(1);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                if r == 0 {
                    self.pc = (self.pc + 1) & PC_MASK;
                    cyc = 2;
                }
            }
            Decfsz => {
                let r = self.read_file(instr.f).wrapping_sub(1);
                if self.write_dest(&instr, r) {
                    cyc = 2;
                }
                if r == 0 {
                    self.pc = (self.pc + 1) & PC_MASK;
                    cyc = 2;
                }
            }

            // ---------- clears / moves ----------
            Clrf => {
                self.write_file(instr.f, 0);
                if self.maybe_pcl_jump(instr.f) {
                    cyc = 2;
                }
                self.mem.set_flag(STATUS_Z, true);
            }
            Clrw => {
                self.w = 0;
                self.mem.set_flag(STATUS_Z, true);
            }
            Movwf => {
                let w = self.w;
                self.write_file(instr.f, w);
                if self.maybe_pcl_jump(instr.f) {
                    cyc = 2;
                }
            }
            Nop => {}

            // ---------- bit-oriented ----------
            Bcf => {
                let v = self.read_file(instr.f) & !(1u8 << instr.bit);
                self.write_file(instr.f, v);
                if self.maybe_pcl_jump(instr.f) {
                    cyc = 2;
                }
            }
            Bsf => {
                let v = self.read_file(instr.f) | (1u8 << instr.bit);
                self.write_file(instr.f, v);
                if self.maybe_pcl_jump(instr.f) {
                    cyc = 2;
                }
            }
            Btfsc => {
                if (self.read_file(instr.f) >> instr.bit) & 1 == 0 {
                    self.pc = (self.pc + 1) & PC_MASK;
                    cyc = 2;
                }
            }
            Btfss => {
                if (self.read_file(instr.f) >> instr.bit) & 1 == 1 {
                    self.pc = (self.pc + 1) & PC_MASK;
                    cyc = 2;
                }
            }

            // ---------- literal ----------
            Movlw => self.w = instr.k as u8,
            Addlw => {
                let (r, c, dc) = add8(self.w, instr.k as u8);
                self.w = r;
                self.set_z(r);
                self.mem.set_flag(STATUS_C, c);
                self.mem.set_flag(STATUS_DC, dc);
            }
            Sublw => {
                let (r, c, dc) = sub8(instr.k as u8, self.w); // k - W
                self.w = r;
                self.set_z(r);
                self.mem.set_flag(STATUS_C, c);
                self.mem.set_flag(STATUS_DC, dc);
            }
            Andlw => {
                let r = self.w & instr.k as u8;
                self.w = r;
                self.set_z(r);
            }
            Iorlw => {
                let r = self.w | instr.k as u8;
                self.w = r;
                self.set_z(r);
            }
            Xorlw => {
                let r = self.w ^ instr.k as u8;
                self.w = r;
                self.set_z(r);
            }

            // ---------- control ----------
            Goto => {
                self.pc = self.branch_target(instr.k);
                cyc = 2;
            }
            Call => {
                let ret = self.pc; // already incremented: instruction after CALL
                self.push(ret);
                self.pc = self.branch_target(instr.k);
                cyc = 2;
            }
            Return => {
                self.pc = self.pop();
                cyc = 2;
            }
            Retlw => {
                self.w = instr.k as u8;
                self.pc = self.pop();
                cyc = 2;
            }
            Retfie => {
                self.pc = self.pop();
                let v = self.mem.phys_read(INTCON as usize) | (1u8 << interrupts::GIE);
                self.mem.phys_write(INTCON as usize, v);
                cyc = 2;
            }
            Clrwdt => {
                self.mem.set_flag(STATUS_TO, true);
                self.mem.set_flag(STATUS_PD, true);
            }
            Sleep => {
                // We don't model the sleep/wake states yet; record the status
                // bits and continue (LED/button labs don't rely on SLEEP).
                self.mem.set_flag(STATUS_TO, true);
                self.mem.set_flag(STATUS_PD, false);
            }
            Unknown => {} // reserved encoding: behave as NOP
        }

        // TMR0 (architecture.md §2 step 5): a write to TMR0 reloads it (clears
        // the prescaler, inhibits counting 2 cycles, and the write instruction
        // does not itself tick); otherwise advance by this instruction's cycles.
        let wrote = writes_file(&instr);
        let f7 = instr.f & 0x7F;
        if wrote && bank == 0 && f7 == TMR0 {
            self.timer.on_tmr0_write();
        } else {
            self.timer.tick(&mut self.mem, cyc);
        }
        self.cycles += cyc;
        // EEPROM control (bank 1): EECON1 read/write, EECON2 unlock sequence.
        if wrote && bank == 1 {
            if f7 == EECON1_OFF {
                self.eecon1_write();
            } else if f7 == EECON2_OFF {
                self.eecon2_write();
            }
        }

        // Interrupts (step 6): vector if an enabled source is flagged and GIE set.
        let entry = self.service_interrupts();
        self.cycles += entry;

        // Frame sampling (architecture.md §4): integrate per-pin on-time over the
        // cycles just elapsed, using the pin state that held during them.
        self.sampler.accumulate(porta_pre, portb_pre, cyc + entry);
        Ok(instr)
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

/// 8-bit add: returns (result, carry-out, digit-carry).
fn add8(a: u8, b: u8) -> (u8, bool, bool) {
    let sum = a as u16 + b as u16;
    let dc = (a & 0x0F) + (b & 0x0F) > 0x0F;
    ((sum & 0xFF) as u8, sum > 0xFF, dc)
}

/// 8-bit subtract `a - b`. On the PIC, C = NOT borrow (set when a >= b) and
/// DC = NOT digit borrow (set when low nibble of a >= low nibble of b).
fn sub8(a: u8, b: u8) -> (u8, bool, bool) {
    (a.wrapping_sub(b), a >= b, (a & 0x0F) >= (b & 0x0F))
}

/// True if this instruction writes its result back to a file register (used to
/// detect writes to TMR0, which reload its prescaler).
fn writes_file(instr: &Instruction) -> bool {
    use Mnemonic::*;
    match instr.mnemonic {
        Movwf | Clrf | Bcf | Bsf => true,
        Addwf | Subwf | Andwf | Iorwf | Xorwf | Comf | Movf | Incf | Decf | Swapf | Rlf | Rrf
        | Incfsz | Decfsz => instr.dest_f,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{STATUS_C, STATUS_DC, STATUS_Z};

    const OPTION_REG: usize = 0x81;
    const TMR0_ADDR: usize = 0x01;
    const INTCON_ADDR: usize = 0x0B;
    const TRISB_ADDR: usize = 0x86;

    fn cpu_with(words: &[u16]) -> Cpu {
        let mut c = Cpu::new();
        c.program = words.to_vec();
        c
    }

    #[test]
    fn por_reset_state() {
        let mut c = Cpu::new();
        c.reset();
        assert_eq!(c.pc, 0);
        assert_eq!(c.w, 0);
        assert_eq!(c.cycles, 0);
        assert_eq!(c.mem.status(), 0x18);
        assert_eq!(c.mem.phys_read(TRISB_ADDR), 0xFF); // POR: all inputs
    }

    #[test]
    fn movlw_loads_w() {
        let mut c = cpu_with(&[0x300A]); // MOVLW 0x0A
        c.step().unwrap();
        assert_eq!(c.w, 0x0A);
        assert_eq!(c.cycles, 1);
        assert_eq!(c.pc, 1);
    }

    #[test]
    fn addlw_sets_digit_carry() {
        let mut c = cpu_with(&[0x3E01]); // ADDLW 0x01
        c.w = 0x0F;
        c.step().unwrap();
        assert_eq!(c.w, 0x10);
        assert!(c.mem.flag(STATUS_DC));
        assert!(!c.mem.flag(STATUS_C));
        assert!(!c.mem.flag(STATUS_Z));
    }

    #[test]
    fn addlw_sets_carry_and_zero_on_wrap() {
        let mut c = cpu_with(&[0x3EFF]); // ADDLW 0xFF
        c.w = 0x01;
        c.step().unwrap();
        assert_eq!(c.w, 0x00);
        assert!(c.mem.flag(STATUS_C));
        assert!(c.mem.flag(STATUS_Z));
    }

    #[test]
    fn sublw_no_borrow_sets_carry() {
        let mut c = cpu_with(&[0x3C05]); // SUBLW 0x05  -> 0x05 - W
        c.w = 0x03;
        c.step().unwrap();
        assert_eq!(c.w, 0x02);
        assert!(c.mem.flag(STATUS_C)); // 5 >= 3, no borrow
    }

    #[test]
    fn sublw_borrow_clears_carry() {
        let mut c = cpu_with(&[0x3C05]); // SUBLW 0x05
        c.w = 0x06;
        c.step().unwrap();
        assert_eq!(c.w, 0xFF); // 5 - 6 = -1 = 0xFF
        assert!(!c.mem.flag(STATUS_C)); // borrow
    }

    #[test]
    fn subwf_into_w() {
        let mut c = cpu_with(&[0x0220]); // SUBWF 0x20, W  -> f - W
        c.mem.phys_write(0x20, 0x05);
        c.w = 0x03;
        c.step().unwrap();
        assert_eq!(c.w, 0x02);
        assert!(c.mem.flag(STATUS_C));
    }

    #[test]
    fn addwf_into_file() {
        let mut c = cpu_with(&[0x07A0]); // ADDWF 0x20, F
        c.mem.phys_write(0x20, 0x10);
        c.w = 0x22;
        c.step().unwrap();
        assert_eq!(c.mem.phys_read(0x20), 0x32);
    }

    #[test]
    fn andwf_sets_zero() {
        let mut c = cpu_with(&[0x0520]); // ANDWF 0x20, W
        c.mem.phys_write(0x20, 0xF0);
        c.w = 0x0F;
        c.step().unwrap();
        assert_eq!(c.w, 0x00);
        assert!(c.mem.flag(STATUS_Z));
    }

    #[test]
    fn incfsz_skips_on_zero() {
        let mut c = cpu_with(&[0x0FA0, 0x0000, 0x0000]); // INCFSZ 0x20, F
        c.mem.phys_write(0x20, 0xFF); // +1 -> 0x00, skip
        c.step().unwrap();
        assert_eq!(c.mem.phys_read(0x20), 0x00);
        assert_eq!(c.pc, 2); // skipped the next instruction
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn decfsz_skips_on_zero() {
        let mut c = cpu_with(&[0x0BA0, 0x0000, 0x0000]); // DECFSZ 0x20, F
        c.mem.phys_write(0x20, 0x01); // -1 -> 0x00, skip
        c.step().unwrap();
        assert_eq!(c.pc, 2);
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn btfsc_skips_when_clear() {
        let mut c = cpu_with(&[0x1820, 0x0000, 0x0000]); // BTFSC 0x20, 0
        c.mem.phys_write(0x20, 0x00); // bit 0 clear -> skip
        c.step().unwrap();
        assert_eq!(c.pc, 2);
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn btfss_skips_when_set() {
        let mut c = cpu_with(&[0x1C20, 0x0000, 0x0000]); // BTFSS 0x20, 0
        c.mem.phys_write(0x20, 0x01); // bit 0 set -> skip
        c.step().unwrap();
        assert_eq!(c.pc, 2);
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn bsf_and_bcf_set_clear_bits() {
        let mut c = cpu_with(&[0x15A0]); // BSF 0x20, 3
        c.mem.phys_write(0x20, 0x00);
        c.step().unwrap();
        assert_eq!(c.mem.phys_read(0x20), 0x08);

        let mut d = cpu_with(&[0x11A0]); // BCF 0x20, 3
        d.mem.phys_write(0x20, 0xFF);
        d.step().unwrap();
        assert_eq!(d.mem.phys_read(0x20), 0xF7);
    }

    #[test]
    fn rlf_rotates_through_carry() {
        let mut c = cpu_with(&[0x0DA0]); // RLF 0x20, F
        c.mem.phys_write(0x20, 0x80); // C in = 0
        c.step().unwrap();
        assert_eq!(c.mem.phys_read(0x20), 0x00);
        assert!(c.mem.flag(STATUS_C)); // bit 7 fell into C
    }

    #[test]
    fn rrf_rotates_through_carry() {
        let mut c = cpu_with(&[0x0CA0]); // RRF 0x20, F
        c.mem.phys_write(0x20, 0x01); // C in = 0
        c.step().unwrap();
        assert_eq!(c.mem.phys_read(0x20), 0x00);
        assert!(c.mem.flag(STATUS_C)); // bit 0 fell into C
    }

    #[test]
    fn swapf_swaps_nibbles() {
        let mut c = cpu_with(&[0x0E20]); // SWAPF 0x20, W
        c.mem.phys_write(0x20, 0x12);
        c.step().unwrap();
        assert_eq!(c.w, 0x21);
    }

    #[test]
    fn goto_loads_pc() {
        let mut c = cpu_with(&[0x2923]); // GOTO 0x123
        c.step().unwrap();
        assert_eq!(c.pc, 0x123);
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn call_pushes_return_then_return_pops() {
        let mut c = cpu_with(&[0x2002, 0x0000, 0x0008]); // CALL 0x002 ; .. ; RETURN
        c.step().unwrap(); // CALL
        assert_eq!(c.pc, 0x002);
        assert_eq!(c.cycles, 2);
        c.step().unwrap(); // RETURN
        assert_eq!(c.pc, 0x001); // address after CALL
        assert_eq!(c.cycles, 4);
    }

    #[test]
    fn retlw_returns_with_literal_in_w() {
        let mut c = cpu_with(&[0x2002, 0x0000, 0x342A]); // CALL ; .. ; RETLW 0x2A
        c.step().unwrap(); // CALL
        c.step().unwrap(); // RETLW
        assert_eq!(c.w, 0x2A);
        assert_eq!(c.pc, 0x001);
    }

    #[test]
    fn addwf_pcl_is_a_computed_jump() {
        let mut c = cpu_with(&[0x3002, 0x0782, 0x0000, 0x0000, 0x0000]);
        c.step().unwrap(); // MOVLW 0x02 -> W = 2
        c.step().unwrap(); // ADDWF PCL, F  (PCL was 2 after fetch) + 2 = 4
        assert_eq!(c.pc, 4);
        assert_eq!(c.cycles, 3); // PCL write costs 2 cycles
    }

    #[test]
    fn pc_out_of_range_errors() {
        let mut c = Cpu::new();
        c.pc = PROG_WORDS as u16; // 0x800, past 2K flash
        assert_eq!(c.step(), Err(StepError::PcOutOfRange(0x800)));
    }

    // ---------- TMR0 + interrupts ----------

    #[test]
    fn tmr0_counts_one_to_one() {
        let mut c = cpu_with(&[0x0000, 0x0000, 0x0000, 0x0000, 0x0000]); // NOPs
        c.mem.phys_write(OPTION_REG, 0x08); // T0CS=0, PSA=1 -> 1:1
        for _ in 0..5 {
            c.step().unwrap();
        }
        assert_eq!(c.mem.phys_read(TMR0_ADDR), 5);
    }

    #[test]
    fn tmr0_prescaler_divides() {
        let mut c = cpu_with(&[0u16; 8]); // NOPs
        c.mem.phys_write(OPTION_REG, 0x00); // PSA=0, PS=000 -> 1:2
        for _ in 0..4 {
            c.step().unwrap();
        }
        assert_eq!(c.mem.phys_read(TMR0_ADDR), 2);
    }

    #[test]
    fn tmr0_overflow_sets_t0if() {
        let mut c = cpu_with(&[0x0000, 0x0000]);
        c.mem.phys_write(OPTION_REG, 0x08); // 1:1
        c.mem.phys_write(TMR0_ADDR, 0xFF);
        c.step().unwrap(); // one NOP -> TMR0 overflows
        assert_eq!(c.mem.phys_read(TMR0_ADDR), 0x00);
        assert!(c.mem.phys_read(INTCON_ADDR) & 0x04 != 0); // T0IF (bit 2)
    }

    #[test]
    fn writing_tmr0_inhibits_two_cycles() {
        // MOVWF TMR0 (0x0081) reloads TMR0 and inhibits the next two ticks.
        let mut c = cpu_with(&[0x0081, 0x0000, 0x0000, 0x0000]);
        c.mem.phys_write(OPTION_REG, 0x08); // 1:1
        c.w = 0x10;
        c.step().unwrap(); // MOVWF TMR0 -> 0x10, write tick skipped
        c.step().unwrap(); // NOP, inhibited
        c.step().unwrap(); // NOP, inhibited
        assert_eq!(c.mem.phys_read(TMR0_ADDR), 0x10);
        c.step().unwrap(); // NOP, now counts
        assert_eq!(c.mem.phys_read(TMR0_ADDR), 0x11);
    }

    #[test]
    fn timer_interrupt_vectors_and_returns() {
        // 0: BSF INTCON,GIE   1: BSF INTCON,T0IE   2: GOTO 2 (spin)
        // 4: BCF INTCON,T0IF  5: RETFIE
        let mut c = cpu_with(&[0x178B, 0x168B, 0x2802, 0x0000, 0x110B, 0x0009]);
        c.mem.phys_write(OPTION_REG, 0x08); // TMR0 1:1
        c.mem.phys_write(TMR0_ADDR, 0xFE); // about to overflow
        c.step().unwrap(); // BSF GIE   (TMR0 0xFE -> 0xFF)
        c.step().unwrap(); // BSF T0IE  (TMR0 0xFF -> 0x00 overflow -> vector)
        assert_eq!(c.pc, 0x0004); // entered ISR (interrupt vector)
        assert_eq!(c.mem.phys_read(INTCON_ADDR) & 0x80, 0); // GIE cleared on entry
        assert_eq!(c.cycles, 4); // 1 + 1 + 2 (interrupt entry)
        c.step().unwrap(); // BCF T0IF
        c.step().unwrap(); // RETFIE -> back to spin, GIE restored
        assert_eq!(c.pc, 0x0002);
        assert!(c.mem.phys_read(INTCON_ADDR) & 0x80 != 0); // GIE set again
    }

    // ---------- frame sampling (persistence of vision) ----------

    #[test]
    fn pin_on_time_uses_pre_execution_state() {
        // BSF PORTB,0 ; NOP ; NOP ; NOP ; BCF PORTB,0 — RB0 is high entering the
        // four instructions after the BSF. (TRISB defaults to 0 here -> output.)
        let mut c = cpu_with(&[0x1406, 0x0000, 0x0000, 0x0000, 0x1006]);
        c.reset_frame();
        for _ in 0..5 {
            c.step().unwrap();
        }
        assert_eq!(c.sampler().high_cycles(8), 4); // RB0 = pin index 8
        assert_eq!(c.sampler().frame_cycles(), 5);
    }

    // ---------- set_pin (input pins + interrupts) ----------

    #[test]
    fn set_pin_drives_input_read_by_firmware() {
        // RB0 input, driven high; BTFSS PORTB,0 then skips the GOTO.
        let mut c = cpu_with(&[0x1C06, 0x2800, 0x0000]); // BTFSS PORTB,0 ; GOTO 0 ; NOP
        c.mem.phys_write(TRISB_ADDR, 0x01); // RB0 input
        c.set_pin(8, true); // drive RB0 high
        c.step().unwrap();
        assert_eq!(c.pc, 2); // bit set -> skipped
    }

    #[test]
    fn set_pin_rb0_raises_intf_on_selected_edge() {
        let mut c = Cpu::new();
        c.mem.phys_write(TRISB_ADDR, 0x01); // RB0 input
        c.mem.phys_write(OPTION_REG, 0x40); // INTEDG = 1 (rising)
        c.set_pin(8, true); // RB0 0 -> 1 (rising)
        assert!(c.mem.phys_read(INTCON_ADDR) & 0x02 != 0); // INTF (bit 1)
    }

    #[test]
    fn set_pin_rb4_raises_rbif_on_change() {
        let mut c = Cpu::new();
        c.mem.phys_write(TRISB_ADDR, 0xF0); // RB4..7 inputs
        c.set_pin(12, true); // RB4 change
        assert!(c.mem.phys_read(INTCON_ADDR) & 0x01 != 0); // RBIF (bit 0)
    }

    // ---------- EEPROM ----------

    #[test]
    fn eeprom_write_unlock_sequence_persists_across_reset() {
        // bank1; EEADR=5; EEDATA=0x2A; WREN; EECON2<-0x55,0xAA; WR.
        let mut c = cpu_with(&[
            0x1683, // BSF STATUS,RP0  (bank 1)
            0x3005, 0x009B, // MOVLW 5    ; MOVWF EEADR
            0x302A, 0x009A, // MOVLW 0x2A ; MOVWF EEDATA
            0x151C, // BSF EECON1,WREN
            0x3055, 0x009D, // MOVLW 0x55 ; MOVWF EECON2
            0x30AA, 0x009D, // MOVLW 0xAA ; MOVWF EECON2
            0x149C, // BSF EECON1,WR
        ]);
        for _ in 0..11 {
            c.step().unwrap();
        }
        assert_eq!(c.eeprom_byte(5), 0x2A);
        c.reset();
        assert_eq!(c.eeprom_byte(5), 0x2A); // non-volatile: survives reset
    }

    #[test]
    fn eeprom_write_ignored_without_unlock() {
        // WREN + WR but no 0x55/0xAA unlock -> the write must not happen.
        let mut c = cpu_with(&[
            0x1683, 0x3005, 0x009B, 0x302A, 0x009A, 0x151C, // ... WREN
            0x149C, // BSF EECON1,WR  (not unlocked)
        ]);
        for _ in 0..7 {
            c.step().unwrap();
        }
        assert_eq!(c.eeprom_byte(5), 0xFF); // unchanged (erased)
    }

    #[test]
    fn eeprom_read_loads_eedata() {
        let mut c = cpu_with(&[
            0x1683, // BSF STATUS,RP0
            0x3005, 0x009B, // MOVLW 5 ; MOVWF EEADR
            0x141C, // BSF EECON1,RD
            0x081A, // MOVF EEDATA,W
        ]);
        c.eeprom[5] = 0x39;
        for _ in 0..5 {
            c.step().unwrap();
        }
        assert_eq!(c.w, 0x39);
    }
}
