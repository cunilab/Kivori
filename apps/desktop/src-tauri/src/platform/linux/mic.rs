//! Linux microphone mute on EVERY capture device (ADR-0011), through the PulseAudio protocol (served
//! by PipeWire through `pipewire-pulse`).
//!
//! WHICH DEVICES: every source that is not the "monitor" of an output (a monitor captures what is
//! playing, not a microphone). Each one's mute is set, then the sources are read back, so the result
//! is always what the server reports for that device, never the request. Muting only the default
//! source is rejected: another app may record from a different one, so "muted" would be false.
//!
//! WHILE MUTED: a source that ARRIVES (a USB microphone plugged in: a name not seen on the previous
//! read) is muted too and read back; if that fails it reads live, so the state says live, not
//! muted. A source that was already there and is unmuted from outside (the GNOME panel) is NOT
//! muted again: it is reported as an `External` change, as on Windows and macOS. Unmuting ends the
//! hold.
//!
//! OUTSIDE CHANGES: a source event makes the thread re-read; the change is `EndpointRebind` when the
//! set of devices changed, else `External`. Own writes are published as `Kivori` at write time, so
//! their echo changes nothing. Like the output backend, the subscribe callback is ingress only.
//!
//! IN USE: any app capture stream (source output) that is not corked and is not on a monitor. It is
//! kept fresh by source-output events, so `in_use()` never blocks. An unreachable server is unknown
//! (`None`), never "not in use".

use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::pulse::{
    join_within, Conn, Event, Link, Source, SourceOutput, CALL_DEADLINE, JOIN_DEADLINE, UNREACHABLE,
};
use crate::platform::{
    ActionAvailability, BackendError, ChangeOrigin, ConfirmationClass, MicBackend, MicChange,
    MicDevice, MicMute,
};

const SETTLE: Duration = Duration::from_millis(30);
const MAX_COALESCE: Duration = Duration::from_millis(200);
const RECONNECT: Duration = Duration::from_secs(3);
const READY_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    NoServer = 0,
    NoMic = 1,
    Ok = 2,
}

impl Health {
    fn from_u8(value: u8) -> Self {
        match value {
            2 => Self::Ok,
            1 => Self::NoMic,
            _ => Self::NoServer,
        }
    }
}

enum Command {
    Devices(Sender<Result<Vec<MicDevice>, BackendError>>),
    SetMute(bool, Sender<Result<Vec<MicDevice>, BackendError>>),
    Event,
    Shutdown,
}

pub struct LinuxMicBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<MicChange>>,
    health: Arc<AtomicU8>,
    in_use: Arc<Mutex<Option<bool>>>,
    thread: Option<JoinHandle<()>>,
}

impl LinuxMicBackend {
    #[must_use]
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let (change_tx, change_rx) = mpsc::channel::<MicChange>();
        let health = Arc::new(AtomicU8::new(Health::NoServer as u8));
        let in_use: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let (thread_health, thread_in_use) = (Arc::clone(&health), Arc::clone(&in_use));
        let self_commands = command_tx.clone();
        let thread = std::thread::Builder::new()
            .name("kivori-mic".to_string())
            .spawn(move || {
                mic_thread(
                    &command_rx,
                    &change_tx,
                    &thread_health,
                    &thread_in_use,
                    &self_commands,
                    &ready_tx,
                );
            })
            .ok();
        // As in the output backend: right after `new()`, but never hung by a silent server.
        let ready = ready_rx.recv_timeout(READY_WAIT).is_ok();
        Self {
            commands: command_tx,
            changes: Mutex::new(change_rx),
            health,
            in_use,
            thread: thread.filter(|_| ready),
        }
    }

    /// Sends a command and waits for its reply, for at most [`CALL_DEADLINE`] (the owning thread
    /// bounds its own requests; this is the backstop for the caller).
    fn call<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, BackendError>>) -> Command,
    ) -> Result<T, BackendError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(make(reply_tx))
            .map_err(|_| gone("the microphone thread is gone"))?;
        reply_rx
            .recv_timeout(CALL_DEADLINE)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => gone("the audio server did not answer in time"),
                RecvTimeoutError::Disconnected => gone("the microphone thread is gone"),
            })?
    }
}

