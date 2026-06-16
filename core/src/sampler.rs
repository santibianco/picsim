//! Per-pin on-time accumulation for persistence-of-vision rendering.
//!
//! A multiplexed display switches digits many times within one rendered frame.
//! Snapshotting pin state once per frame would catch a single digit lit and the
//! rest dark — flicker a real eye never sees (architecture.md §4). Instead the
//! core integrates, per pin, how many cycles it was driven high across the
//! frame; the renderer turns that into brightness (and a wrong refresh duty
//! cycle then *correctly* looks dim).
//!
//! The core records raw per-pin high-time and the total frame cycles. The
//! renderer applies the diagram's active-high/active-low convention.

/// RA0..RA7 occupy indices 0..7, RB0..RB7 occupy 8..15.
pub const PIN_COUNT: usize = 16;

#[derive(Debug, Clone)]
pub struct PinSampler {
    high_cycles: [u64; PIN_COUNT],
    frame_cycles: u64,
}

impl PinSampler {
    pub fn new() -> Self {
        PinSampler {
            high_cycles: [0; PIN_COUNT],
            frame_cycles: 0,
        }
    }

    /// Clear the accumulators at the start of a rendered frame.
    pub fn reset_frame(&mut self) {
        self.high_cycles = [0; PIN_COUNT];
        self.frame_cycles = 0;
    }

    /// Integrate `cycles` worth of time during which PORTA/PORTB held these
    /// values (each high bit accrues `cycles` of on-time).
    pub fn accumulate(&mut self, porta: u8, portb: u8, cycles: u64) {
        for bit in 0..8 {
            if (porta >> bit) & 1 == 1 {
                self.high_cycles[bit] += cycles;
            }
            if (portb >> bit) & 1 == 1 {
                self.high_cycles[8 + bit] += cycles;
            }
        }
        self.frame_cycles += cycles;
    }

    pub fn frame_cycles(&self) -> u64 {
        self.frame_cycles
    }

    pub fn high_cycles(&self, pin: usize) -> u64 {
        self.high_cycles[pin]
    }

    /// Fraction of the frame this pin was driven high (0.0..=1.0). The renderer
    /// inverts this for active-low components.
    pub fn duty(&self, pin: usize) -> f32 {
        if self.frame_cycles == 0 {
            0.0
        } else {
            self.high_cycles[pin] as f32 / self.frame_cycles as f32
        }
    }
}

impl Default for PinSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a pin name like `"RB0"` or `"RA4"` to its sampler index (RA0..7 -> 0..7,
/// RB0..7 -> 8..15). Components in a diagram bind directly to these names.
pub fn pin_index(name: &str) -> Option<usize> {
    let b = name.as_bytes();
    if b.len() != 3 || b[0] != b'R' {
        return None;
    }
    let bit = (b[2] as char).to_digit(10)? as usize;
    if bit > 7 {
        return None;
    }
    match b[1] {
        b'A' | b'a' => Some(bit),
        b'B' | b'b' => Some(8 + bit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_index_maps_porta_and_portb() {
        assert_eq!(pin_index("RA0"), Some(0));
        assert_eq!(pin_index("RA7"), Some(7));
        assert_eq!(pin_index("RB0"), Some(8));
        assert_eq!(pin_index("RB7"), Some(15));
        assert_eq!(pin_index("RC0"), None);
        assert_eq!(pin_index("RB8"), None);
        assert_eq!(pin_index("nope"), None);
    }

    #[test]
    fn accumulate_and_duty() {
        let mut s = PinSampler::new();
        s.accumulate(0b0000_0001, 0b0000_0010, 10); // RA0 high, RB1 high
        s.accumulate(0, 0, 10); // all low
        assert_eq!(s.high_cycles(0), 10); // RA0
        assert_eq!(s.high_cycles(9), 10); // RB1 -> index 8+1
        assert_eq!(s.frame_cycles(), 20);
        assert!((s.duty(0) - 0.5).abs() < 1e-6);
        assert_eq!(s.duty(2), 0.0);
    }

    #[test]
    fn reset_clears_accumulators() {
        let mut s = PinSampler::new();
        s.accumulate(0xFF, 0xFF, 5);
        s.reset_frame();
        assert_eq!(s.frame_cycles(), 0);
        assert_eq!(s.high_cycles(0), 0);
    }
}
