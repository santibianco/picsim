//! TMR0 + prescaler.
//!
//! Almost all delay / multiplex / debounce firmware leans on TMR0. In internal
//! mode (OPTION_REG T0CS = 0) it counts instruction cycles (Fosc/4) through the
//! prescaler and sets T0IF on each 0xFF -> 0x00 overflow. The prescaler rate and
//! assignment come from OPTION_REG; a write to TMR0 clears the prescaler and
//! inhibits counting for two cycles (a real 16F628A timing detail that affects
//! the exact period of timer-reload ISRs).

use crate::interrupts::T0IF;
use crate::memory::{DataMem, INTCON, TMR0};

/// OPTION_REG physical address (bank 1, offset 0x01 -> phys 0x81).
const OPTION_REG: usize = 0x81;

#[derive(Debug, Default)]
pub struct Timer0 {
    /// Fosc/4 ticks accumulated toward the current prescaler divisor.
    prescaler: u32,
    /// Instruction cycles remaining where the counter is inhibited (post-write).
    inhibit: u8,
}

impl Timer0 {
    pub fn new() -> Self {
        Timer0::default()
    }

    pub fn reset(&mut self) {
        self.prescaler = 0;
        self.inhibit = 0;
    }

    /// Firmware wrote TMR0: clear the prescaler and inhibit counting for the
    /// next two instruction cycles. (The CPU also skips this instruction's tick,
    /// so the write itself doesn't increment.)
    pub fn on_tmr0_write(&mut self) {
        self.prescaler = 0;
        self.inhibit = 2;
    }

    /// Advance TMR0 by `cycles` instruction cycles, setting T0IF on overflow.
    pub fn tick(&mut self, mem: &mut DataMem, cycles: u64) {
        let option = mem.phys_read(OPTION_REG);
        if (option >> 5) & 1 == 1 {
            return; // T0CS = 1: external T0CKI source not modeled yet
        }
        // PSA = 0 -> prescaler assigned to TMR0 (divisor 2..256); PSA = 1 -> the
        // prescaler goes to the WDT and TMR0 runs 1:1.
        let div: u32 = if (option >> 3) & 1 == 0 {
            1u32 << (((option & 0x07) as u32) + 1)
        } else {
            1
        };

        let mut ticks = cycles;
        if self.inhibit > 0 {
            let used = (self.inhibit as u64).min(ticks);
            self.inhibit -= used as u8;
            ticks -= used;
        }
        if ticks == 0 {
            return;
        }

        let acc = self.prescaler + ticks as u32;
        let increments = acc / div;
        self.prescaler = acc % div;
        if increments == 0 {
            return;
        }

        let total = mem.phys_read(TMR0 as usize) as u32 + increments;
        mem.phys_write(TMR0 as usize, (total & 0xFF) as u8);
        if total > 0xFF {
            let intcon = mem.phys_read(INTCON as usize) | (1u8 << T0IF);
            mem.phys_write(INTCON as usize, intcon);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_with_option(option: u8) -> DataMem {
        let mut m = DataMem::new();
        m.phys_write(OPTION_REG, option);
        m
    }

    #[test]
    fn counts_one_to_one_when_prescaler_on_wdt() {
        let mut t = Timer0::new();
        let mut m = mem_with_option(0x08); // T0CS=0, PSA=1 -> 1:1
        t.tick(&mut m, 5);
        assert_eq!(m.phys_read(TMR0 as usize), 5);
    }

    #[test]
    fn prescaler_divides() {
        let mut t = Timer0::new();
        let mut m = mem_with_option(0x00); // PSA=0, PS=000 -> 1:2
        t.tick(&mut m, 4);
        assert_eq!(m.phys_read(TMR0 as usize), 2);
    }

    #[test]
    fn overflow_sets_t0if() {
        let mut t = Timer0::new();
        let mut m = mem_with_option(0x08); // 1:1
        m.phys_write(TMR0 as usize, 0xFF);
        t.tick(&mut m, 1);
        assert_eq!(m.phys_read(TMR0 as usize), 0x00);
        assert!(m.phys_read(INTCON as usize) & (1u8 << T0IF) != 0);
    }

    #[test]
    fn write_inhibits_two_cycles() {
        let mut t = Timer0::new();
        let mut m = mem_with_option(0x08); // 1:1
        m.phys_write(TMR0 as usize, 0x10);
        t.on_tmr0_write(); // CPU calls this and skips the write instruction's tick
        t.tick(&mut m, 1); // inhibited
        t.tick(&mut m, 1); // inhibited
        assert_eq!(m.phys_read(TMR0 as usize), 0x10);
        t.tick(&mut m, 1); // counts
        assert_eq!(m.phys_read(TMR0 as usize), 0x11);
    }
}