impl Default for LinuxMicBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxMicBackend {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            // Bounded, as in the output backend.
            join_within(thread, JOIN_DEADLINE);
        }
    }
}

fn gone(why: &str) -> BackendError {
    BackendError::Os(why.to_string())
}

impl MicBackend for LinuxMicBackend {
    fn availability(&self) -> ActionAvailability {
        match Health::from_u8(self.health.load(Ordering::SeqCst)) {
            Health::Ok => ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            Health::NoMic => ActionAvailability::RuntimeUnavailable {
                reason: "no microphone".to_string(),
            },
            Health::NoServer => ActionAvailability::RuntimeUnavailable {
                reason: UNREACHABLE.to_string(),
            },
        }
    }

    fn devices(&self) -> Result<Vec<MicDevice>, BackendError> {
        self.call(Command::Devices)
    }

    fn set_mute(&self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        self.call(|reply| Command::SetMute(muted, reply))
    }

    fn in_use(&self) -> Option<bool> {
        *self.in_use.lock().expect("in-use mutex")
    }

    fn try_recv_change(&self) -> Option<MicChange> {
        self.changes
            .lock()
            .expect("changes channel mutex")
            .try_recv()
            .ok()
    }
}

/// The microphones among `sources`: everything that is not a monitor of an output.
pub(super) fn microphones(sources: Vec<Source>) -> Vec<Source> {
    sources.into_iter().filter(|s| !s.is_monitor).collect()
}

fn device_of(source: &Source) -> MicDevice {
    MicDevice {
        name: source.description.clone(),
        mute: if source.mute {
            MicMute::Muted
        } else {
            MicMute::Live
        },
        // Which device an app records from does not matter for a device with a mute control, and
        // every pulse source has one, so it is left unknown rather than read.
        capturing: None,
    }
}

/// What to do about the microphones just read. `known` is the set of device names seen on the
/// previous read (replaced by the current set). While mute is held, the indexes returned are the
/// devices that ARRIVED unmuted and must be muted; a device that was already there and is now
/// unmuted was unmuted from outside and is left alone, so the outside change is reported.
pub(super) fn arrivals_to_mute(
    hold_muted: bool,
    known: &mut HashSet<String>,
    mics: &[Source],
) -> Vec<u32> {
    let arrived: Vec<u32> = mics
        .iter()
        .filter(|m| !known.contains(&m.name) && !m.mute)
        .map(|m| m.index)
        .collect();
    *known = mics.iter().map(|m| m.name.clone()).collect();
    if hold_muted {
        arrived
    } else {
        Vec::new()
    }
}

/// Whether an app is capturing from a microphone: a stream that is running (not corked) on a source
/// that is not a monitor.
pub(super) fn capture_in_use(outputs: &[SourceOutput], sources: &[Source]) -> bool {
    outputs.iter().any(|output| {
        !output.corked
            && sources
                .iter()
                .find(|s| s.index == output.source)
                .is_some_and(|s| !s.is_monitor)
    })
}

/// How a changed device list is published: a different set of devices is a rebind, otherwise it is
/// an outside change. `None` when it is what was last published.
pub(super) fn classify_mic(
    reported: Option<&[MicDevice]>,
    seen: &[MicDevice],
) -> Option<ChangeOrigin> {
    let reported = reported?;
    if reported == seen {
        return None;
    }
    let names = |devices: &[MicDevice]| {
        let mut names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
        names.sort();
        names
    };
    Some(if names(reported) == names(seen) {
        ChangeOrigin::External
    } else {
        ChangeOrigin::EndpointRebind
    })
}

struct Mic {
    link: Link,
    /// Mic mute is on: a device that arrives is muted too.
    hold_muted: bool,
    /// Names of the devices seen on the previous read: what is NOT in it has just arrived.
    known: HashSet<String>,
    /// Last device state published; observations that do not change it are dropped.
    reported: Option<Vec<MicDevice>>,
    health: Arc<AtomicU8>,
    in_use: Arc<Mutex<Option<bool>>>,
    changes: Sender<MicChange>,
}

impl Mic {
    fn set_health(&self, health: Health) {
        self.health.store(health as u8, Ordering::SeqCst);
    }

