# SimuPIC — project status

A browser-based, OS-agnostic, **cycle-accurate PIC16F628A simulator** for the
classroom (codename *New Proteus*). **Shipped and live:**
<https://santibianco.github.io/SimuPIC/>. See `README.md` (overview), `DEPLOY.md`
(hosting + Moodle), and `docs/architecture.md` (the load-bearing spec).

## Session log (newest first) — update this at the end of each session

- **2026-06-18** — Added a read-only **debugger (Depurador)**: core accessors
  (`np_pc/np_w/np_cycles/np_read_data/np_prog_word/np_disasm/np_step` in `core/src/lib.rs`
  + `wasm.rs`, unit-tested + checked by `verify-core.js`) and a collapsible runtime panel
  with single-step and Programa / Datos / SFR / Vigilar tabs. Also: cross-frame brightness
  persistence (slow-multiplex fix), the **SimuPIC** rename, and `embed-core.js` (a
  PowerShell-safe replacement for the inline `node -e` embed). *Uncommitted at write time.*

## Layout

- `core/` — Rust → WASM cycle-accurate simulation core (the heart). 82 tests.
- `runtime/` — browser UI (the deployable student app): `index.html` (WASM loader
  + Canvas board), `core-wasm.js` (embedded core, generated), `labs.js` (instructor
  lab boards → the "pick a board" dropdown), `manifest.json` + `sw.js` + `icon.svg`
  (PWA), `authoring.html` (instructor-only diagram editor).
- `serve.js` (project root) — tiny static server for local dev (`node serve.js`).
- `embed-core.js` / `verify-core.js` — embed the built wasm into `core-wasm.js`, then
  validate it (EEPROM + debugger fixtures). Always run `verify-core.js` after embedding.
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
- **Browser runtime**: DIP-18 board, LEDs / 7-seg / buttons, clock control, a
  "pick a board" lab dropdown (`labs.js`), JSON diagram loading, four demos, and a
  **Spanish (es-AR) UI**. Verified in-browser.
- **Debugger (Depurador)**: collapsible read-only inspector for everyone — single-step
  (`np_step`), live Ciclos/PC/W/STATUS header, program memory with disassembly + PC
  highlight, data-memory grid (bank 0/1), named SFRs with bit breakdowns, and a watch/
  filter (by address or register name, persisted in `localStorage`). Cycle-exact.
- **EEPROM** (EECON1/EECON2 unlock; persists across reset/power-cycle). Demo
  climbs 1→2→3 across the Reset (power-cycle) button. (step 5)
- **Authoring tool** (`runtime/authoring.html`, instructor-only): visual editor —
  add LED/button/7-seg, assign pins (per-segment for 7-seg), button polarity +
  labels, live preview, export/import JSON. Round-trips to the student runtime.
- **PWA**: `manifest.json` + network-first `sw.js` (offline + installable) +
  responsive canvas. Deploy via `.github/workflows/pages.yml`; see `DEPLOY.md`.
- **Deployed & live** at <https://santibianco.github.io/SimuPIC/> (GitHub Pages via
  Action on every push); embeddable in Moodle. Bundled instructor boards:
  *TP - Simple*, *TP - Dificil*.

## Build / test / run

```sh
# 1. test the core
cd core && cargo test

# 2. build the wasm (Windows host)
cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib

# 3. embed it so the page is self-contained (run from project root).
#    GOTCHA: `cargo rustc --crate-type cdylib` writes the .wasm to release/DEPS/.
#    Use the script — the old inline `node -e` one-liner gets mangled by PowerShell.
node embed-core.js

# 3b. VERIFY the embed before trusting it — validates the wasm and runs the EEPROM
#     + debugger fixtures. Must print both ✓.
node verify-core.js

# 4. run the runtime
node serve.js        # -> http://localhost:8080
```

Division of labor: the wasm build runs on the Windows host; the embed step and
file wrangling can be done from the agent's mounted shell.

## Pending / next (all optional — the project is shipped)

- Embed the live URL in the Moodle course (iframe snippet in `DEPLOY.md`).
- More instructor lab boards: build them in `authoring.html`, paste the export
  into `runtime/labs.js`, push → they appear in the student "pick a board" dropdown.
- Authoring niceties: drag-to-position, a "Test against a .hex" preview, honor
  `x`/`y` in the runtime (currently auto-placed around the chip).
- (simplified) internal pull-ups (RBPU) — not modeled; add if a lab needs it.

## Reconciliation items (vs MPLAB)

- Interrupt entry-cycle count (`INTERRUPT_ENTRY_CYCLES = 2`) and exact TMR0
  reload-ISR period — confirm against MPLAB's stopwatch on a timer lab.
- External TMR0 clock (`T0CS = 1`, counting T0CKI edges) is not modeled.
