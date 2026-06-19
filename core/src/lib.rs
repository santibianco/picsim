//! New Proteus — cycle-accurate PIC16F628A simulation core.
//!
//! This crate is the single source of truth for simulated time. The public
//! surface mirrors the WASM interface in `docs/architecture.md` §5
//! (loadHex / runCycles / readPins / setPin / reset / setClockHz) plus the
//! per-frame on-time sampling for 7-seg persistence of vision (§4). It is pure,
//! dependency-free Rust so it builds and tests with `cargo test` and no network;
//! the `wasm-bindgen` wrapper + `cdylib` are added with the browser build-tooling
//! milestone, wrapping this already-tested API.
//!
//! Status: the §5 core interface is complete (CPU + TMR0/interrupts + frame
//! sampling + input pins). Next is the browser runtime.

pub mod cpu;
pub mod decode;
pub mod hex;
pub mod interrupts;
pub mod memory;
pub mod sampler;
pub mod scheduler;
pub mod timer;

// Browser bindings: a C-ABI over `Core`, compiled only for the wasm target.
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use cpu::{Cpu, StepError};
use scheduler::Scheduler;

pub use sampler::{pin_index, PIN_COUNT};

/// Default oscillator: 4 MHz internal. Stored as a variable so a settable clock
/// comes for free — the core counts cycles and is otherwise clock-agnostic; the
/// clock only matters when converting cycles to wall-clock time for rendering
/// and timers (architecture.md §1).
pub const DEFAULT_CLOCK_HZ: u32 = 4_000_000;

/// The simulator core. Holds the CPU, the cycle scheduler, and the clock.
pub struct Core {
    cpu: Cpu,
    sched: Scheduler,
    clock_hz: u32,
}

impl Core {
    pub fn new() -> Self {
        Core {
            cpu: Cpu::new(),
            sched: Scheduler::new(),
            clock_hz: DEFAULT_CLOCK_HZ,
        }
    }

    /// `loadHex`: parse Intel HEX into program memory and power-on reset.
    pub fn load_hex(&mut self, text: &str) -> Result<(), hex::HexError> {
        let img = hex::parse(text)?;
        self.cpu.load_image(img);
        self.sched.reset();
        Ok(())
    }

    /// `runCycles(n)`: advance the simulation by `n` instruction cycles of
    /// firmware. Returns the cycles actually run, or `StepError::PcOutOfRange`
    /// if the program counter leaves the 2K flash.
    pub fn run_cycles(&mut self, n: u64) -> Result<u64, StepError> {
        self.sched.run_cycles(&mut self.cpu, n)
    }

