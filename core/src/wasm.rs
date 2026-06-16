//! Browser bindings — a tiny C-ABI over [`Core`], built only for the wasm
//! target. No `wasm-bindgen`/`wasm-pack`: the core is dependency-free, so it
//! compiles straight to `wasm32-unknown-unknown` and the JS runtime calls these
//! exported functions directly.
//!
//! Build:
//!   rustup target add wasm32-unknown-unknown
//!   cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib
//!   # -> target/wasm32-unknown-unknown/release/new_proteus_core.wasm
//!
//! Interface (architecture.md §5): a single global Core driven through scalar
//! calls. The `.hex` text is passed by copying its bytes into a static buffer
//! (`np_hex_buffer`) and calling `np_load_hex(len)`.
//!
//! Statics are accessed through raw pointers (`addr_of_mut!`) rather than direct
//! references, to stay clear of the `static_mut_refs` lint / future hard error.

use crate::Core;

static mut CORE: Option<Core> = None;

const HEX_BUF_LEN: usize = 32 * 1024;
static mut HEX_BUF: [u8; HEX_BUF_LEN] = [0; HEX_BUF_LEN];

fn sim() -> &'static mut Core {
    // Single-threaded wasm: a process-global Core is fine.
    unsafe { (*core::ptr::addr_of_mut!(CORE)).get_or_insert_with(Core::new) }
}

/// Pointer to the static buffer the JS side copies `.hex` text into.
#[no_mangle]
pub extern "C" fn np_hex_buffer() -> *mut u8 {
    core::ptr::addr_of_mut!(HEX_BUF) as *mut u8
}

/// Capacity of the hex buffer in bytes.
#[no_mangle]
pub extern "C" fn np_hex_buffer_len() -> usize {
    HEX_BUF_LEN
}

/// Parse `len` bytes of Intel HEX from the buffer; returns 1 on success, 0 on
/// parse error.
#[no_mangle]
pub extern "C" fn np_load_hex(len: usize) -> u32 {
    let n = if len < HEX_BUF_LEN { len } else { HEX_BUF_LEN };
    let bytes = unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(HEX_BUF) as *const u8, n) };
    let text = match core::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    match sim().load_hex(text) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Power-on reset.
#[no_mangle]
pub extern "C" fn np_reset() {
    sim().reset();
}

/// Advance the simulation by `n` instruction cycles.
#[no_mangle]
pub extern "C" fn np_run_cycles(n: u32) {
    let _ = sim().run_cycles(n as u64);
}

/// Reset the per-frame on-time accumulators (call at the start of each frame).
#[no_mangle]
pub extern "C" fn np_reset_frame() {
    sim().reset_frame_accumulators();
}

/// Effective PORTA / PORTB pin levels (snapshot).
#[no_mangle]
pub extern "C" fn np_read_porta() -> u32 {
    sim().read_pins().0 as u32
}
#[no_mangle]
pub extern "C" fn np_read_portb() -> u32 {
    sim().read_pins().1 as u32
}

/// Fraction (0.0..=1.0) of the last frame that `pin` (0..15) was driven high —
/// the persistence-of-vision brightness value.
#[no_mangle]
pub extern "C" fn np_pin_duty(pin: u32) -> f32 {
    sim().pin_duty(pin as usize)
}

/// Drive an external input pin (a button) by index 0..15.
#[no_mangle]
pub extern "C" fn np_set_pin(pin: u32, level: u32) {
    sim().set_pin(pin as usize, level != 0);
}

/// Change the clock (affects cycle -> wall-clock conversion only).
#[no_mangle]
pub extern "C" fn np_set_clock_hz(hz: u32) {
    sim().set_clock_hz(hz);
}

/// Cycles to run per 60 fps frame at the current clock.
#[no_mangle]
pub extern "C" fn np_cycles_per_frame_60() -> u32 {
    sim().cycles_per_frame_60() as u32
}
