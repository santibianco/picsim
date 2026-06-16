//! Interrupt logic: the INTCON sources, when one is pending, and the vector.
//!
//! The CPU checks this after every instruction (architecture.md §2 step 6).
//! Sources used by LED/button/display labs: TMR0 overflow (T0IF), the RB0/INT
//! external interrupt (INTF), and PORTB interrupt-on-change (RBIF) — each gated
//! by its enable bit and by the global GIE.

/// Interrupt vector address (mid-range fixed vector).
pub const INT_VECTOR: u16 = 0x0004;

/// Cycles inserted to acknowledge an interrupt (push PC + vector to 0x0004).
///
/// The datasheet quotes ~3 instruction-cycle latency; the exact accounting that
/// matches MPLAB's stopwatch is a reconciliation item, so it lives here as a
/// single tunable constant rather than being scattered through the CPU.
pub const INTERRUPT_ENTRY_CYCLES: u64 = 2;

// INTCON bit positions.
pub const GIE: u8 = 7;
pub const PEIE: u8 = 6;
pub const T0IE: u8 = 5;
pub const INTE: u8 = 4;
pub const RBIE: u8 = 3;
pub const T0IF: u8 = 2;
pub const INTF: u8 = 1;
pub const RBIF: u8 = 0;

/// True if GIE is set and at least one enabled source is flagged.
///
/// Note: T0IF/INTF/RBIF are core INTCON interrupts gated only by GIE (PEIE
/// gates peripheral interrupts via PIE1/PIR1, which we don't model yet).
pub fn pending(intcon: u8) -> bool {
    if (intcon >> GIE) & 1 == 0 {
        return false;
    }
    let t0 = (intcon >> T0IE) & (intcon >> T0IF) & 1;
    let int = (intcon >> INTE) & (intcon >> INTF) & 1;
    let rb = (intcon >> RBIE) & (intcon >> RBIF) & 1;
    (t0 | int | rb) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gie_off_is_never_pending() {
        // T0IE + T0IF set but GIE clear.
        assert!(!pending((1u8 << T0IE) | (1u8 << T0IF)));
    }

    #[test]
    fn enabled_and_flagged_timer_is_pending() {
        let intcon = (1u8 << GIE) | (1u8 << T0IE) | (1u8 << T0IF);
        assert!(pending(intcon));
    }

    #[test]
    fn flagged_but_not_enabled_is_not_pending() {
        // T0IF set, GIE set, but T0IE clear.
        assert!(!pending((1u8 << GIE) | (1u8 << T0IF)));
    }
}
