# SimuPIC — project status

A browser-based, OS-agnostic, **cycle-accurate PIC16F628A simulator** for the
classroom (codename *New Proteus*). **Shipped and live:**
<https://santibianco.github.io/SimuPIC/>. See `README.md` (overview), `DEPLOY.md`
(hosting + Moodle), and `docs/architecture.md` (the load-bearing spec).

## Session log (newest first) — update this at the end of each session

- **2026-06-24 (UI revamp 2 — desktop/mobile usability)** — Reworked the runtime UI for legibility and
  to let students see more at once (`runtime/index.html` only — **CSS + HTML + JS, no core/wasm change**;
  the 82/83 tests + the embed are untouched, **no rebuild**). **(1) IDE-style layout.** The old top-to-bottom
  flow (board centred, Depurador a collapsible panel stacked under it) became a 3-zone CSS-grid workspace:
  **left controls rail · large board centre · resizable right-docked Depurador**, so the debugger is visible
  *beside* the board. New app-bar **"Depurador" toggle** (show/hide, persisted `np_dock`) and a **drag handle**
  to resize the dock (persisted `np_dock_w`, 320–720px). Responsive: ≤1200px the dock un-docks to a full-width
  panel below the board; ≤920px everything stacks (rail cards wrap). **(2) Two-pane customizable debugger**
  (the headline ask). On desktop the Depurador shows **two panes side by side, each with its own view picker**
  (Programa / Datos / SFR / Pila / Vigilar) — a student can watch e.g. the program *and* a watch list / the
  SFRs at the same time; per-pane choice + data bank persist (`np_panes`). The **second pane is closable** (✕ on
  its header) and reopens via a **⊞ button** on the first pane (persisted, `np_split`) — the side-by-side view is
  optional. Below 1200px it collapses to a single pane (dual view is desktop-only, by design). Rewrote the old single-tab view system: each view now renders
  into a **pane-scoped** container, with **delegated** handlers on `#dbgPanes` for breakpoints, bank switch,
  watch add/remove and inline cell/SFR hex edit — **all prior debugger features preserved** (breakpoints, Paso
  ×1/×10/×100, live PC highlight, inline edit, watch persistence, the 8-level stack). **(3) Board zoom + pan.**
  Scroll/pinch to zoom (about the cursor), drag to pan (clamped to the board), a floating **−/100%/+/Ajustar**
  toolbar + double-click to fit. The canvas now draws through a **view transform on a hi-DPI backing store**
  (`devicePixelRatio`), so it stays crisp at any zoom; **pointer events** unify mouse + touch and a press on a
  component still beats panning; zoom resets on board load. **(4) Legibility.** Bolder/larger pin labels,
  larger live pin-state squares, 7-seg now shows faint **ghost (unlit) segments** so the digit shape is always
  readable plus brighter lit segments, and clearer buttons. **(5) Bug fixed:** the live PC highlight used
  `scrollIntoView`, which scrolled the *whole page* — on mobile it yanked the view down to the dock every
  frame; it now scrolls only inside the program pane. **Verified live in Chrome at 1440 / tablet / 390px:**
  mux counter runs, two-pane Depurador live (Programa+Vigilar / +SFR / +Pila), breakpoints + step + inline
  edit, wheel/button/drag zoom + pan + dock-resize + dock toggle, theme + clock + ASM editor intact,
  single-pane + no page-jump on mobile, **zero console errors**. Pure runtime change → no wasm
  rebuild/embed/verify-core needed. *Uncommitted.*

