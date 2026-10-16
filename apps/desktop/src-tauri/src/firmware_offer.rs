//! The firmware update offer (ADR-0015, step 6; product.md, "Firmware updates (beta)"). Pure: no
//! clock, no Tauri. The device thread feeds [`OfferTracker::evaluate`] what it knows each tick and
//! publishes the answer; "Update now" runs the ordinary flash. Nothing here ever flashes
//! (invariant 58).
//!
//! - Only the firmware bundled for the device's own product and board is offered, and only when it
//!   is newer than the device's. A device the catalog does not know, or whose identity has not
//!   arrived yet, is offered nothing; the flashing guard (`wrongProduct`) stays authoritative.
//! - The offer appears once the device has been connected and idle (no knob gesture, no flash) for
//!   [`IDLE_BEFORE_OFFER`]. Once shown, a gesture does not take it away again.
//! - It is keyed by device hash, bundled firmware version and connection. "Later" hides it until
//!   that device connects again. The memory is per run on purpose: a restart is a new connect, so
//!   "Later" is never a permanent skip.
//! - While an app update is offered, downloading or installing, the firmware offer waits
//!   (`deferredForAppUpdate`): the new app may bundle newer firmware. The app update goes first.
//! - A device that cannot work with this app (an older protocol major, or a capability the catalog
//!   requires missing) gets a required update; one newer than the app is told to update the app.

use std::time::Duration;

use crate::firmware::{update_advice, UpdateAdvice};
use crate::firmware_marker::ImageIdentity;
use crate::identity::DeviceIdentity;

/// How long a connected device must be idle before the offer appears.
pub const IDLE_BEFORE_OFFER: Duration = Duration::from_secs(5);

/// Why an update is required rather than optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredReason {
    /// The device speaks an older protocol major than this app supports.
    ProtocolTooOld,
    /// The device lacks a capability the catalog requires of its product.
    MissingCapabilities,
}

impl RequiredReason {
    /// Every reason, in declaration order.
    pub const ALL: [Self; 2] = [Self::ProtocolTooOld, Self::MissingCapabilities];

    /// The token the webview receives.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ProtocolTooOld => "protocolTooOld",
            Self::MissingCapabilities => "missingCapabilities",
        }
    }
}

/// What the app says about the connected device's firmware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirmwareOffer {
    /// Nothing to say.
    None,
    /// Newer firmware for this device is bundled: "Update now" or "Later".
    OfferOptional {
        /// The device's firmware version.
        from: String,
        /// The bundled version.
        to: String,
    },
    /// The device cannot work with this app until it is updated: "Update now" or "Not now".
    Required {
        /// Why.
        reason: RequiredReason,
    },
    /// The device runs newer firmware than this app understands or bundles: update the app.
    UpdateApp,
    /// An update would be offered, but an app update comes first.
    DeferredForAppUpdate,
}

impl FirmwareOffer {
    /// Every kind token, in declaration order (shared with the webview).
    pub const TOKENS: [&'static str; 5] = [
        "none",
        "offerOptional",
        "required",
        "updateApp",
        "deferredForAppUpdate",
    ];

    /// The kind token the webview receives.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OfferOptional { .. } => "offerOptional",
            Self::Required { .. } => "required",
            Self::UpdateApp => "updateApp",
            Self::DeferredForAppUpdate => "deferredForAppUpdate",
        }
    }

    /// Whether this is a dialog the user answers ("Update now" applies).
    #[must_use]
    pub const fn asks(&self) -> bool {
        matches!(self, Self::OfferOptional { .. } | Self::Required { .. })
    }
}

/// What the device thread knows about the device right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferDevice {
    /// Nothing usable is connected (disconnected, connecting, reconnecting).
    Absent,
    /// A completed handshake.
    Connected {
        /// Short device-identity hash.
        hash: String,
        /// The device's firmware version (`major.minor.patch`).
        firmware: String,
        /// Which product and board it is; `None` while a device that reports one has not yet.
        identity: Option<DeviceIdentity>,
        /// It lacks a capability the catalog requires of its product.
        missing_capabilities: bool,
    },
    /// The handshake found a protocol major this app does not speak (no identity, no version).
    Incompatible {
        /// The major the device reported, if known.
        device_major: Option<u16>,
    },
}

