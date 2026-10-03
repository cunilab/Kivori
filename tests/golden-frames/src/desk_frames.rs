//! Named M1 desk frames, shared by the golden test and the `desk_review` example.

use crate::DIM;
use kivori_framebuffer::TileBand;
use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, CpuHistory, DeskStatus, DeskView, DisplayMode,
    FeedbackKind, MediaInfo, MediaStatus, MediaText,
};
use kivori_model::presentation::{ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Rect, Rgb565};
use kivori_renderer::desk;

/// One named desk frame: either the recovery takeover or the view layer (with any switch
/// animation) with the chrome drawn over it (the mascot itself is out of scope here, so `Buddy`
/// shows its plain background).
pub enum DeskFrame {
    /// `render_recovery` at this percent.
    Recovery(u8),
    /// `render_views` (with an optional live volume overlay) then `render_chrome`.
    View(DeskView, Option<ValueDisplay>),
}

/// Renders a [`DeskFrame`] as one full 240x240 band.
#[must_use]
pub fn render_desk(frame: &DeskFrame) -> Vec<Rgb565> {
    render_desk_with(frame, |_| {})
}

/// [`render_desk`] with `buddy` painting the Buddy view over its background (review exports draw
/// the real mascot there).
#[must_use]
pub fn render_desk_with(frame: &DeskFrame, mut buddy: impl FnMut(&mut TileBand)) -> Vec<Rgb565> {
    let mut buf = vec![Rgb565::from_raw(0); DIM as usize * DIM as usize];
    let mut band = TileBand::new(Rect::new(0, 0, DIM, DIM), &mut buf).expect("full-frame band");
    match frame {
        DeskFrame::Recovery(percent) => desk::render_recovery(&mut band, *percent),
        DeskFrame::View(view, overlay) => {
            let _ = desk::render_views(&mut band, view, |b, mode| {
                desk::render_view(b, mode, view, *overlay);
                if mode == DisplayMode::Buddy {
                    buddy(b);
                }
                Ok::<(), ()>(())
            });
            // As on the device: the volume bar over any view except Volume.
            if let (Some(o), true) = (overlay, view.status.mode != DisplayMode::Volume) {
                kivori_renderer::overlay::render_volume_overlay(
                    &mut band,
                    o.current_percent,
                    o.confidence,
                    o.at_boundary,
                );
            }
            desk::render_chrome(&mut band, view);
        }
    }
    buf
}

fn media(title: &str, artist: &str) -> Option<MediaInfo> {
    Some(MediaInfo {
        title: MediaText::from_text(title),
        artist: MediaText::from_text(artist),
    })
}

fn history(len: usize) -> CpuHistory {
    let mut h = CpuHistory::EMPTY;
    // A deterministic, plausible load trace: a calm baseline with a build spike.
    for i in 0..len {
        let base = 18 + (i * 7 % 11) as u8;
        let spike = if (34..46).contains(&i) {
            52 - 4 * (i as u8).abs_diff(40)
        } else {
            0
        };
        h.push(base + spike);
    }
    h
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
            "clock_0941_30s",
            view(DisplayMode::Clock, |v| v.status.clock = at(9, 41, 30)),
        ),
        (
            "clock_0941_next_second",
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
        (
            "clock_chips",
            view(DisplayMode::Clock, |v| {
                v.status.clock = at(21, 7, 45);
                v.status.volume_percent = Some(42);
                v.status.cpu_percent = Some(12);
            }),
        ),
        (
            "volume_preview_58",
            DeskFrame::View(
                DeskView {
                    status: DeskStatus {
                        mode: DisplayMode::Volume,
                        volume_percent: Some(42),
                        ..DeskStatus::UNKNOWN
                    },
                    ..DeskView::default()
                },
                Some(ValueDisplay {
                    kind: ValueKind::Volume,
                    current_percent: 58,
                    confidence: ValueConfidence::Preview,
                    at_boundary: false,
                }),
            ),
        ),
        (
            "media_playing_info",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Playing);
                v.media_info = media("Clair de lune", "Debussy");
                v.elapsed_ms = 400;
            }),
        ),
        (
            "media_long_title_0",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Playing);
                v.media_info = media(
                    "Señorita (Live at the Café Olé, Zürich)",
                    "Ångström & the Naïve Øresund Orchestra",
                );
            }),
        ),
        (
            "media_long_title_4000",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Playing);
                v.media_info = media(
                    "Señorita (Live at the Café Olé, Zürich)",
                    "Ångström & the Naïve Øresund Orchestra",
                );
                v.elapsed_ms = 4_000;
            }),
        ),
        (
            "media_paused_info",
            view(DisplayMode::Media, |v| {
                v.status.media = Some(MediaStatus::Paused);
                v.media_info = media("Weightless", "Marconi Union");
            }),
        ),
        (
            "system_history",
            view(DisplayMode::System, |v| {
                v.status.cpu_percent = Some(24);
                v.status.ram_percent = Some(58);
                v.cpu_history = history(60);
            }),
        ),
        (
            "system_history_high_load",
            view(DisplayMode::System, |v| {
                v.status.cpu_percent = Some(91);
                v.status.ram_percent = Some(83);
                v.status.high_load = true;
                v.cpu_history = history(25);
                for _ in 0..8 {
                    v.cpu_history.push(92);
                }
            }),
        ),
        (
            "buddy_status_row",
            view(DisplayMode::Buddy, |v| {
                v.status.clock = at(9, 41, 0);
                v.status.muted = Some(true);
                v.status.media = Some(MediaStatus::Playing);
            }),
        ),
        (
            "switch_clock_to_volume_mid",
            view(DisplayMode::Volume, |v| {
                v.status.clock = at(9, 41, 30);
                v.status.volume_percent = Some(42);
                v.previous_mode = Some(DisplayMode::Clock);
                v.mode_age_ms = 90;
            }),
        ),
        (
            "switch_media_to_system_mid",
            view(DisplayMode::System, |v| {
                v.status.media = Some(MediaStatus::Playing);
                v.status.cpu_percent = Some(37);
                v.status.ram_percent = Some(62);
                v.media_info = media("Clair de lune", "Debussy");
                v.cpu_history = history(60);
                v.previous_mode = Some(DisplayMode::Media);
                v.mode_age_ms = 60;
            }),
        ),
        (
            "switch_buddy_to_clock_mid",
            view(DisplayMode::Clock, |v| {
                v.status.clock = at(9, 41, 30);
                v.previous_mode = Some(DisplayMode::Buddy);
                v.mode_age_ms = 120;
            }),
        ),
    ]
}