- **2026-06-24 (dock: editor moved into the side panel)** — Follow-up to the revamp above
  (`runtime/index.html` only, no core change). The **right dock now holds two independent, stackable blocks
  — "Depurador" and "Código (ASM)"** — each with its own ✕ and its own **navbar toggle** (the app bar now has
  *Código* and *Depurador* buttons instead of one *Panel* button). Both can be open **at the same time** (they
  split the dock height, each ~half, resizable via the existing edge drag); closing both hides the dock. The ASM
  editor was thus **moved out of the bottom of the stage into this dock**, and its textarea now **fills the block
  height** (the old bottom panel made it cramped). Selecting a demo that carries `.asm` opens the Código block
  (debugger stays as-is). State persists per block (`np_dock`, `np_editor`); defaults: debugger open on desktop,
  editor closed (open it from the navbar). Verified live: both blocks open together, independent toggles + ✕,
  editor full-height + compiles, demo-source auto-opens Código, no console errors. On phones (≤640px) the
  navbar toggles are **icon-only** (button labels + the chip tag are hidden) so the bar isn't cramped.
  Also, the Depurador navbar button now uses a **bug icon** (standard debugger symbol). And on mobile (≤920px)
  the controls reflow **top→down: Placa · Archivos · board · Simulación · status** — done with `.rail{display:contents}`
  so the rail's cards become direct grid items and `grid-template-areas` orders them around the board (desktop rail
  layout unchanged). *Uncommitted.*

- **2026-06-23 (editor: .asm open/save + example source)** — Editor (`runtime/index.html`) gains **Abrir
  .asm** / **Descargar .asm** (load a local `.asm` into the editor; save the editor to a `programa.asm`
  file via a Blob), and examples can now **carry their source**: a lab with an `asm` field populates the
  editor and opens it when selected, so students see the code behind a demo, not just the hex (loading
  still goes through the existing `hex`, so behaviour is unchanged). Wired the mechanism + attached a
  **verified, readable, commented `.asm` to all four built-in demos** (Parpadeo, 7-seg counter, 2-digit
  mux counter, EEPROM counter). Each was reconstructed from its hardcoded hex with a small PIC14
  disassembler, then hand-cleaned (labels, SFR names, Spanish comments) and confirmed to **re-assemble
  byte-identical** to the original hex — so selecting a demo shows faithful source in the editor that
  round-trips back to the same program. Loading still runs the hex; the `.asm` is editor source. The
  reconstructed sources live as `*_ASM` consts next to the `*_HEX` ones in `runtime/index.html`.
  Also dropped the now-redundant **Cargar ejemplo** button — examples carry their own source now (the
  built-in counter sample still loads into an empty editor on first open). *Uncommitted.*

- **2026-06-23 (board: button-on-output warning)** — New 4th pin-square colour on the chip: **amber =
  a button is wired to a pin the firmware configured as an OUTPUT and it's pressed** — surfaces a
  common beginner mistake (forgetting the button pin needs to stay an input / wrong TRIS) instead of
  the button silently doing nothing. `runtime/index.html` only (canvas draw): `drawChip` now checks the
  pin's TRIS bit plus the wired button's `pressed` flag (button objects carry their `pin`). Verified
  live: Parpadeo board, RA4 forced to output + button held → RA4 square goes amber while its PORTA
  neighbours stay blue. No core/assembler change. *Uncommitted.*

