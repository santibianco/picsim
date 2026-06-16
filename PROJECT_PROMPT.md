# Project Prompt — paste into Cowork on first run

Use this as the kickoff message after creating the Cowork project from this folder.

---

You are helping me build **New Proteus**, a browser-based, OS-agnostic simulator for the
PIC16F628A microcontroller, for classroom use. Before doing anything else:

1. Read every file in this folder — especially `README.md` and `docs/architecture.md`.
   `docs/architecture.md` is the load-bearing spec (timing model + core interface).
2. Summarize back to me: what this project is, the hard scope constraints, the
   non-negotiable requirement, and the build order you'll follow. If anything is
   unclear or underspecified, ask before starting.

Key things to internalize from the docs (do not violate these without flagging it):

- **Scope is deliberately narrow**: PIC16F628A only, digital-only I/O (no analog/SPICE),
  components limited to LEDs / 7-segment displays / buttons, students don't edit diagrams.
- **Cycle-accuracy is the core requirement**, because multiplexing and debouncing are
  timing-dependent. A drifting clock makes students debug correct code — that's the
  failure mode (SimulIDE) we're escaping.
- **Cycles are the master clock**; the render loop samples the core and never drives it.
- **Architecture**: Rust → WASM core (single source of truth for time) with a tiny
  interface (`loadHex` / `runCycles(n)` / `readPins` / `setPin` / `reset` /
  `setClockHz`); JS + Canvas runtime calling `runCycles(clock_hz/4/60)` each frame;
  7-seg rendered via per-segment on-time accumulation for persistence of vision.
- **Input** is Intel HEX from MPLAB — no assembler is built here.
- **Trust anchor**: maintain a test suite of small programs with known register
  outcomes, verified cycle-by-cycle against MPLAB's simulator. Get `BSF`/`BCF`, the
  STATUS flags, and banking correct first.

When we start coding, follow the de-risked build order in `docs/architecture.md` §7:
CPU core + memory + HEX loader (verified) → TMR0 + prescaler + interrupts → scheduler +
frame sampling → components → student runtime → (deferred) authoring tool.

Suggested first concrete task: scaffold the Rust `core/` crate targeting WASM, implement
the Intel HEX loader and the CPU fetch/decode skeleton with the instruction table, and
set up the test harness so we can start asserting against known firmware immediately.
