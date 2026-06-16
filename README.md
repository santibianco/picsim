# New Proteus — Browser-Based PIC16F628A Simulator

A web-based, OS-agnostic simulator for running and visually testing PIC microcontroller
firmware against simple digital components (LEDs, 7-segment displays, buttons).

Built for **classroom use**: students write firmware in their existing IDE (MPLAB),
compile to a `.hex` file, and load it into a browser to watch it drive a virtual board.

---

## Why this exists

The two existing options both fail for teaching digital PIC work:

- **SimulIDE** — free and runs natively, but its PIC core is incomplete. Bit-level
  instructions like `BSF`/`BCF` don't behave correctly, so students end up debugging
  *correct* code. Once a simulator can't be trusted, it's useless as a teaching tool.
- **Proteus** — accurate, but paid, Windows-only, and far heavier than needed. Its
  crown jewel is SPICE-level analog co-simulation, which is overkill for a classroom
  that only needs digital LEDs/displays/buttons. On Apple Silicon it also requires a
  Windows VM (emulation on emulation).

This project deliberately builds the *small, correct* tool that's missing: an accurate
single-chip digital simulator that runs in any browser on any OS.

---

## Scope (deliberately narrow)

These constraints are what make the project achievable. **Do not expand scope without
revisiting the timing/architecture implications.**

- **One chip only: PIC16F628A.** No configurable device abstraction. The 2K flash,
  224 bytes RAM, SFR layout, banking, and peripheral set are all hardcoded to this part.
- **Digital-only I/O.** No analog. No SPICE. No equation solver. A pin is `0` or `1`.
  This removes ~90% of the genuine difficulty.
- **Components: LEDs, 7-segment displays, buttons only.** No character LCD, no other parts.
- **Students do not edit or build diagrams.** They load their `.hex` against a diagram
  the instructor prepared, run it, and interact (click buttons). Diagram authoring is a
  separate concern (see below).

### Explicitly out of scope
- Analog/SPICE simulation
- Freeform wiring with drawn wires between pins
- Any PIC other than the 16F628A
- A built-in assembler (firmware comes in as Intel HEX from MPLAB)
- Character LCD (HD44780) and other components

---

## The non-negotiable requirement: cycle-accuracy

This is the single most important property of the whole project, and the thing SimulIDE
got wrong.

The two techniques students will use — **display multiplexing** and **button
debouncing** — are both fundamentally about *time*:

- **Multiplexing** cycles through digits faster than the eye can detect (~kHz), relying
  on persistence of vision to make all digits appear lit.
- **Debouncing** measures elapsed time (a ~5–20ms window) to reject mechanical contact noise.

If the simulator's count of elapsed instruction cycles drifts from real PIC behavior,
**both features break in ways that make students debug correct code** — multiplexed
displays flicker or ghost, debounce routines misbehave. That is exactly the SimulIDE
failure mode. So cycle-accuracy is not polish; it is the core requirement.

The good news: cycle-accuracy on a single known chip is very achievable. The 16F628A has
a dead-simple timing model — every instruction is **1 instruction cycle (4 clocks)
except branches and skips-taken, which are 2**. At 4 MHz, **1 instruction cycle = 1 µs**.
No pipeline, no cache, no variable latency. You count cycles exactly by construction.

See [`docs/architecture.md`](docs/architecture.md) for the full timing model and core
interface — that document is the load-bearing spec.

---

## High-level architecture

```
.hex (from MPLAB)
      │
      ▼
┌─────────────────────────┐     loadHex / runCycles(n) / readPins / setPin
│   WASM core (Rust)      │◄──────────────────────────────────────────────┐
│  single source of truth │                                                │
│  for simulated time     │                                                │
│  • CPU (~35 instrs)     │                                                │
│  • memory + banking     │                                                │
│  • TMR0 + prescaler     │                                                │
│  • interrupts (INT,     │                                                │
│    PORTB-change)        │                                                │
│  • cycle scheduler      │                                                │
└─────────────────────────┘                                                │
      │ pin states                                                         │
      ▼                                                                    │
┌─────────────────────────┐                                                │
│  JS / Canvas runtime    │  each frame: runCycles(clock_hz / 4 / 60)      │
│  • frame loop ──────────┼────────────────────────────────────────────────┘
│  • 7-seg PoV rendering  │
│  • LEDs, buttons        │
│  • loads JSON diagram   │
└─────────────────────────┘
```