    fn conn(&mut self) -> Option<&mut Conn> {
        if self.link.ensure() {
            self.reported = None;
            let _ = self.refresh();
        }
        if self.link.is_down() {
            self.set_health(Health::NoServer);
            *self.in_use.lock().expect("in-use mutex") = None;
        }
        self.link.conn()
    }

    /// The microphones as the server reports them now, updating health.
    fn sources(&mut self) -> Result<Vec<Source>, BackendError> {
        let Some(conn) = self.conn() else {
            return Err(BackendError::Os(UNREACHABLE.to_string()));
        };
        let Some(all) = conn.sources() else {
            self.set_health(Health::NoServer);
            return Err(BackendError::Os(UNREACHABLE.to_string()));
        };
        let in_use = conn
            .source_outputs()
            .map(|outputs| capture_in_use(&outputs, &all));
        *self.in_use.lock().expect("in-use mutex") = in_use;
        let mics = microphones(all);
        self.set_health(if mics.is_empty() {
            Health::NoMic
        } else {
            Health::Ok
        });
        Ok(mics)
    }

    /// Mutes (or unmutes) `mics` and returns them as read back.
    fn write(&mut self, mics: &[Source], muted: bool) -> Result<Vec<Source>, BackendError> {
        let conn = self.link.conn().ok_or(BackendError::NoEndpoint)?;
        // A failed write is left to the read-back to report: the device then still reads live.
        for mic in mics {
            let _ = conn.set_source_mute(mic.index, muted);
        }
        self.sources()
    }

    fn devices_now(&mut self) -> Result<Vec<MicDevice>, BackendError> {
        let mut mics = self.sources()?;
        let arrived = arrivals_to_mute(self.hold_muted, &mut self.known, &mics);
        if !arrived.is_empty() {
            let new: Vec<Source> = mics
                .iter()
                .filter(|m| arrived.contains(&m.index))
                .cloned()
                .collect();
            mics = self.write(&new, true)?;
        }
        Ok(mics.iter().map(device_of).collect())
    }

    fn devices(&mut self) -> Result<Vec<MicDevice>, BackendError> {
        let devices = self.devices_now()?;
        self.report(&devices, ChangeOrigin::EndpointRebind);
        Ok(devices)
    }

    fn set_mute(&mut self, muted: bool) -> Result<Vec<MicDevice>, BackendError> {
        let mics = self.sources()?;
        if mics.is_empty() {
            self.report(&[], ChangeOrigin::EndpointRebind);
            return Err(BackendError::NoEndpoint);
        }
        self.hold_muted = muted;
        self.known = mics.iter().map(|m| m.name.clone()).collect();
        let devices: Vec<MicDevice> = self.write(&mics, muted)?.iter().map(device_of).collect();
        self.report(&devices, ChangeOrigin::Kivori);
        Ok(devices)
    }

    /// Re-reads after an event and publishes what changed.
    fn refresh(&mut self) -> Result<(), BackendError> {
        let devices = self.devices_now()?;
        if let Some(origin) = classify_mic(self.reported.as_deref(), &devices) {
            self.report(&devices, origin);
        } else if self.reported.is_none() {
            self.report(&devices, ChangeOrigin::EndpointRebind);
        }
        Ok(())
    }

    fn report(&mut self, devices: &[MicDevice], origin: ChangeOrigin) {
        if self.reported.as_deref() == Some(devices) {
            return;
        }
        self.reported = Some(devices.to_vec());
        let _ = self.changes.send(MicChange {
            devices: devices.to_vec(),
            origin,
        });
    }
}

