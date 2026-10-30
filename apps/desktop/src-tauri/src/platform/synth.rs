//! [`InputSynth`] over `enigo`, shared by Windows (`SendInput`) and macOS (Quartz events).
//!
//! macOS only delivers synthesized input from processes the user trusted under
//! Privacy & Security > Accessibility. Without it this reports
//! [`ActionError::PermissionRequired`] (and asks the OS to show its prompt), and sends nothing:
//! a missing permission is a capability state, never a reason to try another mechanism.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use enigo::{Direction, Enigo, Key, Keyboard, NewConError, Settings};

use super::{ActionError, InputGuard, InputSynth, MainThread, MediaKey, Shortcut, ShortcutKey};

/// How long to wait before cancelling pending input or reporting uncertain completion.
const MAIN_THREAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Key synthesis for the current desktop session.
pub struct EnigoInputSynth {
    main: Option<MainThread>,
}

impl EnigoInputSynth {
    /// `main` runs the input on the app's main thread (required on macOS, see `os_services`).
    #[must_use]
    pub fn new(main: Option<MainThread>) -> Self {
        Self { main }
    }

    fn on_main<P: 'static>(
        &self,
        guard: InputGuard,
        prepare: impl FnOnce() -> Result<P, ActionError> + Send + 'static,
        input: impl FnOnce(P) -> Result<(), ActionError> + Send + 'static,
    ) -> Result<(), ActionError> {
        self.on_main_for(guard, MAIN_THREAD_TIMEOUT, prepare, input)
    }

    fn on_main_for<P: 'static>(
        &self,
        mut guard: InputGuard,
        timeout: Duration,
        prepare: impl FnOnce() -> Result<P, ActionError> + Send + 'static,
        input: impl FnOnce(P) -> Result<(), ActionError> + Send + 'static,
    ) -> Result<(), ActionError> {
        let timeout_end = Instant::now() + timeout;
        let end = guard
            .deadline
            .map_or(timeout_end, |deadline| deadline.min(timeout_end));
        guard.deadline = Some(end);
        // Only the winner of Pending→Started or Pending→Cancelled may decide whether input runs.
        const PENDING: u8 = 0;
        const STARTED: u8 = 1;
        const CANCELLED: u8 = 2;
        let state = Arc::new(AtomicU8::new(PENDING));
        let queued = Arc::clone(&state);
        let (tx, rx) = mpsc::channel();
        let job: Box<dyn FnOnce() + Send> = Box::new(move || {
            let result = (|| {
                guard.check()?;
                let prepared = prepare()?;
                // Connecting can prompt/block. Recheck after it, before the first native event.
                guard.check()?;
                queued
                    .compare_exchange(PENDING, STARTED, Ordering::SeqCst, Ordering::SeqCst)
                    .map_err(|_| ActionError::Failed("input was cancelled".into()))?;
                input(prepared)
            })();
            let _ = tx.send(result);
        });
        if let Some(run) = &self.main {
            run(job);
        } else {
            job();
        }
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(result) => result,
            Err(_) => {
                match state.compare_exchange(PENDING, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
                {
                    Ok(_) => Err(ActionError::Failed("the app did not respond".into())),
                    Err(_) => Err(ActionError::Uncertain),
                }
            }
        }
    }
}

fn connect() -> Result<Enigo, ActionError> {
    let settings = Settings {
        // Only reached from a deliberate user action, which is when the OS prompt belongs.
        open_prompt_to_get_permissions: true,
        release_keys_when_dropped: true,
        ..Settings::default()
    };
    Enigo::new(&settings).map_err(|error| match error {
        NewConError::NoPermission => ActionError::PermissionRequired,
        other => ActionError::Failed(other.to_string()),
    })
}

fn failed(error: enigo::InputError) -> ActionError {
    ActionError::Failed(error.to_string())
}

fn key_of(key: ShortcutKey) -> Result<Key, ActionError> {
    Ok(match key {
        ShortcutKey::Char(c) => Key::Unicode(c),
        ShortcutKey::Function(n) => match n {
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            12 => Key::F12,
            13 => Key::F13,
            14 => Key::F14,
            15 => Key::F15,
            16 => Key::F16,
            17 => Key::F17,
            18 => Key::F18,
            19 => Key::F19,
            20 => Key::F20,
            #[cfg(target_os = "windows")]
            21 => Key::F21,
            #[cfg(target_os = "windows")]
            22 => Key::F22,
            #[cfg(target_os = "windows")]
            23 => Key::F23,
            #[cfg(target_os = "windows")]
            24 => Key::F24,
            _ => return Err(ActionError::Failed(format!("F{n} cannot be sent here"))),
        },
        ShortcutKey::Enter => Key::Return,
        ShortcutKey::Tab => Key::Tab,
        ShortcutKey::Space => Key::Space,
        ShortcutKey::Escape => Key::Escape,
        ShortcutKey::Backspace => Key::Backspace,
        ShortcutKey::Delete => Key::Delete,
        ShortcutKey::Home => Key::Home,
        ShortcutKey::End => Key::End,
        ShortcutKey::PageUp => Key::PageUp,
        ShortcutKey::PageDown => Key::PageDown,
        ShortcutKey::Up => Key::UpArrow,
        ShortcutKey::Down => Key::DownArrow,
        ShortcutKey::Left => Key::LeftArrow,
        ShortcutKey::Right => Key::RightArrow,
    })
}

