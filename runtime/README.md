# SimuPIC — browser runtime

A single self-contained page that runs the WASM core and renders a board: the
PIC16F628A drawn as a DIP-18 chip with labeled pins, and components (LEDs, 7-segment
displays, buttons) wired to it. Spanish (es-AR) UI; installable PWA that works offline.

## Run it

From the project root:

```sh
node serve.js     # → http://localhost:8080   (instructor editor → :8080/authoring.html)
```

Then pick a board from the **Placa** dropdown (or a demo) and **Cargar programa (.hex)**.
The embedded core + a service worker mean it works offline after the first load, and the
browser offers to **Install** it.

> Plain `file://` works for the basic simulator, but the PWA (install / offline) and the
> service worker need `http://localhost` (serve.js) or the deployed HTTPS site.

## Rebuilding the core (only when the Rust **core** changes)

1. Build the core to WASM — from `core/`:

   ```sh
   cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib
   ```

2. Embed it — from the project root. **Gotcha:** `cargo rustc --crate-type cdylib` writes
   the `.wasm` to `release/DEPS/`, not the top-level `release/` dir:

   ```sh
   node embed-core.js     # then: node verify-core.js  (must print both ✓)
   ```

   This regenerates `runtime/core-wasm.js` (the base64-embedded core).

## Lab boards

The **Placa** dropdown is built from `labs.js` (instructor boards) plus four built-in
demos. To add a board: build it in `authoring.html`, **Export JSON**, paste it into
`labs.js`, and push. See [`../DEPLOY.md`](../DEPLOY.md).

## Files

- `index.html` — the app (WASM loader, Canvas board, Spanish UI, lab dropdown, clock).
- `core-wasm.js` — base64-embedded WASM core (generated; see above).
- `labs.js` — instructor lab boards (the **Placa** dropdown).
- `manifest.json` + `sw.js` + `icon.svg` — PWA (installable + offline).
- `authoring.html` — instructor-only diagram editor (excluded from the student deploy).

## Rendering

The frame loop sub-divides each frame, samples PORTA/PORTB per sub-step, and accumulates
per-pin on-time; brightness is that fraction ([`../docs/architecture.md`](../docs/architecture.md) §4).
This is what makes multiplexed 7-seg displays show correct persistence-of-vision instead
of the flicker a naive once-per-frame snapshot would produce.
