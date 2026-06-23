// SimuPIC — in-browser PIC16F628A assembler (MVP).
//
// Turns MPASM-style .asm text into Intel HEX, identical in shape to what MPLAB
// emits, so the existing loadHex() path runs it unchanged. Pure JS: no build
// step, runs on any phone, never touches the WASM core.
//
// Scope (MVP): all 35 instructions, labels, EQU/SET, ORG, __CONFIG (numeric and
// symbolic `_A & _B`), BANKSEL, GOTO $, the .dec/0x/h''/b''/d''/o''/'c' radixes
// (bare digit-led numbers default to hex, like MPASM), a built-in PIC16F628A
// symbol table (#include <p16f628a.inc> becomes a no-op), and END. CBLOCK,
// #define and MACRO are deliberately out of scope and rejected with a clear
// Spanish message rather than mis-assembled.
//
// Validated byte-for-byte against MPLAB output: test-asm.js assembles the real
// examples/*.asm and diffs every program word against the matching .hex.
//
// API:  NP_ASM.assemble(text) -> { ok, hex, words, config } | { ok:false, error, line }
(function (global) {
  "use strict";

  // ---- PIC16F628A special-function registers (what p16f628a.inc defines) ----
  const SFR = {
    INDF:0x00, TMR0:0x01, PCL:0x02, STATUS:0x03, FSR:0x04, PORTA:0x05, PORTB:0x06,
    PCLATH:0x0A, INTCON:0x0B, PIR1:0x0C, TMR1L:0x0E, TMR1H:0x0F, T1CON:0x10,
    TMR2:0x11, T2CON:0x12, CCPR1L:0x15, CCPR1H:0x16, CCP1CON:0x17, RCSTA:0x18,
    TXREG:0x19, RCREG:0x1A, CMCON:0x1F,
    OPTION_REG:0x81, OPTION:0x81, TRISA:0x85, TRISB:0x86, PIE1:0x8C, PCON:0x8E,
    PR2:0x92, TXSTA:0x98, SPBRG:0x99, EEDATA:0x9A, EEADR:0x9B, EECON1:0x9C,
    EECON2:0x9D, VRCON:0x9F
  };

  // ---- bit names (global EQUs in MPASM; not scoped to a register) ----
  const BIT = {
    // port pins
    RA0:0,RA1:1,RA2:2,RA3:3,RA4:4,RA5:5,RA6:6,RA7:7,
    RB0:0,RB1:1,RB2:2,RB3:3,RB4:4,RB5:5,RB6:6,RB7:7,
    // STATUS
    C:0, DC:1, Z:2, PD:3, TO:4, RP0:5, RP1:6, IRP:7,
    // INTCON
    RBIF:0, INTF:1, T0IF:2, RBIE:3, INTE:4, T0IE:5, PEIE:6, GIE:7,
    // OPTION_REG
    PS0:0, PS1:1, PS2:2, PSA:3, T0SE:4, T0CS:5, INTEDG:6, RBPU:7, NOT_RBPU:7,
    // EECON1
    RD:0, WR:1, WREN:2, WRERR:3,
    // T1CON
    TMR1ON:0, TMR1CS:1, T1SYNC:2, T1OSCEN:3, T1CKPS0:4, T1CKPS1:5,
    // T2CON
    T2CKPS0:0, T2CKPS1:1, TMR2ON:2, TOUTPS0:3, TOUTPS1:4, TOUTPS2:5, TOUTPS3:6,
    // CMCON
    CM0:0, CM1:1, CM2:2, CIS:3, C1INV:4, C2INV:5, C1OUT:6, C2OUT:7,
    // PIR1 / PIE1
    TMR1IF:0, TMR2IF:1, CCP1IF:2, TXIF:4, RCIF:5, CMIF:6, EEIF:7,
    TMR1IE:0, TMR2IE:1, CCP1IE:2, TXIE:4, RCIE:5, CMIE:6, EEIE:7,
    // RCSTA / TXSTA (common)
    RX9D:0, OERR:1, FERR:2, ADDEN:3, CREN:4, SREN:5, RX9:6, SPEN:7,
    TX9D:0, TRMT:1, BRGH:2, SYNC:4, TXEN:5, TX9:6, CSRC:7
  };

  // ---- __CONFIG constants — transcribed verbatim from Microchip's p16f628a.inc
  // and cross-checked against it by test (test-asm.js); the resulting config word
  // matches MPASM byte-for-byte. (Earlier hand-derived values had PWRTE/WDT on the
  // wrong bits — that, not "revision drift", is why TP-turnos was 0x3F24 vs 0x3F30.)
  const CFG = {
    _BODEN_ON:0x3FFF, _BODEN_OFF:0x3FBF, _BOREN_ON:0x3FFF, _BOREN_OFF:0x3FBF,
    _CP_ON:0x1FFF, _CP_OFF:0x3FFF, _DATA_CP_ON:0x3EFF, _DATA_CP_OFF:0x3FFF,
    _PWRTE_OFF:0x3FFF, _PWRTE_ON:0x3FF7, _WDT_ON:0x3FFF, _WDT_OFF:0x3FFB,
    _LVP_ON:0x3FFF, _LVP_OFF:0x3F7F, _MCLRE_ON:0x3FFF, _MCLRE_OFF:0x3FDF,
    _RC_OSC_CLKOUT:0x3FFF, _RC_OSC_NOCLKOUT:0x3FFE,
    _ER_OSC_CLKOUT:0x3FFF, _ER_OSC_NOCLKOUT:0x3FFE,
    _INTOSC_OSC_CLKOUT:0x3FFD, _INTOSC_OSC_NOCLKOUT:0x3FFC,
    _INTRC_OSC_CLKOUT:0x3FFD, _INTRC_OSC_NOCLKOUT:0x3FFC,
    _EXTCLK_OSC:0x3FEF, _HS_OSC:0x3FEE, _XT_OSC:0x3FED, _LP_OSC:0x3FEC
  };

  // ---- 35-instruction encoding table ----
  // t describes operands: 'fd' file+dest, 'f' file only, 'fb' file+bit,
  // 'k' literal, 'a' 11-bit address, '' inherent.
  const OPS = {
    ADDWF:{b:0x0700,t:"fd"}, ANDWF:{b:0x0500,t:"fd"}, COMF:{b:0x0900,t:"fd"},
    DECF:{b:0x0300,t:"fd"}, DECFSZ:{b:0x0B00,t:"fd"}, INCF:{b:0x0A00,t:"fd"},
    INCFSZ:{b:0x0F00,t:"fd"}, IORWF:{b:0x0400,t:"fd"}, MOVF:{b:0x0800,t:"fd"},
    RLF:{b:0x0D00,t:"fd"}, RRF:{b:0x0C00,t:"fd"}, SUBWF:{b:0x0200,t:"fd"},
    SWAPF:{b:0x0E00,t:"fd"}, XORWF:{b:0x0600,t:"fd"},
    CLRF:{b:0x0180,t:"f"}, MOVWF:{b:0x0080,t:"f"},
    CLRW:{b:0x0100,t:""}, NOP:{b:0x0000,t:""}, CLRWDT:{b:0x0064,t:""},
    RETFIE:{b:0x0009,t:""}, RETURN:{b:0x0008,t:""}, SLEEP:{b:0x0063,t:""},
    BCF:{b:0x1000,t:"fb"}, BSF:{b:0x1400,t:"fb"}, BTFSC:{b:0x1800,t:"fb"}, BTFSS:{b:0x1C00,t:"fb"},
    ADDLW:{b:0x3E00,t:"k"}, ANDLW:{b:0x3900,t:"k"}, IORLW:{b:0x3800,t:"k"},
    MOVLW:{b:0x3000,t:"k"}, SUBLW:{b:0x3C00,t:"k"}, XORLW:{b:0x3A00,t:"k"}, RETLW:{b:0x3400,t:"k"},
    CALL:{b:0x2000,t:"a"}, GOTO:{b:0x2800,t:"a"}
  };

  // Directives we recognise but don't yet support — reject with a clear note.
  const UNSUPPORTED = {
    DT:"DT", DA:"DA", DW:"DW", DE:"DE", FILL:"FILL", RES:"RES"
  };

  function AsmError(line, msg) { this.line = line; this.msg = msg; }

  // Strip a line comment (everything from the first ';' outside a '...' literal).
  function stripComment(s) {
    let q = false;
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (c === "'") q = !q;
      else if (c === ";" && !q) return s.slice(0, i);
    }
    return s;
  }

  // Split operands on top-level commas (ignore commas inside '...' or (...)).
  function splitOps(s) {
    const out = []; let depth = 0, q = false, cur = "";
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (c === "'") { q = !q; cur += c; }
      else if (q) cur += c;
      else if (c === "(") { depth++; cur += c; }
      else if (c === ")") { depth--; cur += c; }
      else if (c === "," && depth === 0) { out.push(cur.trim()); cur = ""; }
      else cur += c;
    }
    if (cur.trim() !== "" || out.length) out.push(cur.trim());
    return out;
  }

  // Parse a single numeric literal, or return null if `tok` isn't one.
  function parseNum(tok) {
    let m;
    if ((m = /^0x([0-9a-f]+)$/i.exec(tok))) return parseInt(m[1], 16);
    if ((m = /^([0-9a-f]+)h$/i.exec(tok))) return parseInt(m[1], 16);
    if ((m = /^[hH]'([0-9a-f]+)'$/.exec(tok))) return parseInt(m[1], 16);
    if ((m = /^\.(-?[0-9]+)$/.exec(tok))) return parseInt(m[1], 10);
    if ((m = /^[dD]'(-?[0-9]+)'$/.exec(tok))) return parseInt(m[1], 10);
    if ((m = /^[bB]'([01]+)'$/.exec(tok))) return parseInt(m[1], 2);
    if ((m = /^[oOqQ]'([0-7]+)'$/.exec(tok))) return parseInt(m[1], 8);
    if ((m = /^[aA]?'(.)'$/.exec(tok))) return tok.slice(-2, -1).charCodeAt(0);
    // bare token starting with a digit => default radix (hex), like MPASM.
    if (/^[0-9][0-9a-f]*$/i.test(tok)) return parseInt(tok, 16);
    return null;
  }

  // Expression evaluator (shunting-yard). Supports & | ^ + - * / << >> ( ) and
  // unary - ~. Identifiers resolve via `resolve`. `$` is the current address.
  const PREC = { "|":2, "^":3, "&":4, "<<":5, ">>":5, "+":6, "-":6, "*":7, "/":7, "%":7 };
  function evalExpr(expr, resolve, line) {
    const toks = [];
    // Order matters: radix/number forms must precede the bare-identifier rule so
    // b'0101', h'3F' and 'c' are not mis-read as the identifiers b, h, a.
    const re = /\s*(<<|>>|[&|^+\-*/%()~]|\$|0x[0-9a-f]+|[hHdDbBoOqQ]'[^']*'|[aA]?'.'|[0-9a-f]+h|\.[0-9]+|[0-9][0-9a-f]*|[A-Za-z_?][\w?]*)/gi;
    let m, last = 0;
    while ((m = re.exec(expr)) !== null) {
      if (m.index > last && expr.slice(last, m.index).trim() !== "")
        throw new AsmError(line, "expresion invalida cerca de \"" + expr.slice(last).trim() + "\"");
      toks.push(m[1]); last = re.lastIndex;
    }
    if (expr.slice(last).trim() !== "")
      throw new AsmError(line, "expresion invalida cerca de \"" + expr.slice(last).trim() + "\"");
    if (!toks.length) throw new AsmError(line, "se esperaba un valor");

    const out = [], ops = [];
    const apply = () => {
      const op = ops.pop();
      if (op === "u-") { out.push(-out.pop()); return; }
      if (op === "u~") { out.push(~out.pop()); return; }
      const b = out.pop(), a = out.pop();
      switch (op) {
        case "&": out.push(a & b); break; case "|": out.push(a | b); break;
        case "^": out.push(a ^ b); break; case "+": out.push(a + b); break;
        case "-": out.push(a - b); break; case "*": out.push(a * b); break;
        case "/": out.push(Math.trunc(a / b)); break; case "%": out.push(a % b); break;
        case "<<": out.push(a << b); break; case ">>": out.push(a >> b); break;
        default: throw new AsmError(line, "operador no soportado: " + op);
      }
    };
    let expectVal = true;
    for (const t of toks) {
      if (t === "(") { ops.push(t); expectVal = true; }
      else if (t === ")") {
        while (ops.length && ops[ops.length - 1] !== "(") apply();
        if (!ops.pop()) throw new AsmError(line, "parentesis sin abrir");
        expectVal = false;
      } else if (t in PREC || (expectVal && (t === "-" || t === "~"))) {
        if (expectVal && t === "-") ops.push("u-");
        else if (expectVal && t === "~") ops.push("u~");
        else { while (ops.length && ops[ops.length - 1] !== "(" && PREC[ops[ops.length - 1]] >= PREC[t]) apply(); ops.push(t); }
        expectVal = true;
      } else {
        const n = parseNum(t);
        out.push(n !== null ? n : resolve(t, line));
        expectVal = false;
      }
    }
    while (ops.length) { if (ops[ops.length - 1] === "(") throw new AsmError(line, "parentesis sin cerrar"); apply(); }
    if (out.length !== 1) throw new AsmError(line, "expresion invalida");
    return out[0] | 0;
  }

  function isDirective(up) {
    return up === "ORG" || up === "EQU" || up === "SET" || up === "=" ||
      up === "__CONFIG" || up === "PROCESSOR" || up === "LIST" || up === "RADIX" ||
      up === "END" || up === "BANKSEL" || up === "BANKISEL" || up === "PAGESEL" ||
      up in UNSUPPORTED;
  }
  function isHashLine(s) { return /^\s*#/.test(s); }

  // A parsed source line.
  function parseLine(raw, n) {
    const noComment = stripComment(raw);
    const hadLabelCol = /^[^\s]/.test(noComment); // something in column 1
    const text = noComment.trim();
    if (text === "") return null;

    let label = null, rest = text;
    const first = text.match(/^([A-Za-z_?][\w?]*)/);
    if (first) {
      const word = first[1];
      const up = word.toUpperCase();
      const afterWord = text.slice(word.length);
      if (/^\s*:/.test(afterWord)) {                         // "LABEL:"
        label = word; rest = afterWord.replace(/^\s*:/, "").trim();
      } else if (hadLabelCol && !(up in OPS) && !isDirective(up) && up !== "END") {
        label = word; rest = afterWord.trim();               // column-1 label
      }
    }
    const mm = rest.match(/^(\S+)/);
    const mnem = mm ? mm[1] : null;
    const operandStr = mm ? rest.slice(mm[1].length).trim() : "";
    return { n, label, mnem, mnemUp: mnem ? mnem.toUpperCase() : null, operandStr };
  }

  // ===== Phase 2 preprocessing: CBLOCK, #define text macros, MACRO/ENDM =====

  // Repeatedly replace identifier tokens that are keys in `subst` with their text,
  // until stable — so a #define can expand to another #define (or a macro param).
  function applyDefines(text, subst, line) {
    if (subst.size === 0) return text;
    let cur = text;
    for (let pass = 0; pass < 60; pass++) {
      let changed = false;
      const next = cur.replace(/[A-Za-z_?][\w?]*/g, function (tok) {
        if (subst.has(tok)) { changed = true; return subst.get(tok); }
        return tok;
      });
      if (!changed) return next;
      cur = next;
    }
    throw new AsmError(line, "expansion de #define/macro circular");
  }

  // Light parse used only during expansion: optional column-1 label, then the
  // mnemonic. A known macro name in column 1 is the operation, not a label.
  function preParse(text, macros) {
    const col0 = /^\S/.test(text);
    let t = text.trim(), label = null, m;
    if ((m = t.match(/^([A-Za-z_?][\w?]*)\s*:([\s\S]*)$/))) { label = m[1]; t = m[2].trim(); }
    else if (col0 && (m = t.match(/^([A-Za-z_?][\w?]*)\b([\s\S]*)$/))) {
      const w = m[1], up = w.toUpperCase();
      if (!macros.has(w) && !(up in OPS) && !isDirective(up) && up !== "END" && m[2].trim() !== "") { label = w; t = m[2].trim(); }
    }
    const mm = t.match(/^(\S+)\s*([\s\S]*)$/);
    return { label: label, mnem: mm ? mm[1] : null, operandStr: mm ? mm[2].trim() : "" };
  }

  // Expand one raw line into >=0 plain lines (macros expanded, #defines applied).
  function expandLine(raw, defines, macros, out, n, depth) {
    if (depth > 60) throw new AsmError(n, "expansion de macro demasiado anidada");
    let text = stripComment(raw);
    if (text.trim() === "") return;
    text = applyDefines(text, defines, n);
    const p = preParse(text, macros);
    if (p.mnem && macros.has(p.mnem)) {
      if (p.label) out.push({ text: p.label, n: n });
      const mac = macros.get(p.mnem);
      const args = p.operandStr ? splitOps(p.operandStr) : [];
      const local = new Map(defines);
      mac.params.forEach(function (pm, i) { local.set(pm, args[i] !== undefined ? args[i] : ""); });
      for (let k = 0; k < mac.body.length; k++) expandLine(mac.body[k], local, macros, out, n, depth + 1);
    } else {
      out.push({ text: text, n: n });
    }
  }

  // Walk the raw lines, building #define / MACRO / CBLOCK tables and emitting the
  // fully expanded line list (each {text, n}).
  function preprocess(rawLines) {
    const defines = new Map(), macros = new Map(), out = [];
    let cblock = null, macroDef = null;
    const numOnly = function (name, line) { throw new AsmError(line, "simbolo no permitido aca: \"" + name + "\""); };
    for (let i = 0; i < rawLines.length; i++) {
      const n = i + 1, raw = rawLines[i], t = stripComment(raw).trim();
      if (macroDef) {
        if (/^ENDM\b/i.test(t)) { macros.set(macroDef.name, { params: macroDef.params, body: macroDef.body }); macroDef = null; }
        else macroDef.body.push(raw);
        continue;
      }
      if (cblock !== null) {
        if (/^ENDC\b/i.test(t)) { cblock = null; continue; }
        if (t !== "") {
          const parts = t.split(",");
          for (let j = 0; j < parts.length; j++) {
            const e = parts[j].trim(); if (!e) continue;
            const m = e.match(/^([A-Za-z_?][\w?]*)\s*(?::\s*([\s\S]+))?$/);
            if (!m) throw new AsmError(n, "entrada de CBLOCK invalida: \"" + e + "\"");
            out.push({ text: m[1] + " EQU 0x" + (cblock & 0x3FFF).toString(16), n: n });
            cblock += m[2] ? evalExpr(applyDefines(m[2], defines, n), numOnly, n) : 1;
          }
        }
        continue;
      }
      if (t === "") continue;
      if (t.charAt(0) === "#") {
        if (/^#\s*include\b/i.test(t)) continue;
        let m;
        if ((m = t.match(/^#\s*define\s+([A-Za-z_?][\w?]*)\s*([\s\S]*)$/i))) { defines.set(m[1], (m[2] || "").trim()); continue; }
        if ((m = t.match(/^#\s*undefine\s+([A-Za-z_?][\w?]*)/i))) { defines.delete(m[1]); continue; }
        throw new AsmError(n, "directiva de preprocesador no soportada: \"" + t + "\"");
      }
      if (/^CBLOCK\b/i.test(t)) { cblock = evalExpr(applyDefines(t.replace(/^CBLOCK\s*/i, "") || "0", defines, n), numOnly, n) & 0x3FFF; continue; }
      const md = t.match(/^([A-Za-z_?][\w?]*)\s+MACRO\b\s*([\s\S]*)$/i);
      if (md) { macroDef = { name: md[1], params: md[2].split(",").map(function (x) { return x.trim(); }).filter(Boolean), body: [], startLine: n }; continue; }
      expandLine(raw, defines, macros, out, n, 0);
    }
    if (macroDef) throw new AsmError(macroDef.startLine, "falta ENDM para la macro \"" + macroDef.name + "\"");
    if (cblock !== null) throw new AsmError(rawLines.length, "falta ENDC para cerrar el CBLOCK");
    return out;
  }

  function assemble(src) {
    try { return assembleInner(src); }
    catch (e) {
      if (e instanceof AsmError) return { ok: false, line: e.line, error: "linea " + e.line + ": " + e.msg };
      return { ok: false, line: 0, error: "error interno del ensamblador: " + (e && e.message ? e.message : e) };
    }
  }

  function assembleInner(src) {
    const rawLines = src.replace(/\r\n?/g, "\n").split("\n");

    // Preprocess: expand CBLOCK/ENDC, #define text macros, and MACRO/ENDM into a
    // flat list of plain source lines (each carrying a line number for errors).
    const lines = [];
    for (const e of preprocess(rawLines)) { const p = parseLine(e.text, e.n); if (p) lines.push(p); }

    const symbols = Object.create(null);   // user EQU/SET + labels
    function resolve(name, line) {
      if (name === "$") return resolve.$;   // current location counter (words)
      const up = name.toUpperCase();
      if (up in symbols) return symbols[up];
      if (up in SFR) return SFR[up];
      if (up in BIT) return BIT[up];
      if (up in CFG) return CFG[up];
      throw new AsmError(line, "simbolo no definido: \"" + name + "\"");
    }
    resolve.$ = 0;

    // ---- Pass 1: assign addresses to labels (need instruction sizes) ----
    let lc = 0, ended = false;
    for (const L of lines) {
      if (ended) break;
      const up = L.mnemUp;
      if (up && up in UNSUPPORTED)
        throw new AsmError(L.n, UNSUPPORTED[up] + " todavia no esta soportado en el editor: compila este programa en MPLAB por ahora.");

      if (up === "EQU" || up === "SET" || up === "=") {
        if (!L.label) throw new AsmError(L.n, up + " necesita una etiqueta a la izquierda");
        resolve.$ = lc;
        symbols[L.label.toUpperCase()] = evalExpr(L.operandStr, resolve, L.n);
        continue;
      }
      if (L.label) symbols[L.label.toUpperCase()] = lc;     // label = current address

      if (!up) continue;                                    // label-only line
      if (up === "END") { ended = true; break; }
      if (up === "ORG") { resolve.$ = lc; lc = evalExpr(L.operandStr, resolve, L.n) & 0x1FFF; continue; }
      if (up === "__CONFIG" || up === "PROCESSOR" || up === "LIST" ||
          up === "RADIX" || up === "PAGESEL") continue;     // no program words here
      if (up === "BANKSEL" || up === "BANKISEL") { lc += 2; continue; } // 2 instructions
      if (up in OPS) { lc += 1; continue; }
      throw new AsmError(L.n, "instruccion o directiva desconocida: \"" + L.mnem + "\"");
    }

    // ---- Pass 2: encode ----
    const words = new Map();   // wordAddr -> 14-bit value
    let config = null;
    const put = (addr, val, line) => {
      if (addr > 0x7FF) throw new AsmError(line, "direccion de programa fuera de rango (0x" + addr.toString(16) + ")");
      words.set(addr, val & 0x3FFF);
    };

    lc = 0; ended = false;
    for (const L of lines) {
      if (ended) break;
      const up = L.mnemUp;
      if (!up || up === "EQU" || up === "SET" || up === "=") continue;
      if (up === "END") { ended = true; break; }
      if (up === "PROCESSOR" || up === "LIST" || up === "RADIX" || up === "PAGESEL") continue;
      if (up === "ORG") { resolve.$ = lc; lc = evalExpr(L.operandStr, resolve, L.n) & 0x1FFF; continue; }
      if (up === "__CONFIG") { resolve.$ = lc; config = evalExpr(L.operandStr, resolve, L.n) & 0x3FFF; continue; }

      resolve.$ = lc;
      if (up === "BANKSEL" || up === "BANKISEL") {
        const a = resolve(L.operandStr.trim(), L.n);
        const rp0 = (a >> 7) & 1, rp1 = (a >> 8) & 1;       // STATUS bit5 / bit6
        put(lc++, (rp0 ? 0x1400 : 0x1000) | (5 << 7) | SFR.STATUS, L.n); // bsf/bcf STATUS,RP0
        put(lc++, (rp1 ? 0x1400 : 0x1000) | (6 << 7) | SFR.STATUS, L.n); // bsf/bcf STATUS,RP1
        continue;
      }

      const op = OPS[up];
      if (!op) throw new AsmError(L.n, "instruccion desconocida: \"" + L.mnem + "\"");
      const ops = op.t ? splitOps(L.operandStr) : [];
      put(lc++, encode(op, ops, resolve, L.n), L.n);
    }

    return { ok: true, hex: toIntelHex(words, config), words, config };
  }

  function destBit(tok, resolve, line) {
    const t = (tok || "").trim();
    if (/^[wW]$/.test(t)) return 0;
    if (/^[fF]$/.test(t)) return 1;
    const v = evalExpr(t, resolve, line);
    if (v !== 0 && v !== 1) throw new AsmError(line, "destino invalido \"" + t + "\" (usa W/F o 0/1)");
    return v;
  }

  // A data-memory file address: 0..0x1FF across the four banks. The instruction
  // holds only the low 7 bits (the bank comes from RP1:RP0 at run time), so we
  // mask — but a value outside the 512-byte space is a real error, not a silent
  // truncation (e.g. clrf 0x200), and TRISB=0x86 stays valid (→ low 7 bits 0x06).
  function fileAddr(v, line) {
    if (v < 0 || v > 0x1FF) throw new AsmError(line, "registro fuera de rango: 0x" + (v >>> 0).toString(16) + " (0-0x1FF)");
    return v & 0x7F;
  }

  function encode(op, ops, resolve, line) {
    switch (op.t) {
      case "": return op.b;
      case "fd": {
        if (ops.length < 1) throw new AsmError(line, "falta el registro");
        const f = fileAddr(evalExpr(ops[0], resolve, line), line);
        const d = ops.length >= 2 ? destBit(ops[1], resolve, line) : 1; // MPASM default: F
        return op.b | (d << 7) | f;
      }
      case "f": {
        if (ops.length < 1) throw new AsmError(line, "falta el registro");
        return op.b | fileAddr(evalExpr(ops[0], resolve, line), line);
      }
      case "fb": {
        if (ops.length < 2) throw new AsmError(line, "falta el numero/nombre de bit");
        const f = fileAddr(evalExpr(ops[0], resolve, line), line);
        const bit = evalExpr(ops[1], resolve, line);
        if (bit < 0 || bit > 7) throw new AsmError(line, "bit fuera de rango (0-7): " + bit);
        return op.b | (bit << 7) | f;
      }
      case "k": {
        if (ops.length < 1) throw new AsmError(line, "falta el literal");
        const k = evalExpr(ops[0], resolve, line);
        if (k < -128 || k > 255) throw new AsmError(line, "literal fuera de rango: " + k + " (no entra en 8 bits, 0-255)");
        return op.b | (k & 0xFF);
      }
      case "a": {
        if (ops.length < 1) throw new AsmError(line, "falta la direccion de salto");
        const a = evalExpr(ops[0], resolve, line);
        if (a < 0 || a > 0x7FF) throw new AsmError(line, "direccion de salto fuera de rango: 0x" + (a >>> 0).toString(16) + " (0-0x7FF)");
        return op.b | (a & 0x7FF);
      }
    }
    throw new AsmError(line, "tipo de operando interno desconocido");
  }

  // ---- Intel HEX output (INHX32, the shape MPLAB emits) ----
  function rec(byteAddr, type, data) {
    const len = data.length;
    const bytes = [len, (byteAddr >> 8) & 0xFF, byteAddr & 0xFF, type, ...data];
    let sum = 0; for (const b of bytes) sum = (sum + b) & 0xFF;
    const ck = (0x100 - sum) & 0xFF;
    return ":" + [...bytes, ck].map((b) => b.toString(16).toUpperCase().padStart(2, "0")).join("");
  }
  function toIntelHex(words, config) {
    // word map -> byte map (little-endian 14-bit words)
    const byteMap = new Map();
    for (const [w, v] of words) { byteMap.set(w * 2, v & 0xFF); byteMap.set(w * 2 + 1, (v >> 8) & 0xFF); }
    if (config !== null) { byteMap.set(0x400E, config & 0xFF); byteMap.set(0x400F, (config >> 8) & 0xFF); }

    const addrs = [...byteMap.keys()].sort((a, b) => a - b);
    const out = [rec(0x0000, 0x04, [0x00, 0x00])];  // extended linear address 0
    let i = 0;
    while (i < addrs.length) {
      const start = addrs[i]; const data = [byteMap.get(start)];
      let prev = start; i++;
      while (i < addrs.length && addrs[i] === prev + 1 && data.length < 16 && (addrs[i] & 0xFFFF) !== 0) {
        data.push(byteMap.get(addrs[i])); prev = addrs[i]; i++;
      }
      out.push(rec(start & 0xFFFF, 0x00, data));
    }
    out.push(":00000001FF");
    return out.join("\n") + "\n";
  }

  const API = { assemble, SFR, BIT, CFG, OPS };
  global.NP_ASM = API;
  if (typeof module !== "undefined" && module.exports) module.exports = API;
})(typeof window !== "undefined" ? window : globalThis);
