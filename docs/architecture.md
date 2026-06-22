# Architecture — Timing Model & Core Interface

This is the load-bearing spec. The CPU core, the cycle-accounting loop, and the
frame-sampling model described here are the ~20% of the project that everything else
depends on. Get this right and the components and UI are mostly mechanical. Get it
wrong and the whole thing feels flaky in exactly the way SimulIDE does.

> **Status:** fully implemented and shipped — see `STATUS.md` and `README.md`. The
> C-ABI exports are prefixed `np_` (e.g. `np_load_hex`, `np_run_cycles`,
> `np_read_portb`, `np_set_pin`); the conceptual names below describe the contract,
> not the literal symbols. A read-only **debugger surface** was layered on top
> (single-step + memory / register / call-stack inspection): `np_step`, `np_pc`, `np_w`,
> `np_cycles`, `np_read_data`, `np_prog_word`, `np_disasm`, `np_stack_depth`/`np_stack_at`
> — see `docs/inspector-plan.md`.

---

## 1. Time model: cycles are the master clock

The simulation's notion of time is **instruction cycles**, counted by the core. Nothing
else is authoritative. In particular, **the render loop must never drive the
simulation** — it only samples whatever the core has already computed.

### 16F628A timing facts
- Every instruction takes **1 instruction cycle**, **except** branches and skips-when-
  taken, which take **2**.
- **1 instruction cycle = 4 oscillator clocks.**
- At the default **4 MHz** internal oscillator: **1 instruction cycle = 1 µs.**
- No pipeline stalls, no cache, no variable memory latency. Cycle count is exact by
  construction.

### Derived constants (do NOT hardcode the 1µs)
Store the clock as a variable so the "any clock value" nice-to-have comes for free:

```
clock_hz            = 4_000_000          // default; make this settable
cycles_per_second   = clock_hz / 4       // instruction cycles per second
cycles_per_frame    = clock_hz / 4 / 60  // ≈ 16,667 at 4 MHz, 60 fps
```

The CPU core itself is **clock-agnostic** — it just counts cycles. Clock speed only
matters when converting simulated cycles to wall-clock time for (a) the render cadence
and (b) timer ticks. So supporting a variable clock is just changing `clock_hz`; never
bake `1 cycle = 1 µs` into the core.

---

## 2. The run loop

Each animation frame (~60 fps), JS asks the core to advance one frame's worth of
*simulated* time, then samples and draws:

```
function frame() {
  core.runCycles(cyclesPerFrame);   // run ~16,667 cycles of firmware
  const pins = core.readPins();     // sample resulting pin state
  render(pins, core.segmentOnTime); // draw (see §4 for 7-seg)
  requestAnimationFrame(frame);
}
```

Within that single `runCycles` call the firmware may execute thousands of instructions
and switch a multiplexed display dozens of times. That is the whole point: the core
resolves all the fast timing internally, and JS only looks once per frame.

### Inside `runCycles(n)` — correct ordering per step
For each instruction executed, in this order:

1. **Fetch** instruction at PC from program memory.
2. **Decode** to one of the ~35 PIC14 operations.
3. **Execute**: mutate W / file registers / STATUS flags; update PC (branches add a cycle).
4. **Advance `cycle_count`** by the instruction's cycle cost (1 or 2).
5. **Tick TMR0** according to how many cycles elapsed, respecting the prescaler
   (see §3). This may set the TMR0 overflow flag (T0IF).
6. **Check interrupts**: if GIE set and an enabled+flagged source is pending
   (T0IF, INTF for RB0/INT, RBIF for PORTB-change), push the return address and
   vector to the ISR.
7. Continue until the requested cycle budget `n` is consumed.