/// One evaluation's inputs.
#[derive(Debug, Clone, Copy)]
pub struct OfferInputs<'a> {
    pub device: &'a OfferDevice,
    /// The connection generation: it changes on every new connect.
    pub generation: u32,
    /// Exact image for the connected identity; never inferred for an incompatible device.
    pub image: Option<&'a ImageIdentity>,
    /// Recovery can ask the user to resolve a target when this build has any images.
    pub restorable: bool,
    /// The protocol major this app speaks.
    pub host_major: u16,
    /// An app update is offered, downloading, installing or held for an idle device.
    pub app_update_due: bool,
    /// A firmware flash or restore is running.
    pub flashing: bool,
    /// A knob gesture is open.
    pub gesture: bool,
}

/// Which offer "Later" applies to: one device, one bundled version, one connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferKey {
    /// Short device hash; `None` for an incompatible device (it has none yet).
    pub hash: Option<String>,
    /// The bundled firmware version.
    pub version: Option<String>,
    /// The connection generation.
    pub generation: u32,
}

impl OfferKey {
    /// An opaque token the webview uses to open the dialog by itself once per offer.
    #[must_use]
    pub fn token(&self) -> String {
        format!(
            "{}:{}:{}",
            self.generation,
            self.hash.as_deref().unwrap_or(""),
            self.version.as_deref().unwrap_or("")
        )
    }
}

/// A decision and the key it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub offer: FirmwareOffer,
    /// Set with [`FirmwareOffer::OfferOptional`] and [`FirmwareOffer::Required`].
    pub key: Option<OfferKey>,
}

impl Decision {
    const fn plain(offer: FirmwareOffer) -> Self {
        Self { offer, key: None }
    }
}

/// The decision table. `settled` is whether the device has been idle long enough in this
/// connection; `later` is the offer the user answered "Later" to, if any.
#[must_use]
pub fn decide(inputs: &OfferInputs<'_>, settled: bool, later: Option<&OfferKey>) -> Decision {
    if inputs.flashing {
        return Decision::plain(FirmwareOffer::None);
    }
    match inputs.device {
        OfferDevice::Absent => Decision::plain(FirmwareOffer::None),
        OfferDevice::Incompatible { device_major } => {
            if device_major.is_some_and(|major| major > inputs.host_major) {
                return Decision::plain(FirmwareOffer::UpdateApp);
            }
            if !inputs.restorable {
                return Decision::plain(FirmwareOffer::None);
            }
            if inputs.app_update_due {
                return Decision::plain(FirmwareOffer::DeferredForAppUpdate);
            }
            if !settled {
                return Decision::plain(FirmwareOffer::None);
            }
            // No identity without a handshake: the restore flow picks the product from the port.
            Decision {
                offer: FirmwareOffer::Required {
                    reason: RequiredReason::ProtocolTooOld,
                },
                key: Some(OfferKey {
                    hash: None,
                    version: None,
                    generation: inputs.generation,
                }),
            }
        }
        OfferDevice::Connected {
            hash,
            firmware,
            identity,
            missing_capabilities,
        } => {
            // Only the image made for this device's product and board counts; anything else
            // (another product, an unknown one, an identity not yet reported) is offered nothing.
            let image = identity.and_then(|identity| {
                inputs.image.filter(|image| {
                    (image.product, image.board) == (identity.product, identity.board)
                })
            });
            let Some(image) = image else {
                return Decision::plain(FirmwareOffer::None);
            };
            match update_advice(Some(firmware.as_str()), Some(image.version.as_str())) {
                UpdateAdvice::DeviceNewer => Decision::plain(FirmwareOffer::UpdateApp),
                UpdateAdvice::UpToDate | UpdateAdvice::Unknown => {
                    Decision::plain(FirmwareOffer::None)
                }
                UpdateAdvice::UpdateAvailable => {
                    if inputs.app_update_due {
                        return Decision::plain(FirmwareOffer::DeferredForAppUpdate);
                    }
                    if !settled {
                        return Decision::plain(FirmwareOffer::None);
                    }
                    let key = OfferKey {
                        hash: Some(hash.clone()),
                        version: Some(image.version.clone()),
                        generation: inputs.generation,
                    };
                    if *missing_capabilities {
                        return Decision {
                            offer: FirmwareOffer::Required {
                                reason: RequiredReason::MissingCapabilities,
                            },
                            key: Some(key),
                        };
                    }
                    if later == Some(&key) {
                        return Decision::plain(FirmwareOffer::None);
                    }
                    Decision {
                        offer: FirmwareOffer::OfferOptional {
                            from: firmware.clone(),
                            to: image.version.clone(),
                        },
                        key: Some(key),
                    }
                }
            }
        }
    }
}

