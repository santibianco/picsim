# New Proteus — project status

A browser-based, OS-agnostic, **cycle-accurate PIC16F628A simulator** for the
classroom. See `README.md` (overview) and `docs/architecture.md` (the
load-bearing spec — timing model + core interface).

## Layout

- `core/` — Rust → WASM cycle-accurate simulation core (the heart). ~60 tests.
- `runtime/` — browser UI (the deployable student app): `index.html` (WASM loader
  + Canvas board), `core-wasm.js` (embedded core, generated), `manifest.json` +
  `sw.js` + `icon.svg` (PWA), `authoring.html` (instructor-only diagram editor).
- `serve.js` (project root) — tiny static server for local dev (`node serve.js`).
- `DEPLOY.md` + `.github/workflows/pages.yml` — GitHub Pages deploy + Moodle embed.
- `examples/` — real MPLAB lab `.hex`/`.asm` pairs used as decode cross-checks.
- `diagrams/` — JSON board definitions (architecture §6); `lab-counter.example.json`.

## Status — implemented + tested

- **CPU**: all 35 PIC14 instructions, STATUS flags (Z/C/DC incl. subtract borrow),
  8-level stack, computed `PCL` jumps, exact cycle counts. (step 1)
- **TMR0** + prescaler + `T0IF`; **interrupt** vectoring (GIE, T0IE/INTE/RBIE),
  `RETFIE`. (step 2)
- **Per-pin on-time sampling** for 7-seg persistence of vision. (step 3)
- **Input pins**: TRIS-aware reads + `set_pin` (buttons) + RB0/INT + PORTB-change. (step 4 core)
- **WASM**: dependency-free C-ABI (no wasm-pack) — `core/src/wasm.rs`.
- **Browser runtime**: DIP-18 board, LEDs / 7-seg / buttons, clock control, JSON
  diagram loading, blink + 7-seg + 2-digit-mux + EEPROM demos. Verified in-browser.
- **EEPROM** (EECON1/EECON2 unlock; persists across reset/power-cycle). Demo
  climbs 1→2→3 across the Reset (power-cycle) button. (step 5)
- **Authoring tool** (`runtime/authoring.html`, instructor-only): visual editor —
  add LED/button/7-seg, assign pins (per-segment for 7-seg), button polarity +
  labels, live preview, export/import JSON. Round-trips to the student runtime.
- **PWA**: `manifest.json` + network-first `sw.js` (offline + installable) +
  responsive canvas. Deploy via `.github/workflows/pages.yml`; see `DEPLOY.md`.

## Build / test / run

```sh
# 1. test the core
cd core && cargo test

# 2. build the wasm (Windows host)
cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib

# 3. embed it so the page is self-contained (run from project root).
#    GOTCHA: `cargo rustc --crate-type cdylib` writes the .wasm to release/DEPS/,
#    not the top-level release/ dir — embed from deps/ (the freshly built one).
node -e "const fs=require('fs');const b=fs.readFileSync('core/target/wasm32-unknown-unknown/release/deps/new_proteus_core.wasm').toString('base64');fs.writeFileSync('runtime/core-wasm.js','window.NP_WASM_BASE64=\"'+b+'\";');"

# 4. run the runtime
node serve.js        # -> http://localhost:8080
```

Division of labor: the wasm build runs on the Windows host; the embed step and
file wrangling can be done from the agent's mounted shell.

## Pending / next

- **Deploy**: push to GitHub + enable Pages (Settings → Pages → Source = GitHub
  Actions); see `DEPLOY.md`. Then embed the URL in Moodle.
- Bundled "Select lab" dropdown in the student app — for now students use
  *Load diagram (.json)* + *Load firmware (.hex)*.
- Authoring niceties: drag-to-position, "Test against a .hex" preview, honor
  `x`/`y` in the runtime (currently auto-placed around the chip).
- (simplified) internal pull-ups (RBPU) — not modeled; add if a lab needs it.

## Reconciliation items (vs MPLAB)

- Interrupt entry-cycle count (`INTERRUPT_ENTRY_CYCLES = 2`) and exact TMR0
  reload-ISR period — confirm against MPLAB's stopwatch on a timer lab.
- External TMR0 clock (`T0CS = 1`, counting T0CKI edges) is not modeled.
