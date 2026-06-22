# SimuPIC — Browser-Based PIC16F628A Simulator

**Live:** <https://santibianco.github.io/SimuPIC/>

A web-based, OS-agnostic, **cycle-accurate** simulator for running and visually testing
PIC16F628A firmware against simple digital components (LEDs, 7-segment displays, buttons).

Built for the **classroom**: students write firmware in MPLAB, compile to a `.hex`, open
the URL (or a Moodle embed), pick the lab board, load their `.hex`, and watch it drive a
virtual board — on any laptop or phone, online or offline. (Originally codenamed
*New Proteus*.)

---

## Status — shipped ✅

Everything below is implemented, tested, and deployed.

- **Cycle-accurate CPU** — all 35 PIC14 instructions, STATUS flags (Z / C / DC incl.
  subtract borrow), 8-level stack, banked SFRs/RAM, FSR/INDF indirection, computed `PCL`
  jumps, and exact per-instruction cycle counts.
- **Peripherals** — TMR0 + prescaler + `T0IF`; interrupts (GIE, TMR0 / INT / PORTB-change,
  `RETFIE`); **EEPROM** (EECON1/EECON2 unlock sequence, persists across reset / power-cycle).
- **I/O** — TRIS-aware port reads, input pins & buttons (`set_pin`, RB0/INT, PORTB-change),
  and per-pin on-time sampling for 7-segment persistence-of-vision.
- **Browser runtime** — DIP-18 board with LEDs / 7-seg / buttons, processor-clock control,
  a "pick a board" lab selector, four built-in demos, and JSON diagram loading. **Spanish
  (es-AR) UI.** Installable **PWA** that runs fully offline after the first visit.
- **Debugger (Depurador)** — a collapsible, read-only inspector for every student:
  single-step (*Paso*), a live PC / W / STATUS / cycle header, program memory with
  disassembly and a highlighted PC, the data-memory grid (bank 0/1), named SFRs with bit
  breakdowns, the 8-level **hardware call stack** (*Pila*), breakpoints + live memory editing,
  and a watch/filter by address or register name. MPLAB-style and cycle-exact.
- **Authoring tool** — instructor-only visual editor: place components, bind pins
  (per-segment for a 7-seg), set button polarity, and export the diagram JSON.
- **Trust** — **82 Rust tests** plus cross-checks against real MPLAB lab `.hex`/`.asm` pairs.

**Deployed** to GitHub Pages by a GitHub Action on every push, and embeddable in Moodle —
see [`DEPLOY.md`](DEPLOY.md). Day-to-day status and build commands live in
[`STATUS.md`](STATUS.md); the design spec is [`docs/architecture.md`](docs/architecture.md).

---

## Why this exists

The two existing options both fail for teaching digital PIC work:

- **SimulIDE** — free and runs natively, but its PIC core is incomplete. Bit-level
  instructions like `BSF`/`BCF` don't behave correctly, so students end up debugging
  *correct* code. Once a simulator can't be trusted, it's useless as a teaching tool.
- **Proteus** — accurate, but paid, Windows-only, and far heavier than needed. Its crown
  jewel is SPICE-level analog co-simulation, which is overkill for a classroom that only
  needs digital LEDs / displays / buttons. On Apple Silicon it also requires a Windows VM.

SimuPIC deliberately builds the *small, correct* tool that's missing: an accurate
single-chip digital simulator that runs in any browser on any OS.

---

## The non-negotiable requirement: cycle-accuracy

This is the single most important property of the project, and the thing SimulIDE got
wrong. The two techniques students rely on — **display multiplexing** and **button
debouncing** — are both fundamentally about *time*:

- **Multiplexing** cycles through digits faster than the eye can detect (~kHz), relying on
  persistence of vision to make all digits appear lit.
- **Debouncing** measures an elapsed-time window (~5–20 ms) to reject contact noise.

