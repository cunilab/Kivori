//! The benchmark scenes: what the device composes in each representative situation. Shared by the
//! riscv bench (`main.rs`) and the host parity test (`../tests/native_parity.rs`), which renders the
//! same frames natively and must match the simulator's frame-buffer hashes bit for bit.
use kivori_model::desk::{
    ActionFeedback, ActionKind, ClockTime, ControlLabels, CpuHistory, DeskStatus, DeskView,
    DisplayMode, FeedbackKind, MediaInfo, MediaStatus, MediaText, MicState,
};
use kivori_model::presentation::{ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{
    CompanionState, MascotAction, MascotAnimator, MascotPersonality, MascotPose, Rgb565,
};

/// One device frame as `Runtime::step` would hand it to the renderer.
pub struct Frame {
    pub state: CompanionState,
    pub pose: MascotPose,
    pub overlay: Option<ValueDisplay>,
    pub view: DeskView,
}

pub struct Scenario {
    pub name: &'static str,
    pub frames: u32,
    /// Frame `i`: one device frame as `Runtime::step` would hand it to the renderer.
    pub build: fn(u32) -> Frame,
}

/// Device time of the first frame of every scenario; a settled idle mascot after boot.
const T0: u32 = 20_000;

fn text(s: &str) -> MediaText {
    MediaText::from_text(s)
}

fn status(mode: DisplayMode, t: u32) -> DeskStatus {
    DeskStatus {
        mode,
        clock: Some(
            ClockTime {
                hour: 14,
                minute: 32,
                second: 10,
            }
            .advanced_by(t),
        ),
        volume_percent: Some(40),
        muted: Some(false),
        media: Some(MediaStatus::Playing),
        cpu_percent: Some(37),
        ram_percent: Some(62),
        high_load: false,
    }
}

fn labels() -> ControlLabels {
    ControlLabels {
        rotate: text("Volume"),
        press: text("Play/Pause"),
        hold: text("Mute"),
        profile: text("Work"),
        ..ControlLabels::default()
    }
}

fn media() -> MediaInfo {
    MediaInfo {
        title: text("Everything In Its Right Place"),
        artist: text("Radiohead"),
    }
}

fn view(mode: DisplayMode, t: u32) -> DeskView {
    DeskView {
        status: status(mode, t),
        elapsed_ms: t,
        media_info: Some(media()),
        controls: Some(labels()),
        mic: MicState {
            muted: Some(false),
            in_use: Some(false),
        },
        ..DeskView::default()
    }
}

fn idle_pose(t: u32) -> MascotPose {
    MascotAnimator::new(CompanionState::Idle, 0).pose_at(t)
}

fn idle_mascot(i: u32) -> Frame {
    let t = T0 + i * 250;
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: DeskView {
            elapsed_ms: t,
            ..DeskView::default()
        },
    }
}

/// What the app shows once connected: the buddy plus clock, indicators and control legend.
fn connected_desk(i: u32) -> Frame {
    let t = T0 + i * 250;
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: view(DisplayMode::Buddy, t),
    }
}

fn media_view(i: u32) -> Frame {
    let t = T0 + i * 500;
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: view(DisplayMode::Media, t),
    }
}

fn system_view(i: u32) -> Frame {
    let t = T0 + i * 500;
    let mut v = view(DisplayMode::System, t);
    for n in 0..CpuHistory::CAPACITY as u32 {
        v.cpu_history.push((20 + (n * 37) % 60) as u8);
    }
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: v,
    }
}

fn clock_view(i: u32) -> Frame {
    let t = T0 + i * 500;
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: view(DisplayMode::Clock, t),
    }
}

/// A busy buddy frame: a state change and a reaction in flight, a volume overlay shrinking the
/// mascot, an action badge, a live microphone, a pressed knob and the legend.
fn buddy_overlay(i: u32) -> Frame {
    let t = T0 + i * 100;
    let mut animator = MascotAnimator::new(CompanionState::Idle, 0);
    animator.set_state(CompanionState::Happy, T0 + 400);
    animator.trigger_action(
        MascotAction::Tickle,
        MascotPersonality::Playful,
        7,
        T0 + 400,
    );
    let mut v = view(DisplayMode::Buddy, t);
    v.feedback = Some(ActionFeedback {
        action: ActionKind::Volume,
        kind: FeedbackKind::StateConfirmed,
    });
    v.button_down = true;
    v.switch_down = true;
    v.status.muted = Some(true);
    v.mic = MicState {
        muted: Some(false),
        in_use: Some(true),
    };
    Frame {
        state: CompanionState::Happy,
        pose: animator.pose_at(t),
        overlay: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 63,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        }),
        view: v,
    }
}

/// The heaviest frame the device draws: the view-switch slide with the buddy (and its mascot)
/// scrolling out while the System view (gauges and the sparkline) scrolls in, so both are drawn.
fn switch_buddy_system(i: u32) -> Frame {
    let t = T0 + i * 40;
    let mut v = system_view(0).view;
    v.elapsed_ms = t;
    v.previous_mode = Some(DisplayMode::Buddy);
    v.mode_age_ms = i * 40;
    Frame {
        state: CompanionState::Idle,
        pose: idle_pose(t),
        overlay: None,
        view: v,
    }
}

pub static SCENARIOS: [Scenario; 7] = [
    Scenario {
        name: "idle_mascot",
        frames: 24,
        build: idle_mascot,
    },
    Scenario {
        name: "connected_desk",
        frames: 24,
        build: connected_desk,
    },
    Scenario {
        name: "media_view",
        frames: 8,
        build: media_view,
    },
    Scenario {
        name: "system_view",
        frames: 8,
        build: system_view,
    },
    Scenario {
        name: "clock_view",
        frames: 8,
        build: clock_view,
    },
    Scenario {
        name: "buddy_overlay",
        frames: 24,
        build: buddy_overlay,
    },
    Scenario {
        name: "switch_buddy_system",
        frames: 8,
        build: switch_buddy_system,
    },
];

/// FNV-1a over the frame buffer, two pixels per round: the checksum the bench reports per frame.
pub fn frame_hash(frame: &[Rgb565]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for pair in frame.as_chunks::<2>().0 {
        hash ^= u32::from(pair[0].raw()) | (u32::from(pair[1].raw()) << 16);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}
