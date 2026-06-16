# Test fixtures

MPLAB-built `.hex` files used to cross-check the core against real firmware.

- **`blink.hex`** — a 5-instruction self-bootstrap program (increment PORTB in a
  loop). Used by `tests/integration.rs` to verify the HEX loader and the
  disassembler. Source equivalent:

  ```asm
          BSF   STATUS, RP0   ; bank 1
          CLRF  TRISB         ; PORTB all outputs
          BCF   STATUS, RP0   ; bank 0
  loop:   INCF  PORTB, F      ; count up on PORTB
          GOTO  loop
  ```

## Adding one of your lab programs

1. Copy `yourlab.hex` (and optionally `yourlab.asm`) into this folder.
2. In `tests/integration.rs`, add a test that `hex::parse`s it and asserts the
   disassembly of the first few words matches your `.asm`. Example:

   ```rust
   let img = hex::parse(include_str!("fixtures/yourlab.hex")).unwrap();
   assert_eq!(decode::disassemble(img.program[0]).as_str(), "MOVLW 0x07");
   ```

This catches decode/encoding mistakes immediately. The deeper, cycle-by-cycle
execute comparison against MPLAB's simulator comes once instruction execute is
implemented (next pass).
