//! S5.4: real desktop policies with fake OS services; only labelled raw cases traverse firmware.
use super::*;
use crate::config::{
    ActionSpec, ConfigStore, MacroSpec, ProfileId, ProfileRef, SlotSpec, StepSpec,
};
use crate::desk::actions::{Action, Macro, Outcome, Step};
use crate::platform::{
    ActionAvailability, ActionError, FakeForeground, FakeMediaObserver, FakeMicBackend,
    FakeMicQuirk, Foreground,
};
use kivori_model::desk::{FeedbackKind, MediaStatus};

fn check(ok: bool, message: &str) -> Result<(), String> {
    ok.then_some(()).ok_or_else(|| message.into())
}

struct Rig {
    sandbox: Sandbox,
    mic: Arc<FakeMicBackend>,
    media: Arc<FakeMediaObserver>,
    foreground: Arc<FakeForeground>,
}

impl Rig {
    fn new(
        executable: &Path,
        volume: FakeVolumeBackend,
        result: Result<(), ActionError>,
    ) -> Result<Self, String> {
        let mic = Arc::new(FakeMicBackend::new());
        let media = Arc::new(FakeMediaObserver::default());
        let foreground = Arc::new(FakeForeground::default());
        let sandbox = Sandbox::open_with(
            executable,
            &[],
            Arc::new(volume),
            Arc::new(FakeInputSynth::new(result)),
            mic.clone(),
            media.clone(),
            foreground.clone(),
        )
        .map_err(|e| e.to_string())?;
        let mut rig = Self {
            sandbox,
            mic,
            media,
            foreground,
        };
        rig.until("handshake and firmware capture", |s| {
            s.snapshot.connected && s.snapshot.frame.is_some()
        })?;
        // Cases request captures explicitly; ordinary action polling need not serialize a full panel.
        rig.sandbox.next_capture = Duration::MAX;
        Ok(rig)
    }