/// What one evaluation changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evaluation {
    /// The published decision changed: emit it and refresh the tray.
    pub changed: bool,
    /// An offer the user answers appeared for the first time this connection (activity).
    pub shown: bool,
}

/// Why a user action on the offer was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// There is no offer to answer.
    NothingOffered,
}

impl Refused {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NothingOffered => "There is no firmware update to install.",
        }
    }
}

/// The idle timer, the "Later" memory and the published decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferTracker {
    /// The connection generation and when its current idle stretch began.
    quiet_since: Option<(u32, Duration)>,
    /// The generation whose idle wait has passed.
    settled: Option<u32>,
    later: Option<OfferKey>,
    /// The last offer recorded as shown.
    announced: Option<OfferKey>,
    current: Decision,
}

impl Default for OfferTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl OfferTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            quiet_since: None,
            settled: None,
            later: None,
            announced: None,
            current: Decision::plain(FirmwareOffer::None),
        }
    }

    /// The decision currently published.
    #[must_use]
    pub const fn current(&self) -> &Decision {
        &self.current
    }

    /// Advances the idle timer to `now` (the device thread's monotonic clock) and decides again.
    pub fn evaluate(&mut self, now: Duration, inputs: &OfferInputs<'_>) -> Evaluation {
        let present = !matches!(inputs.device, OfferDevice::Absent);
        let generation = inputs.generation;
        if !present {
            self.quiet_since = None;
            self.settled = None;
        } else if self.settled != Some(generation) {
            if self.quiet_since.is_some_and(|(g, _)| g != generation) {
                self.quiet_since = None;
            }
            if inputs.flashing || inputs.gesture {
                self.quiet_since = None;
            } else {
                let (_, since) = *self.quiet_since.get_or_insert((generation, now));
                if now.saturating_sub(since) >= IDLE_BEFORE_OFFER {
                    self.settled = Some(generation);
                }
            }
        }
        let next = decide(
            inputs,
            self.settled == Some(generation),
            self.later.as_ref(),
        );
        let shown = next.offer.asks() && next.key.is_some() && self.announced != next.key;
        if shown {
            self.announced.clone_from(&next.key);
        }
        let changed = next != self.current;
        self.current = next;
        Evaluation { changed, shown }
    }

    /// "Update now": allowed while an offer (optional or required) is shown.
    ///
    /// # Errors
    /// [`Refused::NothingOffered`] otherwise.
    pub fn accept(&self) -> Result<(), Refused> {
        if self.current.offer.asks() {
            Ok(())
        } else {
            Err(Refused::NothingOffered)
        }
    }

    /// "Later" on an optional offer: hidden until this device connects again. A required update
    /// has no "Later" ("Not now" only closes its dialog), so it is refused.
    ///
    /// # Errors
    /// [`Refused::NothingOffered`] when no optional offer is shown.
    pub fn later(&mut self) -> Result<(), Refused> {
        let FirmwareOffer::OfferOptional { .. } = self.current.offer else {
            return Err(Refused::NothingOffered);
        };
        self.later = self.current.key.take();
        self.current = Decision::plain(FirmwareOffer::None);
        Ok(())
    }
}