If the simulator's count of elapsed instruction cycles drifts from a real PIC, both
features break in ways that make students debug *correct* code. So cycle-accuracy is not
polish; it is the core requirement — and it's what the 82-test suite defends.

The 16F628A makes this achievable: every instruction is **1 instruction cycle (4 clocks)
except branches and skips-taken, which are 2**. At 4 MHz, **1 instruction cycle = 1 µs**.
No pipeline, no cache, no variable latency — cycles are counted exactly by construction.

---

## Using SimuPIC

### Students
1. Open <https://santibianco.github.io/SimuPIC/> (or the Moodle activity your instructor set up).
2. Pick your lab from the **Placa** dropdown (e.g. *TP - Simple*).
3. **Cargar programa (.hex)** — load the `.hex` you built in MPLAB.
4. Watch it run and click the buttons (*Pulsador*) to interact. **Reiniciar** power-cycles
   the chip (RAM clears, EEPROM survives); **Reloj** changes the processor frequency.

The first time you open it online, the browser offers to **Install** it — afterward it
runs fully offline, as an app, with no internet needed.

### Instructors
1. Build a board in the authoring tool (`runtime/authoring.html`, run locally — see below):
   add LEDs / 7-seg / buttons, assign pins, set button polarity, label them.
2. **Export JSON** and paste it into [`runtime/labs.js`](runtime/labs.js) as a
   `{ "name": "...", "components": [ ... ] }` entry.
3. `git push` — the Action redeploys and your board appears in every student's **Placa**
   dropdown.

---

## Architecture

```
.hex (from MPLAB)
      │
      ▼
┌─────────────────────────┐     load_hex / run_cycles(n) / read_pins / set_pin
│   WASM core (Rust)      │◄──────────────────────────────────────────────┐
│  single source of truth │                                                │
│  for simulated time     │                                                │
│  • CPU (35 instrs)      │                                                │
│  • memory + banking     │                                                │
│  • TMR0 + prescaler     │                                                │
│  • interrupts + EEPROM  │                                                │
│  • cycle scheduler      │                                                │
└─────────────────────────┘                                                │
      │ pin states                                                         │
      ▼                                                                    │
┌─────────────────────────┐                                                │
│  JS / Canvas runtime    │  each frame: run_cycles(clock_hz / 4 / 60)     │
│  • frame loop ──────────┼────────────────────────────────────────────────┘
│  • 7-seg PoV rendering  │
│  • LEDs, buttons        │
│  • loads JSON diagram   │
└─────────────────────────┘
```

- **Core in Rust → WebAssembly.** Dependency-free C-ABI (no wasm-pack), base64-embedded
  into the page so the app is a self-contained static bundle. Runs identically in any
  browser on any OS — no server. This is the clean answer to OS-agnosticism.
- **Runtime in a single HTML file + Canvas.** Renders components, captures button clicks,
  and calls into the WASM core.
- **The core is the single source of truth for time.** The render loop *samples* the
  core's state each frame; it never drives the simulation. This separation is what makes
  persistence-of-vision and debounce timing behave like real hardware.

The pin-binding model is intentionally simple: components bind **directly to pin names**
(e.g. `"RB0"`). There is no net/wire concept — a 7-seg maps to 7 named pins, an LED to one
pin, a button drives one pin. That's the whole abstraction.

---

## Scope (deliberately narrow)

These constraints are what made the project achievable. **Do not expand scope without
revisiting the timing/architecture implications.**

- **One chip only: PIC16F628A.** Flash, RAM, SFR layout, banking, and peripherals are all
  hardcoded to this part.
- **Digital-only I/O.** No analog, no SPICE. A pin is `0` or `1`.
- **Components: LEDs, 7-segment displays, buttons only.**
- **Students don't build diagrams** — they load their `.hex` against an instructor's board.

Explicitly out of scope: analog/SPICE, freeform drawn wiring, any other PIC, a built-in
assembler (firmware arrives as Intel HEX from MPLAB), and character LCDs.