impl InputSynth for EnigoInputSynth {
    fn send_media_key(&self, key: MediaKey, guard: InputGuard) -> Result<(), ActionError> {
        let key = match key {
            MediaKey::PlayPause => Key::MediaPlayPause,
            MediaKey::Previous => Key::MediaPrevTrack,
            MediaKey::Next => Key::MediaNextTrack,
        };
        self.on_main(guard, connect, move |mut enigo| {
            enigo.key(key, Direction::Click).map_err(failed)
        })
    }

    fn send_shortcut(&self, shortcut: &Shortcut, guard: InputGuard) -> Result<(), ActionError> {
        let shortcut = *shortcut;
        let key = key_of(shortcut.key)?;
        self.on_main(guard, connect, move |mut enigo| {
            send_shortcut(&mut enigo, &shortcut, key)
        })
    }
}

fn send_shortcut(enigo: &mut Enigo, shortcut: &Shortcut, key: Key) -> Result<(), ActionError> {
    {
        let modifiers: Vec<Key> = [
            (shortcut.ctrl, Key::Control),
            (shortcut.alt, Key::Alt),
            (shortcut.shift, Key::Shift),
            (shortcut.meta, Key::Meta),
        ]
        .into_iter()
        .filter_map(|(on, key)| on.then_some(key))
        .collect();
        let mut pressed = 0;
        let mut result = Ok(());
        for modifier in &modifiers {
            result = enigo.key(*modifier, Direction::Press).map_err(failed);
            if result.is_err() {
                break;
            }
            pressed += 1;
        }
        if result.is_ok() {
            result = enigo.key(key, Direction::Click).map_err(failed);
        }
        // Release whatever went down, in reverse, even after a failure: a stuck modifier would
        // corrupt the user's next keystrokes.
        for modifier in modifiers[..pressed].iter().rev() {
            let released = enigo.key(*modifier, Direction::Release).map_err(failed);
            result = result.and(released);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    type Job = Box<dyn FnOnce() + Send>;
    const TEST_TIMEOUT: Duration = Duration::from_millis(50);

    fn dispatcher() -> (EnigoInputSynth, mpsc::Receiver<Job>) {
        let (tx, rx) = mpsc::channel();
        (
            EnigoInputSynth::new(Some(Arc::new(move |job| tx.send(job).unwrap()))),
            rx,
        )
    }

    fn guard() -> InputGuard {
        InputGuard {
            deadline: None,
            current: Arc::new(|| true),
        }
    }

    #[test]
    fn an_unsupported_key_is_rejected_before_queueing_or_connecting() {
        let (synth, jobs) = dispatcher();
        let mut shortcut: Shortcut = "Ctrl+M".parse().unwrap();
        shortcut.key = ShortcutKey::Function(u8::MAX);
        assert!(matches!(
            synth.send_shortcut(&shortcut, guard()),
            Err(ActionError::Failed(_))
        ));
        assert!(matches!(jobs.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn accepted_function_keys_match_the_native_sender() {
        let keys = [
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
            Key::F13,
            Key::F14,
            Key::F15,
            Key::F16,
            Key::F17,
            Key::F18,
            Key::F19,
            Key::F20,
            #[cfg(target_os = "windows")]
            Key::F21,
            #[cfg(target_os = "windows")]
            Key::F22,
            #[cfg(target_os = "windows")]
            Key::F23,
            #[cfg(target_os = "windows")]
            Key::F24,
        ];
        for n in 0..=u8::MAX {
            let parsed = format!("Ctrl+F{n}").parse::<Shortcut>();
            if let Some(expected) = n.checked_sub(1).and_then(|i| keys.get(usize::from(i))) {
                let shortcut = parsed.unwrap();
                assert_eq!(key_of(shortcut.key).unwrap(), *expected, "F{n}");
                assert_eq!(shortcut.to_string(), format!("Ctrl+F{n}"));
            } else {
                assert!(parsed.is_err(), "unsupported F{n} was accepted");
                assert!(key_of(ShortcutKey::Function(n)).is_err(), "F{n}");
            }
        }
    }

    #[test]
    fn timeout_cancels_a_job_that_the_main_thread_runs_later() {
        let (synth, jobs) = dispatcher();
        let sent = Arc::new(AtomicBool::new(false));
        let effect = sent.clone();
        let caller = std::thread::spawn(move || {
            synth.on_main_for(
                guard(),
                TEST_TIMEOUT,
                || Ok(()),
                move |()| {
                    effect.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        let job = jobs.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            caller.join().unwrap(),
            Err(ActionError::Failed(_))
        ));
        job();
        assert!(!sent.load(Ordering::SeqCst));
    }

    #[test]
    fn request_deadline_cancels_waiting_before_the_main_thread_timeout() {
        let (synth, jobs) = dispatcher();
        let started = Instant::now();
        let guard = InputGuard {
            deadline: Some(started + TEST_TIMEOUT),
            ..guard()
        };
        let caller = std::thread::spawn(move || {
            synth.on_main_for(
                guard,
                Duration::from_secs(2),
                || Ok(()),
                |()| panic!("injected input after request deadline"),
            )
        });
        let job = jobs.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            caller.join().unwrap(),
            Err(ActionError::Failed(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        job();
    }

    #[test]
    fn timeout_during_preparation_cancels_before_the_first_event() {
        let (synth, jobs) = dispatcher();
        let (preparing, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let sent = Arc::new(AtomicBool::new(false));
        let effect = sent.clone();
        let caller = std::thread::spawn(move || {
            synth.on_main_for(
                guard(),
                MAIN_THREAD_TIMEOUT,
                move || {
                    preparing.send(()).unwrap();
                    resume.recv().unwrap();
                    Ok(())
                },
                move |()| {
                    effect.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        let job = jobs.recv_timeout(Duration::from_secs(2)).unwrap();
        let main = std::thread::spawn(job);
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            caller.join().unwrap(),
            Err(ActionError::Failed(_))
        ));
        release.send(()).unwrap();
        main.join().unwrap();
        assert!(!sent.load(Ordering::SeqCst));
    }

    #[test]
    fn timeout_after_input_started_reports_uncertainty() {
        let (synth, jobs) = dispatcher();
        let (injecting, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let caller = std::thread::spawn(move || {
            synth.on_main_for(
                guard(),
                MAIN_THREAD_TIMEOUT,
                || Ok(()),
                move |()| {
                    injecting.send(()).unwrap();
                    resume.recv().unwrap();
                    Ok(())
                },
            )
        });
        let job = jobs.recv_timeout(Duration::from_secs(2)).unwrap();
        let main = std::thread::spawn(job);
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(caller.join().unwrap(), Err(ActionError::Uncertain));
        release.send(()).unwrap();
        main.join().unwrap();
    }

    #[test]
    fn successful_main_dispatch_injects_exactly_once() {
        let (synth, jobs) = dispatcher();
        let sent = Arc::new(AtomicU8::new(0));
        let effect = sent.clone();
        let caller = std::thread::spawn(move || {
            synth.on_main_for(
                guard(),
                Duration::from_secs(2),
                || Ok(()),
                move |()| {
                    effect.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        jobs.recv_timeout(Duration::from_secs(2)).unwrap()();
        assert_eq!(caller.join().unwrap(), Ok(()));
        assert_eq!(sent.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn expired_request_deadline_blocks_input_even_without_main_queueing() {
        let synth = EnigoInputSynth::new(None);
        let guard = InputGuard {
            deadline: Some(Instant::now()),
            ..guard()
        };
        assert!(matches!(
            synth.on_main(
                guard,
                || panic!("prepared expired input"),
                |(): ()| panic!("injected expired input")
            ),
            Err(ActionError::Failed(_))
        ));
    }

    #[test]
    fn session_or_context_change_during_preparation_blocks_injection() {
        let synth = EnigoInputSynth::new(None);
        let current = Arc::new(AtomicBool::new(true));
        let active = current.clone();
        let guard = InputGuard {
            current: Arc::new(move || active.load(Ordering::SeqCst)),
            ..guard()
        };
        assert!(matches!(
            synth.on_main(
                guard,
                move || {
                    current.store(false, Ordering::SeqCst);
                    Ok(())
                },
                |()| panic!("injected after session or foreground changed"),
            ),
            Err(ActionError::Failed(_))
        ));
    }

    #[test]
    fn a_guard_that_blocks_cannot_admit_input_after_its_deadline() {
        let synth = EnigoInputSynth::new(None);
        let guard = InputGuard {
            deadline: Some(Instant::now() + Duration::from_millis(5)),
            current: Arc::new(|| {
                std::thread::sleep(Duration::from_millis(10));
                true
            }),
        };
        assert!(matches!(
            synth.on_main(
                guard,
                || Ok(()),
                |()| panic!("injected after blocking guard")
            ),
            Err(ActionError::Failed(_))
        ));
    }
}
