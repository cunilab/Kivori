//! Device Studio preview-frame streaming over a Tauri `Channel` (contracts/ipc.md §3; T053).
//!
//! Dev-only. `open_preview_stream` starts a bounded producer thread that renders frames with the shared
//! renderer and pushes raw RGBA8888 over a `Channel` — the webview only blits them (constraint 2; there
//! is no TypeScript renderer). The stream is:
//!
//! * **drift-free** — each frame's canonical time comes from an integer step index ([`step_ms`]), never
//!   accumulated, so a long stream cannot drift (Principle III / ADR-0003);
//! * **rate-capped** — the requested `fps` is clamped to [`MIN_FPS`]..=[`MAX_FPS`];
//! * **back-pressured** — at most [`MAX_IN_FLIGHT`] unacknowledged frames may be outstanding
//!   ([`StreamControl::try_reserve`]); the producer stalls rather than queueing, so a slow webview can
//!   never grow an unbounded backlog;
//! * **cancellable + self-cleaning** — `close_preview_stream`, a failed send (webview gone), or
//!   [`PreviewStreams::cancel_all`] on shutdown/teardown stops the thread and drops the registry entry.
//!
//! The registry and its control primitives are plain `std` types, so the bounding, cancellation, and
//! acknowledgement rules are unit-tested without a running webview.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::ipc::dto;
use crate::render::animation::AnimationTimeline;
use crate::runtime::state::AppState;

/// Slowest permitted stream rate, in frames per second.
pub const MIN_FPS: u16 = 1;
/// Fastest permitted stream rate, in frames per second (the canonical preview rate).
pub const MAX_FPS: u16 = 30;
/// Maximum unacknowledged frames allowed in flight before the producer stalls (back-pressure).
pub const MAX_IN_FLIGHT: usize = 2;

/// The canonical elapsed time of stream frame `step` at `fps`, in integer milliseconds.
///
/// `(step * 1000 + fps/2) / fps` is round-to-nearest with no floating point and no accumulation, so
/// frame *n* always has the same timestamp however long the stream has run. At the canonical 30 fps
/// this matches `kivori_model::frame_step_ms`.
#[must_use]
pub fn step_ms(step: u64, fps: u16) -> u32 {
    let fps = u64::from(fps.clamp(MIN_FPS, MAX_FPS));
    let ms = (step * 1000 + fps / 2) / fps;
    u32::try_from(ms).unwrap_or(u32::MAX)
}

/// Per-stream control shared between the command layer and the producer thread.
#[derive(Debug, Default)]
pub struct StreamControl {
    cancel: AtomicBool,
    in_flight: AtomicUsize,
    pending: Mutex<Option<(AnimationTimeline, u32, usize)>>,
    revision: AtomicUsize,
}

impl StreamControl {
    /// Coalesces externally-clocked preview requests into a single pending frame.
    pub fn update(&self, animation: AnimationTimeline, elapsed_ms: u32) {
        let mut pending = self.pending.lock().expect("preview request lock");
        let revision = self.revision.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        *pending = Some((animation, elapsed_ms, revision));
    }

    /// Takes the newest pending request, dropping obsolete timestamps before rendering.
    pub fn take_request(&self) -> Option<(AnimationTimeline, u32, usize)> {
        self.pending.lock().expect("preview request lock").take()
    }
    /// Whether a request has been superseded while its frame was rendering.
    pub fn is_current(&self, revision: usize) -> bool {
        self.revision.load(Ordering::SeqCst) == revision
    }
    /// Whether the stream has been cancelled (producer must stop).
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Marks the stream cancelled.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Unacknowledged frames currently in flight.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Takes a back-pressure slot. Returns `false` when [`MAX_IN_FLIGHT`] frames are already
    /// outstanding, in which case the producer MUST stall instead of queueing another frame.
    // ponytail: `fetch_update` is deprecated on Rust 1.99 but `try_update` needs 1.95 (MSRV is 1.90); switch when MSRV moves.
    #[allow(deprecated)]
    pub fn try_reserve(&self) -> bool {
        self.in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < MAX_IN_FLIGHT).then_some(n + 1)
            })
            .is_ok()
    }

    /// Releases one slot (a delivered frame was acknowledged). Saturating: a duplicate ack is a no-op.
    #[allow(deprecated)]
    pub fn release(&self) {
        let _ = self
            .in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_sub(1))
            });
    }
}

/// The registry of live preview streams, keyed by channel id.
#[derive(Default)]
pub struct PreviewStreams {
    streams: Mutex<HashMap<u32, Arc<StreamControl>>>,
}

impl PreviewStreams {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of currently-registered streams.
    #[must_use]
    pub fn len(&self) -> usize {
        self.streams.lock().expect("streams lock").len()
    }