/// Body of the dedicated `kivori-mic` OS thread. Owns the connection for its whole lifetime.
fn mic_thread(
    commands: &Receiver<Command>,
    changes: &Sender<MicChange>,
    health: &Arc<AtomicU8>,
    in_use: &Arc<Mutex<Option<bool>>>,
    self_commands: &Sender<Command>,
    ready: &Sender<()>,
) {
    let events = self_commands.clone();
    let mut mic = Mic {
        link: Link::new(
            &[Event::Source, Event::SourceOutput, Event::Server],
            // INGRESS ONLY: runs on libpulse's thread.
            Arc::new(move |_| {
                let _ = events.send(Command::Event);
            }),
        ),
        hold_muted: false,
        known: HashSet::new(),
        reported: None,
        health: Arc::clone(health),
        in_use: Arc::clone(in_use),
        changes: changes.clone(),
    };
    mic.conn();
    let _ = ready.send(());

    let mut dirty_since: Option<Instant> = None;
    loop {
        let received = if dirty_since.is_some() {
            commands.recv_timeout(SETTLE)
        } else if mic.link.is_down() {
            commands.recv_timeout(RECONNECT)
        } else {
            commands.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match received {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Command::Event) => {
                dirty_since.get_or_insert_with(Instant::now);
            }
            Ok(Command::Devices(reply)) => {
                let _ = reply.send(mic.devices());
            }
            Ok(Command::SetMute(muted, reply)) => {
                let _ = reply.send(mic.set_mute(muted));
            }
            Err(RecvTimeoutError::Timeout) => {
                mic.conn();
                if dirty_since.take().is_some() {
                    let _ = mic.refresh();
                }
            }
        }
        if dirty_since.is_some_and(|since| since.elapsed() >= MAX_COALESCE) {
            dirty_since = None;
            let _ = mic.refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{mic_state, mics_muted};

    fn source(index: u32, name: &str, mute: bool, is_monitor: bool) -> Source {
        Source {
            index,
            name: name.to_string(),
            description: format!("{name} description"),
            mute,
            is_monitor,
        }
    }

    fn device(name: &str, mute: MicMute) -> MicDevice {
        MicDevice {
            name: name.to_string(),
            mute,
            capturing: None,
        }
    }

    #[test]
    fn monitors_are_not_microphones() {
        let mics = microphones(vec![
            source(1, "alsa_input.usb-mic", false, false),
            source(2, "alsa_output.pci.analog-stereo.monitor", false, true),
            source(3, "bluez_input.headset", true, false),
        ]);
        assert_eq!(mics.iter().map(|m| m.index).collect::<Vec<_>>(), [1, 3]);
    }

    #[test]
    fn a_device_reads_muted_or_live_by_its_own_mute() {
        assert_eq!(device_of(&source(1, "a", true, false)).mute, MicMute::Muted);
        assert_eq!(device_of(&source(1, "a", false, false)).mute, MicMute::Live);
        assert_eq!(
            device_of(&source(1, "a", false, false)).name,
            "a description",
            "the description is what the user sees"
        );
    }

    #[test]
    fn every_microphone_muted_is_muted_and_one_live_is_live() {
        let all_muted = [device("a", MicMute::Muted), device("b", MicMute::Muted)];
        assert_eq!(mics_muted(&all_muted), Some(true));
        let one_live = [device("a", MicMute::Muted), device("b", MicMute::Live)];
        assert_eq!(mics_muted(&one_live), Some(false));
        assert_eq!(mic_state(&one_live, Some(true)).muted, Some(false));
        assert_eq!(mics_muted(&[]), None, "no microphone is not muted");
    }

    #[test]
    fn capture_is_in_use_only_for_running_streams_on_a_real_microphone() {
        let sources = [
            source(1, "mic", false, false),
            source(2, "out.monitor", false, true),
        ];
        let running = |source| SourceOutput {
            source,
            corked: false,
        };
        let corked = |source| SourceOutput {
            source,
            corked: true,
        };
        assert!(capture_in_use(&[running(1)], &sources));
        assert!(!capture_in_use(&[corked(1)], &sources), "paused capture");
        assert!(
            !capture_in_use(&[running(2)], &sources),
            "a monitor records playback"
        );
        assert!(!capture_in_use(&[running(9)], &sources), "unknown source");
        assert!(!capture_in_use(&[], &sources));
        assert!(capture_in_use(&[corked(1), running(1)], &sources));
    }

    fn names(mics: &[Source]) -> HashSet<String> {
        mics.iter().map(|m| m.name.clone()).collect()
    }

    #[test]
    fn a_source_unmuted_from_outside_is_not_muted_again_and_is_reported_external() {
        // Kivori muted "a"; the GNOME panel then unmuted it.
        let mut known = names(&[source(1, "a", true, false)]);
        let now = [source(1, "a", false, false)];
        assert_eq!(
            arrivals_to_mute(true, &mut known, &now),
            Vec::<u32>::new(),
            "an existing source is left as the outside change set it"
        );
        let reported = [device("a description", MicMute::Muted)];
        let seen: Vec<MicDevice> = now.iter().map(device_of).collect();
        assert_eq!(
            classify_mic(Some(&reported), &seen),
            Some(ChangeOrigin::External),
            "and the change is not hidden"
        );
    }

    #[test]
    fn a_hot_plugged_source_while_muted_is_muted_but_only_that_one() {
        let mut known = names(&[source(1, "a", true, false), source(2, "b", false, false)]);
        let now = [
            source(1, "a", true, false),
            source(2, "b", false, false),
            source(7, "usb", false, false),
        ];
        assert_eq!(
            arrivals_to_mute(true, &mut known, &now),
            [7],
            "the arrival only: b was already there"
        );
        assert!(known.contains("usb"), "it is known from now on");
        assert_eq!(arrivals_to_mute(true, &mut known, &now), Vec::<u32>::new());
    }

    #[test]
    fn nothing_is_muted_on_arrival_unless_mute_is_held() {
        let mut known = HashSet::new();
        assert!(arrivals_to_mute(false, &mut known, &[source(1, "a", false, false)]).is_empty());
        assert_eq!(known.len(), 1);
    }

    #[test]
    fn an_unplugged_and_replugged_source_arrives_again() {
        let mut known = names(&[source(1, "usb", true, false)]);
        assert!(arrivals_to_mute(true, &mut known, &[]).is_empty());
        assert_eq!(
            arrivals_to_mute(true, &mut known, &[source(9, "usb", false, false)]),
            [9]
        );
    }

    #[test]
    fn a_changed_device_set_is_a_rebind_and_a_toggled_mute_is_external() {
        let a_live = [device("a", MicMute::Live)];
        let a_muted = [device("a", MicMute::Muted)];
        let ab = [device("a", MicMute::Live), device("b", MicMute::Live)];
        assert_eq!(classify_mic(Some(&a_live), &a_live), None);
        assert_eq!(
            classify_mic(Some(&a_live), &a_muted),
            Some(ChangeOrigin::External)
        );
        assert_eq!(
            classify_mic(Some(&a_live), &ab),
            Some(ChangeOrigin::EndpointRebind)
        );
        assert_eq!(
            classify_mic(Some(&ab), &a_live),
            Some(ChangeOrigin::EndpointRebind)
        );
        assert_eq!(
            classify_mic(None, &a_live),
            None,
            "the first read is published by the caller"
        );
    }

    /// Real audio server, opt-in: mutes every microphone, reads each back, then puts each one's own
    /// prior mute back through a second connection (the backend has no per-device API, by design).
    /// `cargo test -p kivori-desktop -- --ignored --nocapture linux_mic_round_trip`.
    #[test]
    #[ignore = "changes the live microphone mute (restored afterwards)"]
    fn linux_mic_round_trip() {
        let backend = LinuxMicBackend::new();
        println!("availability: {:?}", backend.availability());
        let mut raw = Conn::connect("Kivori test", &[], |_| {}).expect("audio server");
        let prior: Vec<(u32, bool)> = microphones(raw.sources().expect("list sources"))
            .iter()
            .map(|s| (s.index, s.mute))
            .collect();
        let before = backend.devices().expect("list microphones");
        println!(
            "{} microphone(s), in use: {:?}",
            before.len(),
            backend.in_use()
        );
        for d in &before {
            println!("  {:?} {:?}", d.name, d.mute);
        }
        if before.is_empty() {
            return;
        }
        let muted = backend.set_mute(true).expect("mute all");
        let live = backend.set_mute(false).expect("unmute all");
        // Restore before asserting, so a failed assertion cannot leave a microphone muted.
        for (index, mute) in &prior {
            assert!(raw.set_source_mute(*index, *mute), "restore source {index}");
        }
        assert!(muted.iter().all(|d| d.mute == MicMute::Muted), "{muted:?}");
        assert_eq!(mics_muted(&muted), Some(true));
        assert!(live.iter().all(|d| d.mute == MicMute::Live), "{live:?}");
        let after: Vec<(u32, bool)> = microphones(raw.sources().expect("list sources"))
            .iter()
            .map(|s| (s.index, s.mute))
            .collect();
        assert_eq!(after, prior, "every microphone is back as it was");
    }
}
