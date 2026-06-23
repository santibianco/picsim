// Oracle test for the in-browser assembler (runtime/asm.js).
//
// Assembles each examples/*.asm and compares the result, word for word, against
// the matching MPLAB-produced .hex. The pass bar is ZERO contradicting words:
// wherever both our output and MPLAB's define a program word, they must agree.
// (MPLAB may emit extra fill words and a config word from a slightly different
// source revision; those are reported but not failed, and the config word does
// not affect simulation.) The CBLOCK/#define/MACRO example (TP 2022) ships with a
// drifted .hex that doesn't correspond to its .asm, so it is only checked to
// assemble — it's diffed byte-for-byte against the real MPASMWIN.exe separately.
//
// Run:  node test-asm.js [path/to/examples]
"use strict";
const fs = require("fs");
const path = require("path");
const { assemble } = require("./runtime/asm.js");

const EXBASE = process.argv[2] || path.join(__dirname, "examples");

const CASES = [
  { dir: "Simple one",     asm: "Punto A.asm",             hex: "Punto A.hex",            expect: "ok" },
  { dir: "Multiplication", asm: "Multiplicacion.asm",      hex: "mult.hex",               expect: "ok" },
  { dir: "Larger one",     asm: "TP-turnos-solucion.asm",  hex: "Solucion.hex",           expect: "ok" },
  { dir: "Another large",  asm: "TP 2022 - Ejemplo.asm",   hex: null,                     expect: "assembles" },
];

// Decode Intel HEX into { words: Map(wordAddr->14bit), config }.
function decodeHex(text) {
  const byteMap = new Map();
  let base = 0;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line[0] !== ":") continue;
    const b = [];
    for (let i = 1; i + 1 < line.length; i += 2) b.push(parseInt(line.substr(i, 2), 16));
    const count = b[0], addr = (b[1] << 8) | b[2], type = b[3];
    if (type === 0x00) for (let i = 0; i < count; i++) byteMap.set(base + addr + i, b[4 + i]);
    else if (type === 0x01) break;
    else if (type === 0x02) base = ((b[4] << 8) | b[5]) << 4;
    else if (type === 0x04) base = ((b[4] << 8) | b[5]) << 16;
  }
  const words = new Map();
  for (const a of byteMap.keys()) {
    if (a % 2 !== 0 || a >= 0x4000) continue;            // program region only
    const lo = byteMap.get(a), hi = byteMap.has(a + 1) ? byteMap.get(a + 1) : 0xFF;
    words.set(a / 2, (lo | (hi << 8)) & 0x3FFF);
  }
  let config = null;
  if (byteMap.has(0x400E) || byteMap.has(0x400F))
    config = ((byteMap.get(0x400E) ?? 0xFF) | ((byteMap.get(0x400F) ?? 0xFF) << 8)) & 0x3FFF;
  return { words, config };
}

const hx = (v, n = 4) => "0x" + v.toString(16).toUpperCase().padStart(n, "0");
let failed = 0;

for (const c of CASES) {
  const asmPath = path.join(EXBASE, c.dir, c.asm);
  const src = fs.readFileSync(asmPath, "utf8");
  const res = assemble(src);

  if (c.expect === "assembles") {
    const ok = res.ok && res.words.size > 0;
    console.log(`${ok ? "PASS" : "FAIL"}  ${c.asm} — ensambla (${res.ok ? res.words.size + " palabras, config " + hx(res.config || 0) : res.error})`);
    if (!ok) failed++;
    continue;
  }

  if (!res.ok) { console.log(`FAIL  ${c.asm} — error de ensamblado: ${res.error}`); failed++; continue; }

  const exp = decodeHex(fs.readFileSync(path.join(EXBASE, c.dir, c.hex), "utf8"));
  const got = decodeHex(res.hex);

  const contradictions = [];
  for (const [a, v] of got.words) if (exp.words.has(a) && exp.words.get(a) !== v)
    contradictions.push([a, v, exp.words.get(a)]);

  let overlap = 0, gotOnly = 0, expOnly = 0;
  for (const a of got.words.keys()) (exp.words.has(a) ? overlap++ : gotOnly++);
  for (const a of exp.words.keys()) if (!got.words.has(a)) expOnly++;

  const pass = contradictions.length === 0 && overlap > 0;
  if (!pass) failed++;
  console.log(`${pass ? "PASS" : "FAIL"}  ${c.asm} — ${overlap} palabras coinciden con MPLAB, ${contradictions.length} contradicciones`);
  console.log(`        instrucciones nuestras=${got.words.size} · solo-nuestras=${gotOnly} · solo-MPLAB=${expOnly} (relleno/rev.)` +
              ` · config nuestra=${got.config === null ? "—" : hx(got.config)} MPLAB=${exp.config === null ? "—" : hx(exp.config)}`);
  for (const [a, v, e] of contradictions.slice(0, 12))
    console.log(`        x palabra ${hx(a, 3)}: nuestra ${hx(v)} vs MPLAB ${hx(e)}`);
}

console.log(failed ? `\n${failed} caso(s) fallaron.` : "\nTodos los casos pasaron ✓");
process.exit(failed ? 1 : 0);
