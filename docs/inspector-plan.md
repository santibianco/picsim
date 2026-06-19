# SimuPIC — Live Memory Inspector & Single-Step ("Depurador") — design plan

> **✅ Shipped (2026-06-18).** All six phases below are built, verified in-browser, and
> ready to deploy. Core accessors live in `core/src/{lib,wasm}.rs` (checked by
> `verify-core.js`); the panel is in `runtime/index.html`. Kept as the design record.

Turns SimuPIC from "watch it run" into "see *why* it runs that way." Inspired by MPLAB's
debugger windows. **Read-only** (no memory editing) so the cycle-accurate core is untouched.

## Decisions (agreed)
- **Audience:** everyone — a collapsible "Depurador / Memoria" panel; the default board
  view is unchanged.
- **Control:** inspect **+ single-step** (no breakpoints in v1).
- **Views:** all four — program memory + disassembly, data-memory grid, named SFRs, and a
  watch/filter list.
- **Language:** Spanish UI (consistent); register mnemonics stay canonical (STATUS, PORTB…).

## MPLAB → SimuPIC mapping
| MPLAB window | SimuPIC view |
|---|---|
| Program Memory | **Memoria de programa** — address · opcode · disassembly, PC row highlighted |
| File Registers / Data Memory | **Memoria de datos** — full RAM+SFR map as a live hex grid |
| SFRs | **SFR** — named registers with value + bit breakdown |
| Watches | **Vigilar** — type addresses/register names to track just those (the "filter") |

## Core API (Rust → WASM) — read-only + step
Implement these C-ABI exports. Their **contract is already encoded in `verify-core.js`**,
which doubles as the acceptance test:

- `np_pc() -> u16` — program counter
- `np_w() -> u8` — W register
- `np_cycles() -> u32` — instruction cycles since reset (counter display)
- `np_read_data(addr) -> u8` — data byte at an absolute file-register address (banked map below)
- `np_prog_word(addr) -> u16` — program word at `0x000–0x7FF`
- `np_disasm(word) -> u32` (len) + `np_disasm_buffer() -> ptr` — mnemonic text; **reuses the
  disassembler already in `decode.rs`**, written into a shared buffer like `np_hex_buffer`
- `np_step() -> u8` — execute **exactly one** instruction, return its cycle cost (cycle-exact)

Everything is passive except `np_step`, which advances one instruction. No memory writes.
The 82 tests stay green; `verify-core.js` part (B) checks step + disasm + read_data.

## Data-memory addressing (the careful bit)
16F628A RAM map — get this right so the grid and watch are correct:
- **Bank 0:** SFRs `0x00–0x1F`, GPR `0x20–0x7F`.
- **Bank 1:** SFRs `0x80–0x9F`, GPR `0xA0–0xEF`.
- **Common RAM `0x70–0x7F`** mirrored in both banks.
- `np_read_data` takes the absolute address; the UI maps grid cells / a bank toggle to it.
- Watch-by-name needs an SFR name→address table, e.g. `STATUS=0x03/0x83, PORTA=0x05,
  PORTB=0x06, TRISA=0x85, TRISB=0x86, INTCON=0x0B/0x8B, OPTION=0x81, TMR0=0x01, PCL=0x02,
  FSR=0x04, PCLATH=0x0A, EEDATA=0x9A, EEADR=0x9B, EECON1=0x9C, EECON2=0x9D`, plus bit names
  for STATUS (IRP/RP1/RP0/TO/PD/Z/DC/C), INTCON, OPTION.

## Runtime panel (`runtime/index.html`)
A collapsible **"Depurador"** section under the board (a button expands it):
- **Header strip:** Ciclos (cycle count) · PC · W · STATUS with Z/C/DC/RP0 bits, and a
  **Paso** (step) button next to the existing Pausar / Reiniciar.
- **Views** (tabs or stacked):
  1. **Memoria de programa** — `addr · word(hex) · mnemonic`; current PC row highlighted,
     auto-scrolls to PC on each step. Redraw lazily (only when PC moves / on demand).
  2. **Memoria de datos** — hex grid, Bank 0 / Bank 1 toggle, changed cells flash.
  3. **SFR** — named table: `name · hex · bin · bit flags` for STATUS/INTCON/OPTION/etc.
  4. **Vigilar** — input to add an address (`0x20`) or a name (`PORTB`); shows just those,
     live; remove button; list persisted in `localStorage`.
- **Update cadence:** while running, refresh the *visible* view at ~10 Hz (throttled to stay
  smooth); on Pausar / Paso, refresh immediately. Stepping = one `np_step()` then refresh.

## Phases
1. **Core accessors** — implement the exports to the `verify-core.js` contract; `cargo test`
   + `node verify-core.js` green. (Santiago builds on Windows, embeds from `deps/`.)
2. **Panel scaffold + CPU header + Step** — collapsible panel, Ciclos/PC/W/STATUS readout,
   Paso wired to `np_step`. Verify in browser.
3. **Program memory + disassembly** — list with PC highlight + auto-scroll.
4. **Data-memory grid + named SFRs** — grid (bank toggle) + SFR table with bit breakdowns.
5. **Watch / filter** — add/remove by address or register name, persisted.
6. **Polish** — Spanish labels, changed-value highlight, perf throttle; update README /
   STATUS / architecture docs; push.

## v2 — shipped (2026-06-18)
- **Breakpoints** — click a program row to toggle (red dot); a run stops *before* that word
  executes, and resuming steps once past it. Core: `np_set_break` / `np_clear_break` /
  `np_clear_breaks` / `np_break_hit`; the scheduler stops at a marked PC.
- **Step ×10 / ×100** — blast through delay loops (stops early if it hits a breakpoint).
- **Memory editing** — click a data-grid cell or an SFR value to write it live
  (`np_write_data` / `np_set_w` / `np_set_pc`); editing pauses the run so the refresh can't
  clobber the field.

## Not planned (by request)
- **Symbol/variable names** and a **source-level view** are deliberately omitted, so
  students read the raw machine code — hex addresses, no equs/labels.
- A breakpoint **on a register value** (vs. a PC) could be a future add.

## Cycle-accuracy & rollout safety
All reads are passive — they never advance simulated time; `np_step` advances exactly one
instruction's cycles (checked by `verify-core.js`). The render loop still only *samples*.
**Always run `node verify-core.js` after embedding** — the stale-embed trap is exactly what
broke the app last time, and `verify-core.js` is the guard.
