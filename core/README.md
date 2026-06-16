# new-proteus-core

The cycle-accurate **PIC16F628A** simulation core. Pure Rust, **dependency-free**,
so it builds and tests with no network and no extra tooling.

## Build & test

```sh
cd core
cargo test
```

All suites should pass: `decode`, `hex`, `memory`, `cpu`, `scheduler`, `timer`,
`interrupts`, `lib`, plus the integration and example cross-checks.

## Watch it run (terminal e2e)

```sh
cargo run --example run                          # built-in blink demo
cargo run --example run -- "../examples/Simple one/Punto A.hex"   # PORTB -> 0xFF
cargo run --example run -- path/to/your.hex
```

Loads a `.hex` and renders PORTA/PORTB as ASCII LEDs each frame using the same
`Core` API the browser runtime will use (`load_hex` / `run_cycles` / `read_pins`).

## Status: build-order step 2 complete (CPU core + TMR0/interrupts)

| File | Responsibility |
|---|---|
| `decode.rs` | All **35** PIC14 instructions: table-driven decode + disassembler. |
| `hex.rs` | Intel HEX loader (checksum-verified) into 2K program memory; config + EEPROM. |
| `memory.rs` | Banked data memory: RP0/RP1, shared SFRs, common RAM, FSR/INDF, STATUS flags. |
| `cpu.rs` | Full execute: 35 ops, Z/C/DC, 8-level stack, computed PCL jumps, exact cycles. |
| `timer.rs` | TMR0 + prescaler (OPTION_REG), T0IF overflow, TMR0-write inhibit. |
| `interrupts.rs` | GIE-gated vectoring to 0x0004 (T0IF/INTF/RBIF); RETFIE restores GIE. |
| `scheduler.rs` | `run_cycles(n)` with absolute-target accounting (no timing drift). |
| `lib.rs` | `Core` API: `load_hex` / `run_cycles` / `read_pins` / `reset` / `set_clock_hz`. |

Validated end to end against real MPLAB labs in `../examples/` and synthetic
programs — e.g. Multiplication computes 7×10=70 in exactly **31 cycles**, and a
TMR0 overflow with T0IE+GIE vectors to 0x0004 and returns via RETFIE.

## What's next

- **Step 3** — scheduler frame-sampling + 7-seg on-time/PoV accumulation.
- **Step 4** — components (LEDs, buttons, 7-seg) + JSON diagram loader. Button
  input (`set_pin`) also lights up **INTF/RBIF** (the RB0/INT + PORTB-change
  interrupts), whose vectoring logic is already in place but dormant without pins.
- **Step 5** — student runtime (load hex + diagram, run, click buttons); WASM +
  PWA packaging wrapping this `Core`.

## Known reconciliation items (vs MPLAB)

- Interrupt entry-cycle count (`INTERRUPT_ENTRY_CYCLES`, currently 2) and the
  exact TMR0 period of reload ISRs — confirm against MPLAB's stopwatch using a
  timer lab (e.g. TP 2022's ~15 ms multiplex tick).
- External TMR0 clock (T0CS=1, counting T0CKI edges) is not modeled yet.

## Trust anchor

`cargo test` is the gate, run on every change — the thing SimulIDE lacked.
