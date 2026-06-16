//! Terminal runner — load an Intel HEX and watch the core drive its pins.
//!
//! This is the earliest end-to-end "see it work" surface: it uses the same
//! `Core` API the browser runtime will use (load_hex / run_cycles / read_pins),
//! just rendered as ASCII LEDs in your console instead of a Canvas.
//!
//!   cargo run --example run                         # built-in blink demo
//!   cargo run --example run -- path/to/program.hex  # your own .hex
//!   cargo run --example run -- program.hex 50 40    # 50 cycles/frame, 40 frames
//!
//! Note: programs that multiplex displays or time delays via TMR0/interrupts
//! need build-order step 2 before they animate correctly. Output-only programs
//! (e.g. the Multiplication and "Punto A" examples) run fully today.

use new_proteus_core::Core;

const DEFAULT_HEX: &str = include_str!("../tests/fixtures/blink.hex");

/// MSB..LSB LED bar: '#' = high, '.' = low.
fn leds(byte: u8) -> String {
    (0..8).rev().map(|i| if (byte >> i) & 1 == 1 { '#' } else { '.' }).collect()
}

/// PORTB bits 0..6 drawn as a 7-seg digit (a=RB0 .. g=RB6), illustrative.
fn seven_seg(b: u8) -> [String; 3] {
    let on = |bit: u8| (b >> bit) & 1 == 1;
    let bar = |v: bool, c: char| if v { c } else { ' ' };
    [
        format!(" {} ", bar(on(0), '_')),
        format!("{}{}{}", bar(on(5), '|'), bar(on(6), '_'), bar(on(1), '|')),
        format!("{}{}{}", bar(on(4), '|'), bar(on(3), '_'), bar(on(2), '|')),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (hex, name) = match args.get(1) {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("could not read {path}: {e}");
                std::process::exit(1);
            });
            (text, path.clone())
        }
        None => (DEFAULT_HEX.to_string(), "built-in blink.hex".to_string()),
    };
    let per_frame: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    let frames: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(40);

    let mut core = Core::new();
    if let Err(e) = core.load_hex(&hex) {
        eprintln!("bad hex: {e:?}");
        std::process::exit(1);
    }

    println!("New Proteus — running {name}\n");
    println!("{:>9}   W   flags   PORTA      PORTB", "cycle");
    for _ in 0..frames {
        if let Err(e) = core.run_cycles(per_frame) {
            eprintln!("stopped: {e:?}");
            break;
        }
        let (pa, pb) = core.read_pins();
        let s = core.cpu().mem.status();
        let flags = format!(
            "{}{}{}",
            if (s >> 2) & 1 == 1 { 'Z' } else { '-' },
            if (s >> 1) & 1 == 1 { 'D' } else { '-' },
            if s & 1 == 1 { 'C' } else { '-' },
        );
        println!(
            "{:>9}  {:02X}   {}     {}   {}  0x{:02X}",
            core.cpu().cycles,
            core.cpu().w,
            flags,
            leds(pa),
            leds(pb),
            pb
        );
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    let (_, pb) = core.read_pins();
    println!("\nPORTB as a 7-seg digit (a=RB0 .. g=RB6):");
    for line in seven_seg(pb) {
        println!("   {line}");
    }
}
