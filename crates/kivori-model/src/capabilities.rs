//! Capability bitset for additive, minor-version feature negotiation (data-model §3).
//!
//! Concrete capability flags are allocated centrally as associated constants below (see the bit
//! registry comment in `impl Capabilities`) and never reused once retired. The representation is an
//! opaque `u32` bitset; unknown/higher bits are preserved and ignored by older peers (append-only
//! forward-compatibility). The key negotiation operation is [`Capabilities::intersection`] (features
//! advertised by both peers).

use serde::{Deserialize, Serialize};

/// A set of protocol capability flags, represented as an opaque `u32` bitset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Capabilities(u32);

impl Capabilities {
    /// The empty capability set.
    pub const NONE: Capabilities = Capabilities(0);
    // CAPABILITY BIT REGISTRY — allocate centrally, never reuse a retired bit.
    //   bit 0  MASCOT_INTERACTION  mascot animation (PR #3)
    //   bit 1  PHYSICAL_INPUT_V1   Slice 002
    //   bit 2  PRESENTATION_V1     Slice 002
    //   bit 3  BUTTON_INPUT_V1     M1 push switch (Press / Hold input events)
    //   bit 4  DESK_STATUS_V1      M1 `Status` message (display mode + monitoring)
    //   bit 5  ACTION_FEEDBACK_V1  M1 `Feedback` message (action outcome)
    //   bit 6  DOUBLE_PRESS_V1     M1.1 push-switch `DoublePress` input (next view)
    //   bit 7  MEDIA_INFO_V1       M1.1 `MediaInfo` message (now-playing title / artist)
    //   bit 8  CONTROL_LABELS_V1   `ControlLabels` message (what the knob / press / hold do)
    //   bit 9  CONTEXT_BUTTONS_V1  three contextual buttons (`ControlId::ContextButton` input)
    //   bit 10 HOST_TAKEOVERS_V1   `Bye(HostSleeping | FirmwareUpdate)` takeovers and the host-silence timeout
    //   bit 11 QUIET_ACK_V1        `FeedbackKind::Triggered` quiet acknowledgement for trigger-only outcomes
    //   bit 12 DEVICE_INTERNALS_V1 `InternalsRequest` / `Internals`: the developer-mode device internals snapshot
    //   bit 13 IDLE_LIFE_V1        `BuddySettings`: the device runs the buddy's idle life itself; carries brightness
    //   bit 14 MIC_STATE_V1        `MicStateUpdate`: whether every microphone is muted, and whether one is in use

    /// Bit 0 — device accepts deterministic social mascot actions and returns applied-time
    /// acknowledgments.
    pub const MASCOT_INTERACTION: Capabilities = Capabilities(1 << 0);

    /// Bit 1 — the device may emit `InputEvent` (Slice 002, rotary input).
    pub const PHYSICAL_INPUT_V1: Capabilities = Capabilities(1 << 1);

    /// Bit 2 — the device renders semantic `Presentation` (Slice 002).
    pub const PRESENTATION_V1: Capabilities = Capabilities(1 << 2);

    /// Bit 3 — the device may emit push-switch `InputEvent`s (`ControlId::Button`, M1).
    pub const BUTTON_INPUT_V1: Capabilities = Capabilities(1 << 3);

    /// Bit 4 — the device renders the `Status` message: display modes and monitoring (M1).
    pub const DESK_STATUS_V1: Capabilities = Capabilities(1 << 4);

    /// Bit 5 — the device renders the `Feedback` message: action outcomes (M1).
    pub const ACTION_FEEDBACK_V1: Capabilities = Capabilities(1 << 5);

    /// Bit 6 — the device detects double presses and sends `InputKind::DoublePress`. Without it the
    /// device never waits for a second press, so a single press fires immediately.
    pub const DOUBLE_PRESS_V1: Capabilities = Capabilities(1 << 6);

    /// Bit 7 — the device renders the `MediaInfo` message (now-playing title and artist).
    pub const MEDIA_INFO_V1: Capabilities = Capabilities(1 << 7);

    /// Bit 8 — the device renders the `ControlLabels` message (what each control does).
    pub const CONTROL_LABELS_V1: Capabilities = Capabilities(1 << 8);

    /// Bit 9 — the device has the three contextual buttons and may emit
    /// `ControlId::ContextButton` Press / Hold input events.
    pub const CONTEXT_BUTTONS_V1: Capabilities = Capabilities(1 << 9);

    /// Bit 10 — the device honours `Bye(HostSleeping)` (keeps showing Sleeping after the link drops)
    /// and `Bye(FirmwareUpdate)` (shows Updating), and goes offline after 4 s of host silence.
    pub const HOST_TAKEOVERS_V1: Capabilities = Capabilities(1 << 10);

    /// Bit 11 — the device renders `FeedbackKind::Triggered` as a quiet acknowledgement (keycap
    /// press and label pulse, no badge). Without it the desktop sends `Unverified` instead.
    pub const QUIET_ACK_V1: Capabilities = Capabilities(1 << 11);

    /// Bit 12 — the device answers `InternalsRequest` with one `Internals` snapshot (flash, RAM,
    /// CPU, chip and link counters) for the desktop's developer mode.
    pub const DEVICE_INTERNALS_V1: Capabilities = Capabilities(1 << 12);

    /// Bit 13 — the device runs the buddy's idle life itself (self-play, glances, looks, evening
    /// blinks) and accepts `BuddySettings` (personality, reactions on/off, brightness). Without it
    /// the desktop director keeps scheduling self-play and sends no settings.
    pub const IDLE_LIFE_V1: Capabilities = Capabilities(1 << 13);

    /// Bit 14 — the device renders `MicStateUpdate`: the mic indicator (muted or live) in the top
    /// indicator slot and the covered-mouth buddy face. Without it the desktop sends no mic state.
    pub const MIC_STATE_V1: Capabilities = Capabilities(1 << 14);

    /// Creates a capability set from a raw bitmask.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Capabilities(bits)
    }

    /// Returns the raw bitmask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` if the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if every flag in `other` is present in `self`.
    #[must_use]
    pub const fn contains(self, other: Capabilities) -> bool {
        (self.0 & other.0) == other.0
    }

    /// The negotiated set: flags advertised by **both** peers (bitwise intersection).
    #[must_use]
    pub const fn intersection(self, other: Capabilities) -> Capabilities {
        Capabilities(self.0 & other.0)
    }

    /// The union of two capability sets (bitwise or).
    #[must_use]
    pub const fn union(self, other: Capabilities) -> Capabilities {
        Capabilities(self.0 | other.0)
    }
}
