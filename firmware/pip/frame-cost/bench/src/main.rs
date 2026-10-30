//! Frame-cost bench: composes representative Pip frames through the SAME `TileRenderer` the
//! product firmware runs (`prepare_frame_row`, once per tile row, into a static frame buffer),
//! compiled for riscv32imc with the product's release profile. It has no hardware: the instruction-set
//! simulator in `../` loads this ELF and reads the marker writes below to attribute cycles to rows.
//!
//! Marker protocol (stores to the unmapped `MARK` window; the simulator intercepts them and charges
//! nothing): scenario name pointer and length, then scenario/frame/row begin and end, then exit.
#![no_std]
#![no_main]

use core::hint::black_box;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::DisplaySink;
use kivori_firmware::render::{TileRenderer, FRAME_PIXELS};
use kivori_model::{Rect, Rgb565};

mod scenes;
use scenes::{frame_hash, Frame, SCENARIOS};

/// Same asset blob the product embeds (`include_bytes!` in `firmware/pip/src/main.rs`); lives in DROM.
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

/// The product's frame buffer is a static in RAM (`physical_st7789::FRAME_BUFFER`).
static mut FRAME_BUFFER: [Rgb565; FRAME_PIXELS] = [Rgb565::from_raw(0); FRAME_PIXELS];

/// Reads the frame buffer the renderer just filled. The renderer is idle here, so nothing else
/// accesses it while this shared view exists.
fn frame_pixels() -> &'static [Rgb565] {
    // SAFETY: single-threaded; the renderer's exclusive borrow is not used during the read.
    unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(FRAME_BUFFER).cast(), FRAME_PIXELS) }
}

const MARK: usize = 0x6000_0000;
const M_SCENARIO: usize = 0x00;
const M_FRAME_BEGIN: usize = 0x04;
const M_ROW_BEGIN: usize = 0x08;
const M_ROW_END: usize = 0x0C;
const M_FRAME_END: usize = 0x10;
const M_EXIT: usize = 0x14;
const M_NAME_PTR: usize = 0x20;
const M_NAME_LEN: usize = 0x24;

fn mark(offset: usize, value: u32) {
    // SAFETY: the simulator maps this window; on any other target it would fault, by design.
    unsafe { core::ptr::write_volatile((MARK + offset) as *mut u32, value) }
}

core::arch::global_asm!(
    ".section .text._start",
    ".globl _start",
    "_start:",
    "la sp, _stack_top",
    "call bench_main",
    "1: j 1b",
);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    mark(M_EXIT, 2);
    halt()
}

/// The simulator stops at the exit marker; this only keeps the function from returning.
fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Nothing is transferred while composing (changed tiles stay pending), so the sink is never called.
struct NullSink;
impl DisplaySink for NullSink {
    type Error = ();
    fn blit_tile(&mut self, _: Rect, _: &[Rgb565]) -> Result<(), ()> {
        Ok(())
    }
}

#[no_mangle]
extern "C" fn bench_main() -> ! {
    let blob = match AssetBlob::parse(black_box(ASSETS)) {
        Ok(blob) => blob,
        Err(_) => {
            mark(M_EXIT, 3);
            halt()
        }
    };
    // SAFETY: single-threaded, and this is the only reference to the static.
    let storage = unsafe { &mut *core::ptr::addr_of_mut!(FRAME_BUFFER) };
    let mut renderer = TileRenderer::with_frame_buffer(storage);
    let mut sink = NullSink;
    for (id, scenario) in SCENARIOS.iter().enumerate() {
        mark(M_NAME_PTR, scenario.name.as_ptr() as u32);
        mark(M_NAME_LEN, scenario.name.len() as u32);
        mark(M_SCENARIO, id as u32);
        for i in 0..scenario.frames {
            let frame = black_box((scenario.build)(black_box(i)));
            mark(M_FRAME_BEGIN, i);
            compose(&mut renderer, &blob, &frame, &mut sink);
            // The checksum of the composed frame proves the simulator ran the same code as a host build.
            mark(M_FRAME_END, frame_hash(black_box(frame_pixels())));
        }
    }
    mark(M_EXIT, 0);
    halt()
}

#[cfg(not(feature = "whole-frame"))]
fn compose(r: &mut TileRenderer, blob: &AssetBlob, f: &Frame, sink: &mut NullSink) {
    for row in 0..kivori_firmware::render::TILE_ROWS {
        mark(M_ROW_BEGIN, row as u32);
        let result = r.prepare_frame_row(blob, f.state, &f.pose, f.overlay, &f.view, row, sink);
        mark(M_ROW_END, row as u32);
        if result.is_err() {
            mark(M_EXIT, 4);
        }
    }
}

/// Pre-#150 renderer: no per-row entry point, so the whole frame is one span (row 0).
#[cfg(feature = "whole-frame")]
fn compose(r: &mut TileRenderer, blob: &AssetBlob, f: &Frame, sink: &mut NullSink) {
    mark(M_ROW_BEGIN, 0);
    let result = r.prepare_frame(blob, f.state, &f.pose, f.overlay, &f.view, sink);
    mark(M_ROW_END, 0);
    if result.is_err() {
        mark(M_EXIT, 4);
    }
}
