//! Named M1 desk frames, shared by the golden test and the `desk_review` example.

use crate::DIM;
use kivori_framebuffer::TileBand;
use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, DeskStatus, DeskView, DisplayMode, FeedbackKind,
    MediaStatus,
};
use kivori_model::presentation::{ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Rect, Rgb565};
use kivori_renderer::desk;

/// One named desk frame: either the recovery takeover or a mode view with the chrome drawn over
/// it (the mascot itself is out of scope here, so `Buddy` shows its plain background).
pub enum DeskFrame {
    /// `render_recovery` at this percent.
    Recovery(u8),
    /// `render_mode` (with an optional live volume overlay) then `render_chrome`.
    View(DeskView, Option<ValueDisplay>),
}

/// Renders a [`DeskFrame`] as one full 240x240 band.
#[must_use]
pub fn render_desk(frame: &DeskFrame) -> Vec<Rgb565> {
    let mut buf = vec![Rgb565::from_raw(0); DIM as usize * DIM as usize];
    let mut band = TileBand::new(Rect::new(0, 0, DIM, DIM), &mut buf).expect("full-frame band");
    match frame {
        DeskFrame::Recovery(percent) => desk::render_recovery(&mut band, *percent),
        DeskFrame::View(view, overlay) => {
            desk::render_mode(&mut band, view, *overlay);
            desk::render_chrome(&mut band, view);
        }
    }
    buf
}

fn view(mode: DisplayMode, edit: impl FnOnce(&mut DeskView)) -> DeskFrame {
    let mut v = DeskView::default();
    v.status.mode = mode;
    edit(&mut v);
    DeskFrame::View(v, None)
}

fn feedback(kind: FeedbackKind, elapsed_ms: u32) -> DeskFrame {
    view(DisplayMode::Volume, |v| {
        v.status.volume_percent = Some(42);
        v.feedback = Some(ActionFeedback {
            action: ActionKind::Mute,
            kind,
        });
        v.elapsed_ms = elapsed_ms;
    })
}

/// The named desk frames pinned in `manifest.toml` `[desk_frames]` and exported by the
/// `desk_review` example.
#[must_use]
pub fn desk_frames() -> Vec<(&'static str, DeskFrame)> {
    let at = |hour, minute, second| {
        Some(ClockTime {
            hour,
            minute,
            second,
        })
    };
    let overlay = DeskFrame::View(
        DeskView {
            status: DeskStatus {
                mode: DisplayMode::Volume,
                ..DeskStatus::UNKNOWN
            },
            ..DeskView::default()
        },
        Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 75,
            confidence: ValueConfidence::Unverified,
            at_boundary: false,
        }),
    );
    vec![
        (
            "clock_0941_colon",
            view(DisplayMode::Clock, |v| v.status.clock = at(9, 41, 30)),
        ),
        (
            "clock_0941_blink_off",
            view(DisplayMode::Clock, |v| v.status.clock = at(9, 41, 31)),
        ),
        ("clock_unknown", view(DisplayMode::Clock, |_| {})),
        (
            "volume_42",
            view(DisplayMode::Volume, |v| v.status.volume_percent = Some(42)),
        ),
        (
            "volume_100_muted",
            view(DisplayMode::Volume, |v| {
                v.status.volume_percent = Some(100);
                v.status.muted = Some(true);
            }),
        ),
        ("volume_unknown", view(DisplayMode::Volume, |_| {})),
        ("volume_overlay_unverified_75", overlay),
        (
            "media_playing",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Playing)
            }),
        ),
        (
            "media_paused",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Paused)
            }),
        ),
        (
            "media_stopped",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Stopped)
            }),
        ),
        ("media_unknown", view(DisplayMode::Media, |_| {})),
        (
            "system_normal",
            view(DisplayMode::System, |v| {
                v.status.cpu_percent = Some(37);
                v.status.ram_percent = Some(62);
            }),
        ),
        (
            "system_high_load",
            view(DisplayMode::System, |v| {
                v.status.cpu_percent = Some(93);
                v.status.ram_percent = Some(71);
                v.status.high_load = true;
            }),
        ),
        ("system_unknown", view(DisplayMode::System, |_| {})),
        ("recovery_0", DeskFrame::Recovery(0)),
        ("recovery_50", DeskFrame::Recovery(50)),
        ("recovery_100", DeskFrame::Recovery(100)),
        (
            "buddy_load_pressed",
            view(DisplayMode::Buddy, |v| {
                v.status.high_load = true;
                v.button_down = true;
            }),
        ),
        (
            "buddy_indicators",
            view(DisplayMode::Buddy, |v| {
                v.status.muted = Some(true);
                v.status.media = Some(MediaStatus::Playing);
                v.elapsed_ms = 600;
            }),
        ),
        (
            "feedback_state_confirmed",
            feedback(FeedbackKind::StateConfirmed, 0),
        ),
        (
            "feedback_execution_confirmed",
            feedback(FeedbackKind::ExecutionConfirmed, 0),
        ),
        ("feedback_unverified", feedback(FeedbackKind::Unverified, 0)),
        ("feedback_error", feedback(FeedbackKind::Error, 0)),
        (
            "feedback_processing_0",
            feedback(FeedbackKind::Processing, 0),
        ),
        (
            "feedback_processing_300",
            feedback(FeedbackKind::Processing, 300),
        ),
    ]
}