Keep accumulating any cycle remainder across frames so timing doesn't slowly drift
(don't truncate `cycles_per_frame` to an int and lose the fraction every frame).

---

## 3. Peripherals required even in digital-only mode

Digital-only removes analog, but these peripherals are unavoidable because LED/button/
display firmware uses them constantly:

- **PORTA / PORTB** with **TRISA / TRISB** direction registers — the I/O surface.
  A pin driven as output reflects the latch value; a pin set as input reads the
  externally-driven value (from a button component).
- **TMR0 + prescaler** — almost all timing/delay/multiplex code uses it. Implement the
  prescaler assignment (shared with WDT via OPTION_REG) and the T0IF overflow flag.
- **RB0/INT external interrupt (INTF)** — common with buttons.
- **PORTB interrupt-on-change (RBIF)** — common with buttons.

### Implemented since this spec
**EEPROM** (EECON1/EECON2 unlock, non-volatile across reset/power-cycle) is now fully
implemented — labs that persist state across power cycles work. The comparators, USART,
and CCP/PWM module remain stubbed (reads return sane defaults, writes accepted/ignored);
implement later only if a lab needs them.

### Memory / CPU correctness points that need care (not difficulty, just precision)
- **Bank switching** via RP0/RP1 in STATUS (the 16F628A uses banked SFR/RAM access).
- **Indirect addressing** through FSR/INDF.
- **W and STATUS flag interactions** — Z, C, DC must be set correctly per instruction.
- **PCL / PCLATH paging** on computed jumps (`ADDWF PCL`, etc.).

These are the spots where naive emulators go subtly wrong. Cover them with tests (§5).

---

## 4. 7-segment rendering: persistence of vision

A multiplexed display switches digits **many times within one rendered frame**. If you
naively snapshot pin state once at frame end, you'll catch a single digit lit and the
rest dark — showing flicker a human eye would never see on real hardware. That's a bug
that makes correct student code look broken.

**Solution — accumulate per-segment on-time across the frame.** As the core runs the
frame's cycles, track how long each digit/segment was actually driven. At frame end,
render brightness proportional to on-time:

- A digit lit for 1/4 of the frame renders at ~25% brightness.
- This *correctly* reproduces persistence of vision.
- It also surfaces real bugs: a display that's too dim because the student's refresh
  duty cycle is wrong will actually look dim, which is pedagogically valuable.

Implementation: the core (or a thin layer over it) maintains an on-time accumulator per
segment, reset at the start of each frame and integrated as pin states change during
`runCycles`. JS reads these accumulators when rendering.

**Simpler fallback** (if you want to defer PoV): sample pin state at sub-frame intervals
(e.g. every ~1 ms of simulated time) and OR the results. Less accurate — no brightness
gradation — but easy. Prefer the on-time model; it's not much more work and it's honest.

### 4.1 What the runtime actually does, and the flicker-threshold decision (2026-06)

The runtime's brightness model evolved past the pure on-time/duty model above. A 7-seg
segment's drawn brightness now **decays toward 0 with a ~45 ms time constant (the eye's
persistence) and is pulled to full whenever the segment is actually lit** (its digit
selected *and* the segment driven). Net effect:

- A mux refreshed faster than flicker fusion fuses to a **solid, bright** display (full
  brightness, not 25% — we assume the current-boost most real multiplexed boards use).
- A mux refreshed slower than the eye **visibly flickers**, exactly as it would on real
  hardware. We deliberately do **not** hide a slow refresh the way Proteus does.

**Decision (Santiago, 2026-06): keep the simulator realistic rather than paper over slow
firmware.** Students should see a too-slow refresh flicker and fix it (shorten the TMR0
period), not ship a flickery board that only looked fine in the sim — this is where SimuPIC
is *more* useful than Proteus, not less. (An earlier "latch-and-hold" model forced every mux
solid regardless of refresh rate; it was reverted for this reason.)

**Known limitation — the flicker threshold is tied to the monitor's refresh rate.** Brightness
is integrated once per rendered frame (`requestAnimationFrame`), so the sim effectively samples
the display at the monitor's refresh. A digit looks solid only if the *whole* mux cycle (all
digits once) fits inside one screen frame:

- 60 Hz monitor → 16.7 ms frame → solid if the full cycle is under ~16.7 ms (4 digits ≤ ~4 ms dwell).
- 120 Hz monitor → 8.3 ms frame → solid if under ~8.3 ms.