### Known simplifications

- **Internal pull-ups (RBPU) are not modeled.** Use external pull-ups/downs, or let the
  diagram define the button's idle level (`activeLow`) — which is how the demos work.
- **External TMR0 clock (T0CKI counting, `T0CS = 1`) is not modeled** — TMR0 counts the
  internal instruction clock.

These are the only digital-behavior gaps; everything else tracks the datasheet.

---

## Repo structure

```
SimuPIC/
├── README.md                  ← this file
├── STATUS.md                  ← live status + build/test/run commands
├── DEPLOY.md                  ← GitHub Pages deploy + Moodle embed
├── PROJECT_PROMPT.md          ← original project brief
├── serve.js                   ← tiny static dev server (node serve.js → :8080)
├── .github/workflows/pages.yml← deploys runtime/ to GitHub Pages on push
├── core/                      ← Rust → WASM, the cycle-accurate heart
│   ├── src/
│   │   ├── cpu.rs             ← fetch/decode/execute, TMR0 tick, interrupts, EEPROM, sampling
│   │   ├── decode.rs          ← 35-instruction decode + disassembler
│   │   ├── memory.rs          ← banking, SFRs, TRIS-aware ports, FSR/INDF
│   │   ├── timer.rs           ← TMR0 + prescaler
│   │   ├── interrupts.rs      ← INT, TMR0, PORTB-change vectoring
│   │   ├── scheduler.rs       ← run_cycles(n), cycle accounting, frame sampling
│   │   ├── sampler.rs         ← per-pin on-time accumulation (PoV)
│   │   ├── hex.rs             ← Intel HEX loader (program + config + EEPROM)
│   │   ├── wasm.rs            ← dependency-free C-ABI exports (np_*)
│   │   └── lib.rs             ← Core API
│   ├── tests/                 ← 82 tests: integration.rs, examples.rs, fixtures/
│   └── Cargo.toml
├── runtime/                   ← the deployable student app
│   ├── index.html             ← WASM loader + Canvas board + Spanish UI + lab dropdown
│   ├── core-wasm.js           ← base64-embedded WASM core (generated)
│   ├── labs.js                ← instructor lab boards (the "Placa" dropdown)
│   ├── manifest.json + sw.js + icon.svg   ← PWA (installable + offline)
│   └── authoring.html         ← instructor-only diagram editor
├── diagrams/                  ← example diagram JSON
├── examples/                  ← real MPLAB lab .hex/.asm pairs (decode cross-checks)
└── docs/
    └── architecture.md        ← timing model + core interface (the load-bearing spec)
```

---

## Build / develop

```sh
cd core && cargo test                                   # 1. run the 82-test suite
cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib   # 2. build WASM
# 3. embed it (the cdylib lands in release/DEPS/ — see STATUS.md step 3):
node -e "const fs=require('fs');const b=fs.readFileSync('core/target/wasm32-unknown-unknown/release/deps/new_proteus_core.wasm').toString('base64');fs.writeFileSync('runtime/core-wasm.js','window.NP_WASM_BASE64=\"'+b+'\";');"
node serve.js                                           # 4. run → http://localhost:8080
#                                                       #    instructor editor → :8080/authoring.html
```

The WASM build runs on the host (needs Rust + the `wasm32-unknown-unknown` target);
the embed and serving are plain Node. Full notes and gotchas: [`STATUS.md`](STATUS.md).

## Deploy

GitHub Pages, built by [`.github/workflows/pages.yml`](.github/workflows/pages.yml).
One-time: **Settings → Pages → Source = GitHub Actions**, then `git push`. The workflow
publishes `runtime/` (without the instructor authoring page) to
<https://santibianco.github.io/SimuPIC/>, and the same URL embeds in Moodle via an iframe.
Step-by-step: [`DEPLOY.md`](DEPLOY.md).