- **2026-06-23 (ASM editor — Phase 2: macros)** — The in-browser assembler now handles the full MPASM
  preprocessor the labs use: **`CBLOCK`/`ENDC`** (auto-incrementing RAM allocation → synthetic EQUs),
  **`#define`** text macros (incl. multi-token values like `PORTB,0` and instruction-valued ones like
  `CLRF PORTA`, substituted recursively), and **`MACRO`/`ENDM`** with parameters (invocations expanded
  with arg substitution; bodies re-preprocessed for nested #defines/macros; a label on the invocation
  line is handled). Added as a preprocessing pass in `runtime/asm.js` (`preprocess` / `expandLine` /
  `applyDefines` / `preParse`) that runs before the existing two passes — no change to encoding or timing.
  `DT`/`DW`/`DE`/`FILL`/`RES` stay out of scope (clean reject). **Validated byte-for-byte against the REAL
  compiler:** ran the installed `MPASMWIN.exe` on all four example labs via `mpe2e/run.bat` and diffed —
  **Punto A 9/9, Multiplicación 10/10, TP-turnos 107/107, and the macro/CBLOCK/#define TP 2022 119/119
  program words + config word, zero diffs, zero extra words.** (Heads-up: `examples/Another large/TP 2022
  - Ejemplo.hex` is a *non-corresponding* revision — 0/119 vs the `.asm` — so `test-asm.js` only checks
  TP 2022 *assembles*; the MPASM diff is the real check. The other 3 `.hex` pairs still match with the
  usual 2 fill-word drift.) `test-asm.js` updated (TP 2022 case reject→assembles). `S:\New Proteus\mpe2e\`
  is throwaway MPASM-comparison scratch (safe to delete / gitignore). *Uncommitted.*

- **2026-06-23 (built-in ASM editor)** — Added an in-browser **"Editor de código (ASM)"** so students
  can write MPASM and compile to a runnable `.hex` on their phone, no MPLAB needed. **New
  `runtime/asm.js`** — a pure-JS two-pass PIC16F628A assembler: all 35 instructions, labels, `EQU`/`SET`,
  `ORG`, `__CONFIG` (numeric + symbolic `_A & _B`), `BANKSEL` (→ bcf/bsf `STATUS,RP0` then `RP1`, 2
  words), `GOTO $`, the `.dec`/`0x`/`h''`/`b''`/`d''`/`'c'` radixes (bare digit-led = hex, MPASM
  default), a built-in p16f628a symbol table (SFRs + bit names + config consts, so `#include` is a
  no-op), and `END`. Emits Intel HEX into the **existing `loadHex()` path unchanged — no core/WASM
  change, the 83 tests and the embed are untouched, no rebuild**. CBLOCK/`#define`/MACRO are out of MVP
  scope and **rejected with a clear Spanish "compilá en MPLAB por ahora"** message rather than
  mis-assembled. `index.html` — a collapsible **Código** panel mirroring the Depurador (monospace
  textarea + "Compilar y cargar"; success → `loadHex`+`afterHex`, runs like a file load; error → `línea
  N: motivo`; source saved in `localStorage` `np_src`; a built-in counter example). `sw.js` — `asm.js`
  added to the precache SHELL, cache **v1→v2** (offline-safe). **Validated by a new Node oracle
  `test-asm.js`** (no Rust): assembles the real `examples/*.asm` and diffs every program word vs the
  matching MPLAB `.hex` — **Punto A, Multiplicación and TP-turnos match byte-for-byte (126 instruction
  words, 0 contradictions)**; the macro/CBLOCK `TP 2022` is rejected cleanly. (TP-turnos's config word
  differs, 0x3F24 vs 0x3F30 — the `.asm`/`.hex` are slightly revision-drifted, also visible as 2 stray
  fill words per file; the config word doesn't affect the sim.) Sample compiles (13 instr, checksums
  OK); editor JS parses. **Range-check fix (Santiago caught it):** out-of-range operands
  now error instead of silently masking — `movlw .20000` → "literal fuera de rango (0-255)",
  plus goto/call (0-0x7FF) and file-register (0-0x1FF, via `fileAddr`) checks; `clrf TRISB`
  (0x86→low 7 bits) still valid, oracle still byte-exact, confirmed live in the browser.
  **Config-constant fix (via the real `p16f628a.inc`, which Santiago attached):** cross-checked
  every symbol table against the authoritative inc — **SFR 35/35 and bit names 74/74 exact**, and
  `__MAXRAM H'1FF'` confirms the file-register ceiling. But my `__CONFIG` table had **PWRTE and WDT on
  the wrong bits** (real chip: PWRTE=bit3 `_PWRTE_ON=0x3FF7`, WDTE=bit2 `_WDT_OFF=0x3FFB`; I'd had them
  one bit off each). Regenerated the whole CFG verbatim from the inc → **TP-turnos's config word is now
  0x3F30, matching MPLAB exactly** (so the example `.hex` was right all along; my constants were the bug,
  not "revision drift"). All three assembled examples now match MPLAB byte-for-byte *including config*.
  **End-to-end confirmation against the real compiler:** ran `MPASMWIN.exe` (installed MPASM Suite) on
  all three sources via `S:\New Proteus\mpe2e\run.bat` and diffed its `.hex` against the in-browser
  output — **byte-for-byte identical on every program word AND the config word** (puntoa 9/9, mult 10/10,
  turnos 107/107; zero diffs, zero extra words). MPASM's only output was `Message[302]` bank reminders.
  (Confirms the 2 stray fill words vs the *old* example `.hex` were source drift — a fresh compile has
  none.) `S:\New Proteus\mpe2e\` is throwaway test scratch, safe to delete.
  *Uncommitted.*

- **2026-06-22 (transport controls)** — Replaced the Ejecutar/Pausar/Reiniciar **text** buttons with
  **media-transport icon controls** in the Simulación card: a **morphing play↔pause toggle** (shows ▶
  when paused/stopped, ⏸ while running; `aria-label`/title swap to match) plus a **Stop** button. The
  **accent highlight is on the ▶ play state** (the primary "press me to run" action); the running/pause
  and Stop states are neutral.
  `runtime/index.html` only (HTML + CSS + control rewiring, **no core change**).
  **Stop = halt + power-cycle reset** (`np_reset`, stopwatch → 0, components idled) — so it both stops
  *and* rewinds, where the old Reiniciar reset but kept running. New `setRunning/play/pause/stop/
  togglePlay` replace the `runBtn`/`resetBtn` toggle; Space now calls `togglePlay`; the two status
  strings that named "Reiniciar" now say "Detener". **Verified live:** auto-run lights play + advances
  cycles; pause freezes cycles; play resumes; stop zeroes cycles + stopwatch; Space toggles play/pause;
  no console errors. **Stop also blanks the *board***, not just the core: each LED/7-seg/button gained a
  `clear()` that zeroes its persisted brightness (`b`) + accumulators, called from `stop()` — so a
  stopped board visibly goes dark and resets, instead of freezing the last lit frame the way Pause does
  (which is what made pause/stop look identical until you pressed play). Verified: a lit display reads
  brightness 6.5 → Pause leaves it 6.5 → Stop drops it to 0. *Uncommitted.*
- **2026-06-22 (security hardening)** — Acted on `SECURITY-REVIEW.md`. Baseline risk was already
  low (static, client-only, no data / secrets / accounts), so this closes the two real items + cheap
  defense-in-depth, **with no core/wasm change — the 83 tests and the embed are untouched, no rebuild
  needed**. `runtime/index.html`: **M1** the status line uses `textContent`, not `innerHTML`, so a
  malicious `.hex` filename or lab name can't run script (the one cross-user vector — an instructor
  opening a student's file); **L1/L4** dropdown names/groups + watch labels escaped via a shared
  `esc()`; **I1** `np_set_pin` is skipped when a pin name is invalid (`pinIndex<0`) so a malformed
  `labs.js` can't trap the sim; **L2** added a `<meta>` CSP (`default-src 'none'; connect-src 'self';
  object-src 'none'; base-uri 'none'; …`) — `script-src` keeps `'wasm-unsafe-eval'` so the embedded
  WASM still instantiates. `runtime/authoring.html`: **L5** the editor card escapes its label/type
  interpolations (self-XSS on JSON import). `serve.js`: **M2** `/__save_labs` now requires a custom
  `X-SimuPIC` header (a cross-origin page sending it would trigger a CORS preflight we never answer)
  plus a same-origin `Sec-Fetch-Site`, and the path-prefix check is tightened — closes the dev-box
  CSRF; `authoring.html` sends the header (**restart `serve.js` for this to take effect**). **Verified
  live** (reloaded the open tab): core loads, sim runs (cycles advance, PORTB updates, stopwatch
  ticks), SW registers, dropdown populates — **zero console errors / no CSP violations** — and a
  direct test confirmed an injected `<img onerror>` through the status line renders as inert text.
  Deliberately skipped as higher-effort / low-value: the full `script-src 'self'` refactor (would
  break the single self-contained `index.html`) and the Moodle-iframe `sandbox`. *Uncommitted.*
- **2026-06-18 (stack view)** — Added the debugger's **"Pila" view** — the PIC's 8-level CALL/RETURN
  hardware stack, showing depth + the return address at each level (top marked). Needs a **core
  change** (the stack isn't memory-mapped): `Cpu::stack_depth`/`stack_at` → `Core` → WASM
  `np_stack_depth`/`np_stack_at` (`core/src/{cpu,lib,wasm}.rs`), a `stack_view_surface` unit test
  (→ 83 tests), and a `verify-core.js` part (C). Runtime `index.html` has the Pila tab/view; it
  degrades to a "recompilá el núcleo" note on an old core. **Done: rebuilt + embedded (59,884-byte
  wasm); `verify-core.js` all ✓ incl. `stack surface … ✓`; 83 tests green; verified live in-browser
  — a CALL shows depth 0→1, level 0 = 0x001, top marked.** *Uncommitted.*
- **2026-06-18 (persistence of vision)** — Reworked 7-seg brightness to model **honest persistence
  of vision** (`runtime/index.html`). Each segment decays toward 0 with a ~45 ms time constant (the
  eye's persistence) and is pulled to full while *actually lit* (its digit selected AND the segment
  driven). So a mux faster than flicker-fusion fuses to a **solid, bright** display, while a mux
  slower than the eye **visibly flickers** — like real hardware, and deliberately *unlike* Proteus,
  which hides a slow refresh. Time-based → frame-rate-agnostic. **Design call (Santiago): keep the
  sim realistic rather than paper over slow firmware** — students should see and fix a too-slow
  refresh. The TP-Dificil example refreshes each digit only ~17 Hz → now flickers (segment ripple
  0.51, peaks at full brightness); a kHz-range refresh shows solid. (First tried a latch-and-hold
  that forced everything solid — reverted in favour of realism.) **The decision and its one known
  limitation — the flicker threshold is tied to the monitor's refresh rate (~8–17 ms-cycle mux can
  look solid at 60 Hz but flicker at 120 Hz), with the fix-if-needed — are documented in
  `docs/architecture.md` §4.1.** *Uncommitted.*
- **2026-06-18 (timing fix)** — Fixed the sim running **too fast on high-refresh displays**. The
  frame loop assumed 60 fps (`cycleBudget += clockHz/4/60`), so on a 120 Hz screen it ran 2× real
  speed (measured ratio 2.00). Now the budget is driven by **real elapsed time** (`clockHz/4 × dt`,
  the same `dt` the stopwatch uses, capped at 100 ms), so a "4 MHz" program runs at true 4 MHz
  wall-clock on any display (re-measured ratio 1.00; simulated time now equals the stopwatch).
  `runtime/index.html` only. *Uncommitted.*
- **2026-06-18 (runtime QoL)** — Four small conveniences (`runtime/index.html` only): a **clock
  slider** (1-2-5 steps, 1 Hz–8 MHz) synced two-way with the Reloj text input (quick way to slow
  things down to watch a multiplex cycle); an always-visible **real wall-clock stopwatch**
  (mm:ss:ms) in the Simulación card — it counts real seconds the sim has run, **independent of the
  PIC clock** (the slider doesn't change it; cycle count stays in the debugger), pauses with the
  run, and resets on load / Reiniciar; **drag-and-drop a `.hex`** onto the board to load it; and **space =
  Ejecutar/Pausar**. Verified slider, readout, drag highlight, and spacebar — the apparent "freeze"
  during headless testing was just the backgrounded tab pausing `requestAnimationFrame` (`frame()`
  steps cleanly, no error). *Uncommitted.* The **hardware stack view** (needs a core export + wasm
  rebuild) is still pending.
- **2026-06-18 (lab setup)** — Boards are now teacher-managed, not student-uploaded. Removed the
  **`.json` diagram upload** from the runtime. The dropdown now **groups by a per-board `group`
  label** the teacher chooses (defaults to "Trabajos Prácticos"; built-ins → "Ejemplos"; teacher
  boards listed first). The **authoring tool** (`runtime/authoring.html`) gained a **board
  library** (add / edit / remove, persisted in `localStorage`). It **auto-imports the existing
  `labs.js`** on open (seeds the library; plus an **Import labs.js** button), and **"💾 Save
  labs.js"** overwrites `runtime/labs.js` directly via a **localhost-only `POST /__save_labs`**
  endpoint added to `serve.js` (`window.NP_LABS=[{name,group,components}]`), falling back to a
  download if serve.js isn't reachable. **Restart `serve.js` once** to enable the write endpoint.
  Verified dropdown grouping, round-trip export, and auto-import of the 2 existing boards. *Uncommitted.*
- **2026-06-18 (seg pin map)** — 7-seg displays now show the explicit **segment→pin map**
  instead of the `seg RB1–RB7` range: a shared key `a RB1 · b RB2 · … · g RB7` once below the
  display row (when displays share segment pins — the usual multiplexed case), plus each
  display's own **`com RAx`** badge; a per-display 2-column legend is the fallback for displays
  with different pins. `drawSegKey` / `drawSegLegend` in `runtime/index.html`. Verified on
  TP-Dificil (4), the mux (2), and the single-seg counter. *Uncommitted.*
- **2026-06-18 (pin states)** — Added **live pin-state squares** on the chip pins, Proteus-style:
  red = 1, blue = 0, grey = no defined value. Read PORTA/PORTB + TRISA/TRISB each frame in
  `drawChip` (live while running, holds when paused). Rule: **outputs** always show their
  driven level; an **input** shows a level only if a button is wired to it (`buttonPins`, from
  the diagram) — so it idles per the button's polarity (active-low → red, active-high → blue)
  and flips on press; **unused inputs and VSS/VDD stay grey**. The pin name shifts just
  outboard of the square. Verified idle + press on the mux board. *Uncommitted.*
- **2026-06-18 (board view)** — Reworked how the board shows connections (`runtime/index.html`,
  canvas drawing only — no core change). Explored a labeled segment bus and tidy direct
  right-angle wiring, but both still read busy on the 4-display TP, so **settled on
  labels-only**: *no wires at all* — every component carries a **pin badge** (LED / button →
  its pin; 7-seg → a "seg RB1–RB7 · com RAx" badge), and students read each connection by name
  against the chip's pin labels. This also frees the layout (displays moved up + enlarged,
  parts placed for clarity). `build()` now emits badge specs only; `drawWiring()` renders the
  badges on top; `sample`/`integrate` timing untouched. Verified in-browser on TP-Dificil
  (4 displays + 2 buttons), the 2-digit mux (digits still persist + Status LED), and the
  8-LED demo. *Uncommitted.*
- **2026-06-18 (UI revamp)** — Reworked the runtime UI (`runtime/index.html`, CSS + layout
  only — **no core/wasm change**): **dark + light themes** via CSS variables with a **toggle**
  in the app bar (persisted in `localStorage` `np_theme`, no-flash inline head script; **new
  users default to light**, a saved toggle is respected); a **restructured layout** — top app bar (brand + theme
  switch) · left side-rail with Placa / Simulación / Archivos cards + status · centered board ·
  docked Depurador panel; and a friendlier "classroom" style (rounded cards, softer surfaces,
  indigo accent, larger controls). All element IDs / JS hooks preserved. Verified in-browser in
  **both themes**: board + 2-digit multiplex render, controls, theme persistence across reload,
  and the full debugger (Programa / Datos / SFR / Vigilar, PC highlight, breakpoints, editing)
  all intact; responsive `@media (max-width:920px)` rule confirmed in the CSSOM. Pure runtime
  change → **no wasm rebuild / embed / verify-core needed**. *Uncommitted.*
- **2026-06-18 (v2)** — Debugger v2: **breakpoints** (click a program row; the run stops
  there, resume steps past it — core `np_set_break`/`np_clear_break`/`np_break_hit`, the
  scheduler stops at a marked PC), **Paso ×10/×100**, and **live memory editing** (click a
  data cell or SFR value → `np_write_data`/`np_set_w`). Verified in-browser. *Uncommitted.*
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
- **Browser runtime**: DIP-18 board with live pin-state squares + per-component pin
  labels, LEDs / 7-seg / buttons, clock control, a teacher-managed "pick a board"
  dropdown grouped by lab (`labs.js`), four demos, and a **Spanish (es-AR) UI**.
  Verified in-browser.
- **Debugger (Depurador)**: collapsible read-only inspector for everyone — single-step
  (`np_step`), live Ciclos/PC/W/STATUS header, program memory with disassembly + PC
  highlight, data-memory grid (bank 0/1), named SFRs with bit breakdowns, the 8-level
  hardware call stack (*Pila*), breakpoints + live memory editing, and a watch/filter (by
  address or register name, persisted in `localStorage`). Cycle-exact.
- **EEPROM** (EECON1/EECON2 unlock; persists across reset/power-cycle). Demo
  climbs 1→2→3 across the Reset (power-cycle) button. (step 5)
- **Authoring tool** (`runtime/authoring.html`, instructor-only): visual editor —
  add LED/button/7-seg, assign pins (per-segment for 7-seg), button polarity +
  labels, live preview, export/import JSON. Round-trips to the student runtime.
- **Built-in ASM editor** (`runtime/asm.js` + the **Código** panel): pure-JS PIC16F628A
  assembler — write MPASM, compile to a runnable `.hex` in the browser/phone, no MPLAB. 35
  instructions, labels, `EQU`/`ORG`/`__CONFIG`/`BANKSEL`, all radixes, built-in p16f628a symbols;
  emits Intel HEX into the existing `loadHex()` (no core change). CBLOCK/`#define`/MACRO → clean
  Spanish "use MPLAB" rejection. Validated byte-for-byte vs MPLAB (`node test-asm.js`).
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

