//! Cycle scheduler: turns a requested cycle budget into instruction steps.
//!
//! Cycles are the master clock; this is the spine the render loop samples but
//! never drives. The target is kept as an **absolute** cycle count so that an
//! instruction straddling a frame boundary (a 2-cycle op when 1 cycle was left)
//! is naturally credited to the next call — simulated time never drifts, which
//! is exactly what multiplexing and debouncing depend on (architecture.md §2).

use crate::cpu::{Cpu, StepError};

#[derive(Debug, Clone)]
pub struct Scheduler {
    /// Absolute cycle target. `run_cycles` advances this and steps the CPU until
    /// its elapsed cycle count catches up; overshoot carries forward for free.
    target: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { target: 0 }
    }

    /// Reset the absolute target to zero, aligned with the CPU's post-reset
    /// cycle count (both are 0 after a power-on reset).
    pub fn reset(&mut self) {
        self.target = 0;
    }

    /// Advance the target by `n` cycles and step the CPU until it reaches it.
    /// Returns the number of cycles actually run this call, or the error that
    /// stopped progress.
    ///
    /// The target is **absolute and monotonic**: if the previous call overshot
    /// by a cycle (a 2-cycle op when 1 was left in the budget), the CPU's cycle
    /// count is already past the old target, so this call naturally runs one
    /// fewer — the overshoot is carried, not lost. That is the no-drift property.
    pub fn run_cycles(&mut self, cpu: &mut Cpu, n: u64) -> Result<u64, StepError> {
        cpu.break_hit = false;
        self.target = self.target.saturating_add(n);
        let start = cpu.cycles;
        while cpu.cycles < self.target {
            // Breakpoint: stop *before* executing the instruction at a marked PC.
            // The runtime steps once past it on resume, so this never deadlocks.
            if cpu.is_break(cpu.pc) {
                cpu.break_hit = true;
                self.target = cpu.cycles; // drop the unreached budget so it can't pile up
                break;
            }
            cpu.step()?;
        }
        Ok(cpu.cycles - start)
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Cpu;

    #[test]
    fn runs_requested_cycles_with_nops() {
        let mut c = Cpu::new();
        c.program = vec![0x0000; 100]; // NOPs
        let mut s = Scheduler::new();
        let ran = s.run_cycles(&mut c, 10).unwrap();
        assert_eq!(ran, 10);
        assert_eq!(c.cycles, 10);
        assert_eq!(c.pc, 10);
    }

    #[test]
    fn absolute_target_does_not_drift_across_calls() {
        let mut c = Cpu::new();
        c.program = vec![0x0000; 100];
        let mut s = Scheduler::new();
        s.run_cycles(&mut c, 7).unwrap();
        s.run_cycles(&mut c, 5).unwrap();
        assert_eq!(c.cycles, 12);
    }
}