- **Core in Rust → WebAssembly.** Runs identically in any browser on any OS. No server.
  This is the clean answer to OS-agnosticism.
- **Runtime in JS/TypeScript + Canvas (or SVG).** Renders components, captures button
  clicks, calls into the WASM core.
- **The core is the single source of truth for time.** The render loop *samples* the
  core's state; it never drives the simulation. (This separation is critical — see the
  architecture doc.)

---

## Two surfaces, one diagram format

The project splits into two tools with different audiences:

1. **Student runtime** (zero editing) — loads a fixed diagram + the student's `.hex`,
   runs it, shows components reacting, lets the student click buttons. Most effort lives
   in the WASM core, not this UI.

2. **Authoring tool** (instructor creates diagrams) — places components and binds each to
   specific pins, saves as a diagram file. **This is deferrable.** A diagram is just a
   JSON file (see [`diagrams/lab-counter.example.json`](diagrams/lab-counter.example.json)),
   so the first labs can be hand-written. A drag-and-drop editor is a later convenience
   that reuses ~80% of the runtime's rendering code.

The pin-binding model is intentionally simple: components bind **directly to pin names**
(e.g. `"RB0"`). There is no "net" or wire concept. A 7-seg maps to 7 named pins, an LED to
one pin, a button drives one pin. That's the whole abstraction.

---

## Suggested repo structure

```
new-proteus/
├── README.md              ← this file
├── PROJECT_PROMPT.md      ← paste into Cowork when starting the project
├── core/                  ← Rust → WASM (the cycle-accurate heart)
│   ├── src/
│   │   ├── cpu.rs         ← fetch/decode/execute, ~35 instrs
│   │   ├── memory.rs      ← banking, SFRs, FSR/INDF
│   │   ├── timer.rs       ← TMR0 + prescaler
│   │   ├── interrupts.rs  ← INT, PORTB-change
│   │   ├── scheduler.rs   ← runCycles(n), cycle accounting
│   │   └── lib.rs         ← WASM exports
│   └── Cargo.toml
├── runtime/               ← student-facing UI
│   ├── index.html
│   ├── sim.js             ← frame loop, calls runCycles(clock/4/60)
│   ├── render.js          ← 7-seg PoV accumulation, LEDs, buttons
│   └── diagram-loader.js
├── diagrams/
│   └── lab-counter.example.json
└── docs/
    └── architecture.md    ← timing model + core interface (the real spec)
```

---

## Rough effort estimate (solo)

| Milestone | Timeline |
|---|---|
| CPU core, cycle-accurate, correct 16F628A firmware (verified vs. MPLAB) | 2–3 weeks |
| TMR0 + prescaler + interrupts (INT, PORTB-change) | 1–2 weeks |
| Cycle-driven scheduler + frame sampling | ~1 week |
| 7-seg with on-time/PoV rendering + LEDs + buttons | 1–2 weeks |
| Student runtime (load hex + JSON diagram, run, click buttons) | ~1 week |
| Hand-authored JSON diagrams | trivial |
| Authoring GUI | deferred |

A correct, browser-based, classroom-trustworthy simulator is realistically a
**~1.5–2 month solo project**, with real firmware running in the first couple of weeks.

---

## The trust anchor: test against MPLAB

The feature that makes this worth building is *correctness you can trust* — the exact
thing SimulIDE lacked. Build a test suite of small PIC programs with known register
outcomes (easy to generate from MPLAB's own simulator) and assert the core matches
cycle-by-cycle. This test-driven approach is what separates "another flaky toy" from
"the tool I actually use in class." Make it part of the core from day one.
