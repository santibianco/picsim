// SimuPIC — verify the EMBEDDED core before trusting it. Run from the project root:
//
//     node verify-core.js
//
// It decodes the base64 that actually ships in the page (runtime/core-wasm.js),
// validates it, and runs the EEPROM fixture: PORTB must climb 1 → 2 → 3 across
// power-cycles. This is the check that catches a STALE or WRONG embed (e.g. the
// `release/` vs `release/deps/` mix-up) — the one failure mode that bit us once.
// Exits non-zero on any problem, so it's safe to chain after the embed step.
const fs = require("fs");

const js = fs.readFileSync("runtime/core-wasm.js", "utf8");
const m = js.match(/NP_WASM_BASE64\s*=\s*"([A-Za-z0-9+/=]+)"/);
if (!m) {
  console.error("✗ no NP_WASM_BASE64 in runtime/core-wasm.js — run the embed step first");
  process.exit(1);
}
const bytes = new Uint8Array(Buffer.from(m[1], "base64"));
console.log("embedded wasm:", bytes.length, "bytes");
if (!WebAssembly.validate(bytes)) {
  console.error("✗ embedded wasm failed to validate (truncated / corrupt / wrong file?)");
  process.exit(1);
}

WebAssembly.instantiate(bytes, {}).then(({ instance }) => {
  const X = instance.exports;

  // (A) EEPROM persistence fixture: PORTB must climb 1 → 2 → 3 across power-cycles.
  const hex = fs.readFileSync("core/tests/fixtures/eeprom_counter.hex", "utf8");
  const d = new TextEncoder().encode(hex);
  new Uint8Array(X.memory.buffer, X.np_hex_buffer(), d.length).set(d);
  X.np_load_hex(d.length);
  const r = [];
  X.np_run_cycles(4000); r.push(X.np_read_portb());           // power-on  → "1"
  X.np_reset(); X.np_run_cycles(4000); r.push(X.np_read_portb()); // cycle 1 → "2"
  X.np_reset(); X.np_run_cycles(4000); r.push(X.np_read_portb()); // cycle 2 → "3"
  const eepromOk = r[0] === 0x06 && r[1] === 0x5b && r[2] === 0x4f; // 7-seg digits 1,2,3
  console.log(
    "PORTB across power-cycles:", r.map((v) => "0x" + v.toString(16)).join(", "),
    eepromOk ? "✓ EEPROM persists 1→2→3" : "✗ unexpected — stale or broken embed"
  );

  // (B) debugger inspection surface (added 2026-06-18). Catches an OLD embed that
  //     predates these exports — which would silently disable the runtime debugger.
  const needed = ["np_step", "np_pc", "np_w", "np_cycles", "np_read_data",
                  "np_prog_word", "np_disasm", "np_disasm_buffer"];
  const missing = needed.filter((n) => typeof X[n] !== "function");
  let dbgOk = missing.length === 0;
  if (!dbgOk) {
    console.log("debug exports: ✗ missing", missing.join(", "),
      "— rebuild the wasm, an OLD core is embedded");
  } else {
    // Canonical blink fixture: word 0 is BSF STATUS,RP0 (0x1683). Step it once.
    const prog = ":0A000000831686018312860A032886\n:00000001FF\n";
    const pd = new TextEncoder().encode(prog);
    new Uint8Array(X.memory.buffer, X.np_hex_buffer(), pd.length).set(pd);
    X.np_load_hex(pd.length);
    const pc0 = X.np_pc(), cy0 = X.np_cycles(), w0 = X.np_prog_word(0);
    const ran = X.np_step();                       // execute BSF STATUS,RP0
    const pc1 = X.np_pc(), cy1 = X.np_cycles();
    const dn = X.np_disasm(w0);
    const txt = new TextDecoder().decode(new Uint8Array(X.memory.buffer, X.np_disasm_buffer(), dn));
    const st = X.np_read_data(0x03), w = X.np_w(); // STATUS now 0x38 (POR 0x18 | RP0), W untouched
    dbgOk = pc0 === 0 && pc1 === 1 && ran === 1 && cy1 === cy0 + 1 &&
            w0 === 0x1683 && txt === "BSF 0x03, 5" && st === 0x38 && w === 0;
    console.log(
      `debug surface: pc ${pc0}→${pc1}, cycles +${cy1 - cy0}, disasm(0x${w0.toString(16)})="${txt}", STATUS=0x${st.toString(16)}`,
      dbgOk ? "✓ step/inspect OK" : "✗ unexpected — stale or broken embed"
    );
  }

  // Let Node exit naturally — process.exit() here races wasm/libuv teardown on
  // Windows and trips a (harmless but noisy) assertion. exitCode does the same job.
  process.exitCode = eepromOk && dbgOk ? 0 : 1;
}).catch((e) => {
  console.error("✗ instantiate failed:", e.message);
  process.exitCode = 1;
});