    /// Whether no stream is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Registers a stream and returns the control shared with its producer thread.
    pub fn register(&self, id: u32) -> Arc<StreamControl> {
        let control = Arc::new(StreamControl::default());
        self.streams
            .lock()
            .expect("streams lock")
            .insert(id, Arc::clone(&control));
        control
    }

    /// The control for `id`, if registered.
    #[must_use]
    pub fn control(&self, id: u32) -> Option<Arc<StreamControl>> {
        self.streams.lock().expect("streams lock").get(&id).cloned()
    }

    /// Drops the registry entry for `id` (producer cleanup).
    pub fn remove(&self, id: u32) {
        self.streams.lock().expect("streams lock").remove(&id);
    }

    /// Cancels one stream and drops its entry. Returns `true` if it was registered.
    pub fn cancel(&self, id: u32) -> bool {
        match self.streams.lock().expect("streams lock").remove(&id) {
            Some(control) => {
                control.cancel();
                true
            }
            None => false,
        }
    }

    /// Cancels every stream (window teardown / explicit quit).
    pub fn cancel_all(&self) {
        for (_, control) in self.streams.lock().expect("streams lock").drain() {
            control.cancel();
        }
    }

    /// Acknowledges one delivered frame, freeing a back-pressure slot. `true` if the stream exists.
    pub fn ack(&self, id: u32) -> bool {
        match self.control(id) {
            Some(control) => {
                control.release();
                true
            }
            None => false,
        }
    }
}

/// Opens a capped, back-pressured preview stream for `state`; returns the channel id as its handle.
///
/// # Errors
/// Returns an error string if `state` is not a companion token, or the producer thread cannot start.
#[tauri::command]
pub fn open_preview_stream(
    app: State<'_, AppState>,
    state: String,
    fps: u16,
    channel: Channel<InvokeResponseBody>,
    animation: Option<AnimationTimeline>,
    elapsed_ms: Option<u32>,
) -> Result<u32, String> {
    let companion = dto::companion_from_token(&state)
        .ok_or_else(|| format!("unknown companion state: {state}"))?;
    let fps = fps.clamp(MIN_FPS, MAX_FPS);
    if let Some(animation) = &animation {
        animation.validate()?;
    }
    let external_clock = animation.is_some();
    let id = channel.id();
    let control = app.previews.register(id);
    if let Some(animation) = animation {
        control.update(animation, elapsed_ms.unwrap_or(0));
    }
    let streams = Arc::clone(&app.previews);
    let frame_interval = Duration::from_millis(1000 / u64::from(fps));

    std::thread::Builder::new()
        .name(format!("kivori-preview-{id}"))
        .spawn(move || {
            let mut step: u64 = 0;
            while !control.is_cancelled() {
                // Back-pressure: stall (never queue) while the webview is behind.
                if !control.try_reserve() {
                    std::thread::sleep(frame_interval);
                    continue;
                }
                let rgba = if external_clock {
                    let Some((animation, elapsed_ms, revision)) = control.take_request() else {
                        control.release();
                        std::thread::sleep(frame_interval);
                        continue;
                    };
                    let rendered = match crate::render::render_animation_rgba(
                        crate::render::bundled_blob(),
                        &animation,
                        elapsed_ms,
                    ) {
                        Ok(rgba) => rgba,
                        Err(_) => break,
                    };
                    if !control.is_current(revision) {
                        control.release();
                        continue;
                    }
                    rendered
                } else {
                    crate::render::render_preview_bundled(
                        companion,
                        elapsed_ms.unwrap_or(0).saturating_add(step_ms(step, fps)),
                    )
                };
                if channel.send(InvokeResponseBody::Raw(rgba)).is_err() {
                    break; // webview gone — stop and clean up
                }
                step = step.wrapping_add(1);
                std::thread::sleep(frame_interval);
            }
            streams.remove(id);
        })
        .map_err(|e| {
            app.previews.cancel(id);
            format!("could not start preview stream: {e}")
        })?;
    Ok(id)
}

/// Updates an existing externally-clocked stream without replacing its producer or channel.
#[tauri::command]
pub fn update_preview_stream(
    app: State<'_, AppState>,
    handle: u32,
    animation: AnimationTimeline,
    elapsed_ms: u32,
) -> Result<(), String> {
    animation.validate()?;
    let control = app
        .previews
        .control(handle)
        .ok_or("preview stream is closed")?;
    control.update(animation, elapsed_ms);
    Ok(())
}

/// Closes a preview stream by handle. Idempotent: closing an unknown handle returns `false`.
#[tauri::command]
pub fn close_preview_stream(app: State<'_, AppState>, handle: u32) -> bool {
    app.previews.cancel(handle)
}

/// Acknowledges one received frame, releasing a back-pressure slot.
#[tauri::command]
pub fn ack_preview_frame(app: State<'_, AppState>, handle: u32) -> bool {
    app.previews.ack(handle)
}
