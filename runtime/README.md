# New Proteus — browser runtime

A single self-contained page that runs the WASM core and renders a board: the
PIC16F628A drawn as a DIP-18 chip with labeled pins, and components wired to it.

## One-time setup (only repeat when the Rust **core** changes)

1. **Build the core to WASM** — from `core/`:

   ```sh
   cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib
   ```

2. **Embed it** so the page only needs a `.hex` — from the project root:

   ```sh
   node -e "const fs=require('fs');const b=fs.readFileSync('core/target/wasm32-unknown-unknown/release/new_proteus_core.wasm').toString('base64');fs.writeFileSync('runtime/core-wasm.js','window.NP_WASM_BASE64=\"'+b+'\";');"
   ```

   This writes `runtime/core-wasm.js`. Now the page loads the core automatically.

## Everyday use

Open `runtime/index.html` (double-click — works from `file://`). The core is
already loaded, so just **Load firmware (.hex)** (or **Blink demo**) → **Run**.

> If `core-wasm.js` isn't present, the page falls back to a **Load core (.wasm)**
> file button. Embedding is just the convenience that removes that step — and
> makes `index.html` + `core-wasm.js` a self-contained thing you can hand to
> students.

## What you'll see

- **Blink demo** — the 8 LEDs (RB0–7) run as a binary counter. The **Speed**
  slider is log-scaled: the left third is the slow, step-by-step range; the right
  end is real-time (16,667 cyc/frame at 4 MHz).
- **`examples/Simple one/Punto A.hex`** — drives PORTB to 0xFF → all 8 LEDs on.
- The **RA4 button** is click-and-hold (drives the input pin via `set_pin`).

## Rendering

The frame loop sub-divides each frame, samples PORTA/PORTB at each sub-step, and
accumulates per-pin on-time; brightness is that fraction (architecture.md §4). A
7-seg display renderer exists in the page and will be wired in with a
display-specific diagram (and a matching multiplexing `.hex`).

## Next

- Instructor-authored JSON diagrams (architecture.md §6) instead of the built-in
  demo board.
- A multiplexed 7-seg demo (needs a re-exported TP 2022 `.hex`); **turnos** also
  needs EEPROM, still stubbed in the core.