- ASM editor **Phase 2** (optional): `CBLOCK`/`ENDC`, `#define` text macros, and `MACRO`/`ENDM` so the
  macro/cblock labs (e.g. the multiplexed `TP 2022`) assemble in-browser too — today they're rejected
  with a "usá MPLAB" note. The macro processor is the long pole; everything else is additive and still
  needs no core change. `test-asm.js` already has those two files as fixtures.

- Embed the live URL in the Moodle course (iframe snippet in `DEPLOY.md`).
- More instructor lab boards: build them in `authoring.html` (it auto-loads the current
  `labs.js`), **Save to lab list**, then **💾 Save labs.js** to overwrite `runtime/labs.js`
  directly (needs `serve.js` running) → commit + push and they appear in the student
  dropdown under the **group** you set on each board.
- Authoring niceties: drag-to-position, a "Test against a .hex" preview, honor
  `x`/`y` in the runtime (currently auto-placed around the chip).
- (simplified) internal pull-ups (RBPU) — not modeled; add if a lab needs it.

## Reconciliation items (vs MPLAB)

- Interrupt entry-cycle count (`INTERRUPT_ENTRY_CYCLES = 2`) and exact TMR0
  reload-ISR period — confirm against MPLAB's stopwatch on a timer lab.
- External TMR0 clock (`T0CS = 1`, counting T0CKI edges) is not modeled.
