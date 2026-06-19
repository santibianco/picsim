// SimuPIC — embed the freshly built WASM core into runtime/core-wasm.js.
// Run from the project root:  node embed-core.js
//
// Replaces the fragile `node -e "..."` one-liner (whose nested quotes get mangled
// by PowerShell). GOTCHA: `cargo rustc --crate-type cdylib` writes the wasm to
// release/DEPS/, so we read that (falling back to the top-level release/ dir).
const fs = require("fs");

const candidates = [
  "core/target/wasm32-unknown-unknown/release/deps/new_proteus_core.wasm",
  "core/target/wasm32-unknown-unknown/release/new_proteus_core.wasm",
];
const wasmPath = candidates.find((p) => fs.existsSync(p));
if (!wasmPath) {
  console.error(
    "✗ no built wasm found. Build it first:\n" +
      "  cd core; cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib; cd .."
  );
  process.exit(1);
}

const b = fs.readFileSync(wasmPath).toString("base64");
fs.writeFileSync("runtime/core-wasm.js", 'window.NP_WASM_BASE64="' + b + '";');
console.log(
  "embedded " + b.length + " base64 chars (" + fs.statSync(wasmPath).size +
    "-byte wasm) from " + wasmPath + " -> runtime/core-wasm.js"
);
console.log("now run:  node verify-core.js");
