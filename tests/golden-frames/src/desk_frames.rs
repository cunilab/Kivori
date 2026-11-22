//! Named M1 desk frames, shared by the golden test and the `desk_review` example.

use crate::DIM;
use kivori_framebuffer::TileBand;
use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, ControlLabels, CpuHistory, DeskStatus, DeskView,
    DisplayMode, FeedbackKind, MediaInfo, MediaStatus, MediaText,
};
use kivori_model::picker::{HoldRing, PickerName, PickerPhase, PickerView};
use kivori_model::presentation::{ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Rect, Rgb565};
use kivori_renderer::desk;

/// One named desk frame: either the recovery takeover or the view layer (with any switch
/// animation) with the chrome drawn over it (the mascot itself is out of scope here, so `Buddy`
/// shows its plain background).
// A short-lived list of test fixtures; boxing the view would only add noise.
#[allow(clippy::large_enum_variant)]
pub enum DeskFrame {
    /// `render_recovery` at this percent.
    Recovery(u8),
    /// `render_updating`.
    Updating,
    /// `render_wrong_board`.
    WrongBoard,
    /// `render_views` (a live volume overlay takes the screen as the Volume view) then `render_chrome`.
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
        DeskFrame::Updating => desk::render_updating(&mut band),
        DeskFrame::WrongBoard => desk::render_wrong_board(&mut band),
        // The profile picker owns the whole screen, with no chrome (as on the device).
        DeskFrame::View(view, _) if view.picker.is_some() => {
            desk::render_picker(&mut band, view);
        }
        DeskFrame::View(view, overlay) => {
            // As on the device: a live volume value takes the whole screen as the Volume view
            // (in Buddy too), unless Volume is already the current view.
            if overlay.is_some() && view.status.mode != DisplayMode::Volume {
                desk::render_view(&mut band, DisplayMode::Volume, view, *overlay);
            } else {
                let _ = desk::render_views(&mut band, view, |b, mode| {
                    desk::render_view(b, mode, view, *overlay);
                    if mode == DisplayMode::Buddy {
                        buddy(b);
                    }
                    Ok::<(), ()>(())
                });
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

fn controls(rotate: &str, press: &str, hold: &str) -> Option<ControlLabels> {
    Some(ControlLabels {
        rotate: MediaText::from_text(rotate),
        press: MediaText::from_text(press),
        hold: MediaText::from_text(hold),
        ..ControlLabels::default()
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

fn picker(index: u8, from: u8, name: &str, phase: PickerPhase) -> DeskFrame {
    view(DisplayMode::Buddy, |v| {
        v.picker = Some(PickerView {
            count: 3,
            index,
            from,
            moved_ms: 1_000,
            name: PickerName::from_text(name),
            phase,
            phase_ms: 0,
        });
        v.button_down = true;
        v.switch_down = true;
    })
}

/// The Buddy view with a Hold ring at `percent` (the knob switch held, Hold = Mute).
fn ring(percent: u8) -> DeskFrame {
    view(DisplayMode::Buddy, |v| {
        v.status.clock = Some(ClockTime {
            hour: 8,
            minute: 5,
            second: 0,
        });
        let mut c = controls("Volume", "Play/Pause", "Mute");
        if let Some(c) = c.as_mut() {
            c.buttons = ["Previous", "Play/Pause", "Next"].map(MediaText::from_text);
            c.profile = MediaText::from_text("Media");
            c.pinned = true;
        }
        v.controls = c;
        v.button_down = true;
        v.switch_down = true;
        v.hold = Some(HoldRing {
            control: 0,
            percent,
            label: MediaText::from_text("Mute"),
        });
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
            // A live volume value over the Buddy view takes the whole screen as the Volume view.
            "buddy_volume_takeover_confirmed_58",
            DeskFrame::View(
                DeskView::default(),
                Some(ValueDisplay {
                    kind: ValueKind::Volume,
                    current_percent: 58,
                    confidence: ValueConfidence::Confirmed,
                    at_boundary: false,
                }),
            ),
        ),
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
        ("updating", DeskFrame::Updating),
        ("wrong_board", DeskFrame::WrongBoard),
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
            "buddy_mic_muted",
            view(DisplayMode::Buddy, |v| {
                v.mic.muted = Some(true);
                v.status.muted = Some(true);
                v.status.media = Some(MediaStatus::Playing);
                v.elapsed_ms = 600;
            }),
        ),
        (
            "buddy_mic_live",
            view(DisplayMode::Buddy, |v| {
                v.mic.muted = Some(false);
                v.mic.in_use = Some(true);
                v.status.muted = Some(true);
                v.elapsed_ms = 600;
            }),
        ),
        (
            "buddy_controls",
            view(DisplayMode::Buddy, |v| {
                v.status.clock = at(19, 52, 0);
                v.controls = controls("Volume", "Play/Pause", "Mute");
            }),
        ),
        (
            "buddy_buttons",
            view(DisplayMode::Buddy, |v| {
                v.status.clock = at(19, 52, 0);
                v.status.media = Some(MediaStatus::Playing);
                let mut c = controls("Volume", "Play/Pause", "Mute");
                if let Some(c) = c.as_mut() {
                    c.buttons = ["Previous", "", "Next track please"].map(MediaText::from_text);
                }
                v.controls = c;
            }),
        ),
        (
            "buddy_profile",
            view(DisplayMode::Buddy, |v| {
                v.status.clock = at(19, 52, 0);
                let mut c = controls("Tabs", "Play/Pause", "Mute");
                if let Some(c) = c.as_mut() {
                    c.buttons = ["Back", "Reload", "New tab"].map(MediaText::from_text);
                    c.profile = MediaText::from_text("Browser");
                }
                v.controls = c;
            }),
        ),
        (
            "buddy_pinned_switch_down",
            view(DisplayMode::Buddy, |v| {
                v.status.clock = at(8, 5, 0);
                let mut c = controls("Volume", "Play/Pause", "Mute");
                if let Some(c) = c.as_mut() {
                    c.buttons = ["Previous", "Play/Pause", "Next"].map(MediaText::from_text);
                    c.profile = MediaText::from_text("Media");
                    c.pinned = true;
                }
                v.controls = c;
                v.button_down = true;
                v.switch_down = true;
            }),
        ),
        ("buddy_hold_ring_30", ring(30)),
        ("buddy_hold_ring_70", ring(70)),
        ("buddy_hold_ring_ready", ring(100)),
        (
            "buddy_hold_ring_long_label",
            view(DisplayMode::Buddy, |v| {
                v.button_down = true;
                v.hold = Some(HoldRing {
                    control: 1,
                    percent: 45,
                    label: MediaText::from_text("Open Spotify Premium"),
                });
            }),
        ),
        ("picker_auto", picker(0, 0, "", PickerPhase::Choosing)),
        (
            "picker_pin1",
            picker(1, 0, "Claude Code", PickerPhase::Choosing),
        ),
        (
            "picker_pin2",
            picker(2, 1, "Photoshop", PickerPhase::Choosing),
        ),
        (
            "picker_long_name",
            picker(2, 2, "Adobe Premiere Pro 2026", PickerPhase::Choosing),
        ),
        (
            "picker_pin1_waiting",
            picker(1, 1, "Claude Code", PickerPhase::Waiting),
        ),
        (
            "picker_pin1_confirmed",
            picker(1, 1, "Claude Code", PickerPhase::Confirmed),
        ),
        (
            "picker_auto_confirmed",
            picker(0, 0, "", PickerPhase::Confirmed),
        ),
        (
            "picker_pin1_unconfirmed",
            picker(1, 1, "Claude Code", PickerPhase::Unconfirmed),
        ),
        (
            "picker_slide_mid",
            view(DisplayMode::Buddy, |v| {
                v.picker = Some(PickerView {
                    count: 3,
                    index: 2,
                    from: 1,
                    moved_ms: 60,
                    name: PickerName::from_text("Photoshop"),
                    phase: PickerPhase::Choosing,
                    phase_ms: 0,
                });
            }),
        ),
        (
            "buddy_controls_long_and_empty",
            view(DisplayMode::Buddy, |v| {
                v.controls = controls("Volume", "Open Spotify Premium", "");
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