    fn until(&mut self, name: &str, ready: impl Fn(&Sandbox) -> bool) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.sandbox.tick();
            if let Some(error) = &self.sandbox.snapshot.error {
                return Err(format!("{name}: {error}"));
            }
            if ready(&self.sandbox) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!("{name}: bounded wait expired"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn action(&mut self, action: Action) -> Result<Outcome, String> {
        let (reply, mut response) = tauri::async_runtime::channel(1);
        self.sandbox
            .desk
            .test(action, self.sandbox.started.elapsed(), &mut |_| {}, reply);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.sandbox.tick();
            if let Some(error) = &self.sandbox.snapshot.error {
                return Err(error.clone());
            }
            if let Ok(outcome) = response.try_recv() {
                return Ok(outcome);
            }
            if Instant::now() >= deadline {
                return Err("desktop action reply timed out".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn sent(&self) -> Vec<String> {
        self.sandbox.synth.sent.lock().expect("fake synth").clone()
    }

    fn apply(&mut self, store: &ConfigStore) {
        self.sandbox
            .desk
            .apply_config(&store.resolved(), &mut |_| {});
    }

    fn raw_button(&mut self, mask: u8, duration: u32) {
        let now = self.sandbox.started.elapsed().as_millis() as u32;
        self.sandbox.edges.extend([
            Edge {
                at_ms: now + 1,
                levels: mask,
            },
            Edge {
                at_ms: now + duration + 1,
                levels: 0,
            },
        ]);
    }

    fn focus(&self, id: &str) {
        *self.foreground.0.lock().expect("fake foreground") = Foreground::App {
            id: id.into(),
            name: id.into(),
        };
    }
}

fn bind(store: &mut ConfigStore, action: ActionSpec) -> Result<(), String> {
    let next = store.edited(|f| {
        f.profiles.entry(ProfileId::General).or_default().press = Some(SlotSpec {
            action: Some(action),
            label: None,
        });
    });
    store.save(next).map_err(|e| format!("save binding: {e:?}"))
}

fn persistence_and_profiles(executable: &Path, config_dir: &Path) -> Result<(), String> {
    let mut store = ConfigStore::open(config_dir);
    check(
        !store.file().onboarding.completed,
        "new isolated setup was already complete",
    )?;
    bind(&mut store, ActionSpec::NextTrack)?;
    let next = store.edited(|f| f.onboarding.completed = true);
    store.save(next).map_err(|e| format!("setup save: {e:?}"))?;
    drop(store);
    let mut store = ConfigStore::open(config_dir);
    check(
        store.notice().is_none() && store.file().onboarding.completed,
        "isolated setup did not survive reopen",
    )?;
    let mut rig = Rig::new(executable, FakeVolumeBackend::new(50), Ok(()))?;
    rig.apply(&store);
    rig.sandbox.input("tap")?;
    rig.until("persisted raw press binding", |s| {
        !s.synth.sent.lock().expect("fake synth").is_empty()
    })?;
    check(
        rig.sent() == ["media-next"],
        "saved binding was not used by the firmware-origin press",
    )?;
    println!("PASS S5.4 firmware-session: saved/reopened binding executes exactly once; isolated setup persists");

    rig.focus("code.exe");
    rig.until("automatic Code profile", |s| {
        s.desk.context().profile().id == ProfileRef::Builtin(ProfileId::Code)
    })?;
    rig.raw_button(16, 600);
    rig.until("raw middle hold pins General", |s| {
        s.desk.context().pinned()
            && s.desk.context().profile().id == ProfileRef::Builtin(ProfileId::General)
    })?;
    rig.focus("zoom.exe");
    // A correlated system action commits the observed focus while preserving the pin.
    check(
        rig.action(Action::PreviousTrack)?.kind == FeedbackKind::Triggered,
        "pinned system action failed",
    )?;
    check(
        rig.sandbox.desk.context().pinned()
            && rig.sandbox.desk.context().profile().id == ProfileRef::Builtin(ProfileId::General),
        "focus overrode the pin",
    )?;
    bind(
        &mut store,
        ActionSpec::Shortcut {
            keys: "Ctrl+M".into(),
        },
    )?;
    rig.apply(&store);
    *rig.foreground.0.lock().expect("fake foreground") = Foreground::Protected;
    let before = rig.sent();
    rig.sandbox.input("tap")?;
    rig.until("Protected firmware-origin shortcut refusal", |s| {
        s.desk.last_action().is_some_and(|action| {
            action.kind == FeedbackKind::Error
                && action.action == crate::desk::catalog::ActionToken::Shortcut
        })
    })?;
    check(
        rig.sent() == before,
        "Protected raw press injected custom input",
    )?;
    let refused = rig.action(Action::Shortcut(
        "Ctrl+M".parse().map_err(|e| format!("shortcut: {e:?}"))?,
    ))?;
    check(
        refused.kind == FeedbackKind::Error
            && rig.sent() == before
            && rig.sandbox.desk.context().protected(),
        "Protected admitted custom input",
    )?;
    check(
        rig.action(Action::PlayPause)?.kind == FeedbackKind::Triggered
            && rig.sent().last().is_some_and(|s| s == "media-play-pause"),
        "Protected blocked system media",
    )?;
    check(
        rig.action(Action::Launch("Calculator".into()))?.kind == FeedbackKind::Error,
        "simulator launched an app",
    )?;
    *rig.foreground.0.lock().expect("fake foreground") = Foreground::Unknown;
    check(
        rig.action(Action::Launch("Calculator".into()))?.kind == FeedbackKind::Error,
        "isolated launcher was not disabled",
    )?;
    println!("PASS S5.4 firmware-session + desktop-action: focus/pin, Protected refusal/system allowance and disabled launch");

    let original_bytes =
        std::fs::read(config_dir.join("config.json")).map_err(|e| e.to_string())?;
    let invalid = store.edited(|f| {
        f.profiles.entry(ProfileId::General).or_default().press = Some(SlotSpec {
            action: Some(ActionSpec::NextTrack),
            label: Some("x".repeat(33)),
        })
    });
    check(store.save(invalid).is_err(), "invalid settings were saved")?;
    check(
        std::fs::read(config_dir.join("config.json")).map_err(|e| e.to_string())? == original_bytes,
        "rejected save changed persisted bytes",
    )?;
    drop(store);
    let reopened = ConfigStore::open(config_dir);
    check(
        reopened.file().onboarding.completed
            && matches!(
                reopened.file().profiles[&ProfileId::General]
                    .press
                    .as_ref()
                    .and_then(|s| s.action.as_ref()),
                Some(ActionSpec::Shortcut { .. })
            ),
        "rejected save replaced prior settings",
    )?;
    let mut detached = ConfigStore::detached();
    check(
        detached
            .save(detached.edited(|f| f.onboarding.completed = true))
            .is_err()
            && !detached.file().onboarding.completed,
        "unavailable setup save claimed completion",
    )?;
    println!("PASS S5.4 config-store: rejected save preserves prior bytes; unavailable setup save stays incomplete");
    Ok(())
}

fn microphones(executable: &Path) -> Result<(), String> {
    let mut rig = Rig::new(executable, FakeVolumeBackend::new(50), Ok(()))?;
    check(
        rig.sandbox.desk.mic().0.muted.is_none(),
        "empty microphone coverage claimed muted",
    )?;
    rig.mic
        .add_device("simulated microphone", FakeMicQuirk::Works);
    rig.mic.set_in_use(Some(true));
    rig.until("observed live microphone", |s| s.desk.mic().0.is_live())?;
    check(
        rig.action(Action::ToggleMicMute)?.kind == FeedbackKind::StateConfirmed,
        "healthy mic mute did not confirm observed state",
    )?;
    rig.until("all microphones muted", |s| {
        s.desk.mic().0.muted == Some(true)
    })?;
    rig.mic
        .add_device("simulated healthy hotplug", FakeMicQuirk::Works);
    rig.until("healthy hotplug inherits mute", |s| {
        s.desk.mic().1.len() == 2 && s.desk.mic().0.muted == Some(true)
    })?;
    rig.mic
        .add_device("simulated hotplug failure", FakeMicQuirk::StaysLive);
    rig.until("failed hotplug remains live", |s| {
        s.desk.mic().0.muted == Some(false)
    })?;
    check(
        rig.action(Action::ToggleMicMute)?.kind == FeedbackKind::Error,
        "failed hotplug write was confirmed",
    )?;
    println!("PASS S5.4 desktop-action: unknown microphone coverage, healthy mute, hotplug failure and truthful live state");
    drop(rig);
    for (quirk, expected) in [
        (FakeMicQuirk::Unreadable, FeedbackKind::Unverified),
        (FakeMicQuirk::Unsettable, FeedbackKind::Error),
    ] {
        let mut rig = Rig::new(executable, FakeVolumeBackend::new(50), Ok(()))?;
        rig.mic.add_device("simulated incomplete coverage", quirk);
        let outcome = rig.action(Action::ToggleMicMute)?;
        check(
            outcome.kind == expected && rig.sandbox.desk.mic().0.muted != Some(true),
            "incomplete microphone coverage was falsely confirmed",
        )?;
        println!("PASS S5.4 desktop-action: mic {quirk:?} produces {expected:?}, never all-muted confirmation");
    }
    Ok(())
}

fn volume_and_media(executable: &Path) -> Result<(), String> {
    let mut rig = Rig::new(executable, FakeVolumeBackend::quantised(55, 10), Ok(()))?;
    check(
        rig.sandbox.snapshot.volume == 55 && rig.sandbox.volume.read() == Ok(55),
        "quantized volume scenario did not start at55",
    )?;
    rig.sandbox.input("clockwise")?;
    rig.until("quantized detent", |s| s.snapshot.detents == 1)?;
    check(
        rig.sandbox.snapshot.volume == 50 && rig.sandbox.volume.read() == Ok(50),
        "volume asserted requested57 instead of observed50",
    )?;
    rig.sandbox.volume.external_change(63);
    rig.until("external volume read-back", |s| {
        s.desk.observed().volume_percent == Some(63)
    })?;
    println!("PASS S5.4 firmware-session + desktop observation: quantized detent uses observed50; external63 updates sampled status");

    drop(rig);
    let mut rig = Rig::new(
        executable,
        FakeVolumeBackend::unreadable_after_write(50),
        Ok(()),
    )?;
    check(
        rig.action(Action::ToggleMute)?.kind == FeedbackKind::Unverified,
        "unreadable volume write was confirmed",
    )?;
    drop(rig);
    let mut rig = Rig::new(
        executable,
        FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
            reason: "simulated missing endpoint".into(),
        }),
        Ok(()),
    )?;
    check(
        rig.action(Action::ToggleMute)?.kind == FeedbackKind::Error
            && rig.sandbox.desk.observed().volume_percent.is_none(),
        "missing endpoint asserted a volume/state confirmation",
    )?;
    println!("PASS S5.4 desktop-action: volume read-back failure is Unverified; missing endpoint is unknown/Error");

    drop(rig);
    for (result, expected) in [
        (Ok(()), FeedbackKind::Triggered),
        (
            Err(ActionError::NotImplemented {
                target: "simulated",
            }),
            FeedbackKind::Error,
        ),
        (Err(ActionError::PermissionRequired), FeedbackKind::Error),
        (Err(ActionError::Uncertain), FeedbackKind::Unverified),
    ] {
        let mut rig = Rig::new(executable, FakeVolumeBackend::new(50), result.clone())?;
        let outcome = rig.action(Action::PlayPause)?;
        check(
            outcome.kind == expected
                && outcome.permission_required
                    == matches!(result, Err(ActionError::PermissionRequired))
                && rig.sandbox.desk.observed().media.is_none(),
            "media dispatch fabricated observed playback",
        )?;
        *rig.media.status.lock().expect("fake media") = Some(MediaStatus::Playing);
        rig.until("observed media playback", |s| {
            s.desk.observed().media == Some(MediaStatus::Playing)
        })?;
        check(
            rig.sandbox.desk.last_action().is_some_and(|action| {
                action.action == crate::desk::catalog::ActionToken::PlayPause
                    && action.kind == expected
            }),
            "observed media upgraded an unrelated action",
        )?;
        if matches!(result, Err(ActionError::Uncertain)) {
            let before = rig.sent().len();
            let action = Action::Macro(Arc::new(Macro {
                id: "uncertain".into(),
                name: "Uncertain".into(),
                steps: vec![
                    Step::Run(Action::PreviousTrack),
                    Step::Run(Action::NextTrack),
                ],
            }));
            check(
                rig.action(action)?.kind == FeedbackKind::Unverified
                    && rig.sent().len() == before + 1
                    && rig.sent().last().is_some_and(|s| s == "media-previous"),
                "uncertain macro input continued into another step",
            )?;
        }
        println!("PASS S5.4 desktop-action: media {result:?} => {expected:?}; playback changes only from observer");
    }
    Ok(())
}

fn macros(executable: &Path, config_dir: &Path) -> Result<(), String> {
    let mut store = ConfigStore::open(config_dir);
    store
        .save_macro(MacroSpec {
            id: "ordered".into(),
            name: "Ordered".into(),
            steps: vec![
                StepSpec::Action {
                    action: ActionSpec::PreviousTrack,
                },
                StepSpec::Delay { ms: 100 },
                StepSpec::Action {
                    action: ActionSpec::NextTrack,
                },
            ],
        })
        .map_err(|e| format!("macro save: {e:?}"))?;
    bind(
        &mut store,
        ActionSpec::Macro {
            id: "ordered".into(),
        },
    )?;
    drop(store);
    let mut store = ConfigStore::open(config_dir);
    let mut rig = Rig::new(executable, FakeVolumeBackend::new(50), Ok(()))?;
    rig.apply(&store);
    rig.sandbox.input("tap")?;
    rig.until("persisted macro order", |s| {
        s.synth.sent.lock().expect("fake synth").len() >= 2
    })?;
    check(
        rig.sent() == ["media-previous", "media-next"],
        "persisted macro order changed",
    )?;
    println!("PASS S5.4 firmware-session: persisted macro executes previous then next in order");

    store
        .save_macro(MacroSpec {
            id: "ordered".into(),
            name: "Interrupted".into(),
            steps: vec![
                StepSpec::Action {
                    action: ActionSpec::PreviousTrack,
                },
                StepSpec::Delay { ms: 2000 },
                StepSpec::Action {
                    action: ActionSpec::NextTrack,
                },
            ],
        })
        .map_err(|e| format!("macro update: {e:?}"))?;
    rig.apply(&store);
    let (reply, mut interrupted) = tauri::async_runtime::channel(1);
    // The host Test path has no raw-input freshness deadline, isolating session cancellation.
    rig.sandbox.desk.test(
        Action::Macro(store.resolved().macros["ordered"].clone()),
        rig.sandbox.started.elapsed(),
        &mut |_| {},
        reply,
    );
    rig.until("macro first step before disconnect", |s| {
        s.synth.sent.lock().expect("fake synth").len() == 3
    })?;
    rig.sandbox.connection("disconnect")?;
    // A correlated request is a worker barrier: the cancelled macro has finished before this reply.
    check(
        rig.action(Action::PreviousTrack)?.kind == FeedbackKind::Triggered,
        "post-cancel worker barrier failed",
    )?;
    check(
        rig.sent()
            == [
                "media-previous",
                "media-next",
                "media-previous",
                "media-previous",
            ],
        "macro continuation crossed disconnect",
    )?;
    check(
        interrupted
            .try_recv()
            .is_ok_and(|outcome| outcome.kind == FeedbackKind::Error),
        "interrupted macro did not finish as cancelled",
    )?;
    bind(&mut store, ActionSpec::PlayPause)?;
    rig.apply(&store);
    rig.sandbox.connection("reconnect")?;
    rig.until("reconnected macro sandbox", |s| s.snapshot.connected)?;
    rig.sandbox.input("tap")?;
    rig.until("new-session press", |s| {
        s.synth.sent.lock().expect("fake synth").len() >= 5
    })?;
    check(
        rig.sent().last().is_some_and(|s| s == "media-play-pause") && rig.sent().len() == 5,
        "old macro replayed on reconnect",
    )?;
    println!("PASS S5.4 desktop-action + real session/worker barrier: disconnect cancels remaining macro; fresh firmware press works");
    Ok(())
}

/// Requires an explicit temporary directory supplied by the native CLI, never app-managed settings.
pub fn desktop_smoke(executable: &Path, config_dir: &Path) -> Result<(), String> {
    persistence_and_profiles(executable, config_dir)?;
    microphones(executable)?;
    volume_and_media(executable)?;
    macros(executable, config_dir)?;
    Ok(())
}