So mux cycles in the **~8–17 ms band (~60–120 Hz per digit)** can look solid on a 60 Hz screen
yet flicker faintly on a 120 Hz one — two students may see slightly different results for the
same `.hex` in that narrow band. Outside it the result is consistent across machines: anything
clearly slow (e.g. the *TP 2022* example at ~17 Hz / 60 ms cycle) flickers on every screen, and
anything in the proper kHz range is solid on every screen. The band roughly coincides with where
real people start disagreeing about flicker, so it's a defensible place for the threshold to sit.

**If this ever causes confusion or support questions:** move the brightness integration down into
the per-sub-step sample loop (the frame already samples PORTA/PORTB ~64×/frame). Integrate
brightness against each sub-step's real-time slice instead of once per frame, and the flicker-
fusion threshold becomes a fixed real-time constant (the ~45 ms `tau`), independent of monitor
refresh — at the cost of a little more per-frame work. Where: `runtime/index.html`, the 7-seg /
LED `integrate()` methods plus the frame sample loop.

---

## 5. The core's public interface (WASM exports)

Keep the surface tiny. Everything hangs off these:

| Function | Purpose |
|---|---|
| `loadHex(text)` | Parse Intel HEX (from MPLAB) into program memory; reset the CPU. |
| `runCycles(n)` | Execute until `n` instruction cycles are consumed (see §2). |
| `readPins()` | Return current logical state of all I/O pins. |
| `setPin(pin, level)` | Externally drive an input pin — used for button presses. |
| `reset()` | Power-on reset: clear registers, reset PC and SFRs to POR values. |
| `setClockHz(hz)` | Optional: change clock; only affects cycle→wall-clock conversion. |

For 7-seg PoV, also expose the per-segment on-time accumulators (e.g.
`segmentOnTime()` returning the integrated on-times since the last frame, and a
`resetFrameAccumulators()` called at frame start) — or fold that into `runCycles`
returning the accumulated data. Exact shape is an implementation choice; the
*requirement* is that JS can render brightness from on-time.

### Input format
Firmware enters as **Intel HEX**, which MPLAB already produces. Parse HEX records into
the 2K program memory. No assembler is built — the IDE is the toolchain.

---

## 6. Diagram format (shared by runtime and authoring tool)

A diagram is plain JSON. Components bind directly to pin names — no nets, no wires.
Both tools read this format: the runtime renders and runs it; the authoring tool writes
it. See `diagrams/lab-counter.example.json` for a concrete instance.

Minimum fields per component type:
- **led** — `id`, `pin`, position
- **button** — `id`, `pin`, position, `activeLow` (whether pressed = 0)
- **sevenseg** — `id`, `pins` (array of 7, segment order a–g, plus optionally a common/
  digit-select pin for multiplexed multi-digit setups), position

For multiplexed multi-digit displays, represent each digit's common/select line as a
pin the firmware drives, so the PoV accumulation (§4) reflects which digit is active
when.

---

## 7. Build order (de-risked)

1. **CPU core + memory + HEX loader**, verified against known firmware. Get `BSF`/`BCF`
   and the flag/banking edge cases right *first* — they're the SimulIDE failure and the
   trust foundation.
2. **TMR0 + prescaler + interrupts.** Now delay loops and timer-based debounce match
   hardware.
3. **Cycle-driven scheduler + frame sampling.** The spine; lock it down early.
4. **Components**: LEDs, buttons, then 7-seg with PoV accumulation.
5. **Student runtime**: load hex + JSON diagram, run/stop, button clicks.
6. **Authoring**: hand-write JSON for the first labs; build a drag-and-drop editor later
   if it's worth it.

Throughout: maintain the **MPLAB-comparison test suite** (§ trust anchor in README) so
every core change is checked cycle-by-cycle against ground truth.

**All six steps are complete** — plus EEPROM, a Spanish installable-PWA runtime with a
lab selector, the authoring tool, and GitHub Pages deployment. 82 tests passing.