    /// `reset`: power-on reset.
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.sched.reset();
    }

    /// `setClockHz`: change the clock. Only affects cycle -> wall-clock
    /// conversion (render cadence and timer ticks), never the core's counting.
    pub fn set_clock_hz(&mut self, hz: u32) {
        self.clock_hz = hz;
    }

    pub fn clock_hz(&self) -> u32 {
        self.clock_hz
    }

    /// Cycles per 60 fps frame: `clock_hz / 4 / 60` (≈16,666 at 4 MHz).
    pub fn cycles_per_frame_60(&self) -> u64 {
        (self.clock_hz as u64) / 4 / 60
    }

    /// `readPins`: effective PORTA/PORTB pin levels (output bits from the latch,
    /// input bits from the externally-driven level). For brightness use the
    /// on-time API below rather than this single snapshot.
    pub fn read_pins(&self) -> (u8, u8) {
        (self.cpu.mem.port_a(), self.cpu.mem.port_b())
    }

    /// `setPin`: drive an external input pin (a button) by sampler index
    /// (0..7 = RA0..RA7, 8..15 = RB0..RB7). Raises RB0/INT or PORTB-change
    /// interrupt flags where applicable.
    pub fn set_pin(&mut self, pin: usize, level: bool) {
        self.cpu.set_pin(pin, level);
    }

    /// Drive an input pin by name, e.g. `set_pin_name("RB0", false)`. Returns
    /// false if the name isn't a valid pin.
    pub fn set_pin_name(&mut self, name: &str, level: bool) -> bool {
        match pin_index(name) {
            Some(p) => {
                self.cpu.set_pin(p, level);
                true
            }
            None => false,
        }
    }

    // ---- per-frame on-time sampling (persistence of vision, §4) ----

    /// Reset the per-frame on-time accumulators. Call at the start of each
    /// rendered frame, before `run_cycles`.
    pub fn reset_frame_accumulators(&mut self) {
        self.cpu.reset_frame();
    }

    /// Fraction (0.0..=1.0) of the last frame that `pin` was driven high.
    /// Indices: 0..7 = RA0..RA7, 8..15 = RB0..RB7 (see [`pin_index`]). The
    /// renderer inverts this for active-low components.
    pub fn pin_duty(&self, pin: usize) -> f32 {
        self.cpu.sampler().duty(pin)
    }

    /// Raw cycles `pin` was driven high during the frame.
    pub fn pin_high_cycles(&self, pin: usize) -> u64 {
        self.cpu.sampler().high_cycles(pin)
    }

    /// Total cycles integrated into the current frame.
    pub fn frame_cycles(&self) -> u64 {
        self.cpu.sampler().frame_cycles()
    }

    /// Borrow the CPU (registers, memory, program) for inspection/tests.
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    // ---- debugger / inspection surface (read-only + single-step) ----
    // Read accessors never advance simulated time; `step_one` advances exactly
    // one instruction (cycle-exact). The C-ABI in `wasm.rs` is a thin wrapper and
    // `verify-core.js` pins the contract.

    /// Program counter (13-bit).
    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    /// Working register W.
    pub fn w(&self) -> u8 {
        self.cpu.w
    }

    /// Instruction cycles elapsed since the last reset.
    pub fn cycle_count(&self) -> u64 {
        self.cpu.cycles
    }

    /// Read a data-memory cell by absolute physical address (`0x000..=0x1FF`,
    /// i.e. `bank*0x80 + offset`). Returns the raw register byte — PORT cells
    /// read as the output latch — and 0 for out-of-range addresses.
    pub fn read_data(&self, addr: usize) -> u8 {
        if addr < 0x200 {
            self.cpu.mem.phys_read(addr)
        } else {
            0
        }
    }

    /// Read a program-flash word by address (`0x000..=0x7FF`). Out-of-range
    /// addresses return the blank (erased) word.
    pub fn prog_word(&self, addr: usize) -> u16 {
        self.cpu.program.get(addr).copied().unwrap_or(hex::BLANK_WORD)
    }

    /// Single-step: execute exactly one instruction; returns its cycle cost
    /// (0 if the PC has left valid program memory). Cycle-exact.
    pub fn step_one(&mut self) -> u8 {
        let before = self.cpu.cycles;
        let _ = self.cpu.step();
        self.cpu.cycles.wrapping_sub(before) as u8
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_hex_then_inspect_first_instruction() {
        let mut core = Core::new();
        let blink = ":0A000000831686018312860A032886\n:00000001FF\n";
        core.load_hex(blink).expect("valid hex");
        // First instruction of the fixture is BSF STATUS, RP0.
        assert_eq!(core.cpu().decode_at_pc().mnemonic, decode::Mnemonic::Bsf);
    }

    #[test]
    fn clock_and_frame_math() {
        let core = Core::new();
        assert_eq!(core.clock_hz(), 4_000_000);
        assert_eq!(core.cycles_per_frame_60(), 16_666);
    }

    #[test]
    fn frame_sampling_tracks_pin_duty() {
        // BSF PORTB,0 ; NOP ; NOP ; NOP ; BCF PORTB,0 -> RB0 high for 4 of 5 cy.
        let mut core = Core::new();
        core.cpu_mut().program = vec![0x1406, 0x0000, 0x0000, 0x0000, 0x1006];
        core.reset_frame_accumulators();
        core.run_cycles(5).unwrap();
        assert_eq!(core.pin_high_cycles(8), 4); // RB0 = index 8
        assert_eq!(core.frame_cycles(), 5);
        assert!((core.pin_duty(8) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn set_pin_lets_firmware_poll_a_button() {
        let mut core = Core::new();
        // RB0 as input; BTFSS PORTB,0 ; GOTO 0 ; NOP
        core.cpu_mut().mem.phys_write(0x86, 0x01); // TRISB bit0 = input
        core.cpu_mut().program = vec![0x1C06, 0x2800, 0x0000];
        assert!(core.set_pin_name("RB0", true)); // press -> high
        core.run_cycles(1).unwrap(); // BTFSS skips the GOTO
        assert_eq!(core.cpu().pc, 2);
    }

    #[test]
    fn debugger_surface_steps_and_inspects() {
        // Mirrors verify-core.js part (B): step BSF STATUS,RP0 and inspect.
        let mut core = Core::new();
        core.load_hex(":0A000000831686018312860A032886\n:00000001FF\n")
            .unwrap();
        assert_eq!(core.pc(), 0);
        assert_eq!(core.cycle_count(), 0);
        assert_eq!(core.prog_word(0), 0x1683);
        assert_eq!(crate::decode::disassemble(core.prog_word(0)), "BSF 0x03, 5");
        assert_eq!(core.step_one(), 1); // BSF is 1 cycle
        assert_eq!(core.pc(), 1);
        assert_eq!(core.cycle_count(), 1);
        assert_eq!(core.read_data(0x03), 0x38); // STATUS = POR 0x18 | RP0 (0x20)
        assert_eq!(core.w(), 0);
        assert_eq!(core.read_data(0x500), 0); // out-of-range read -> 0
    }
}
