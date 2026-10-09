//! Pure window-lifecycle policy (FR-030, SC-007).
//!
//! Closing the settings window HIDES it and keeps the native core (connection manager, heartbeat,
//! reconnect) running; only an explicit Quit ends the process; re-activating (dock/taskbar click or a
//! second-instance launch) surfaces the single existing window. This is the pure decision layer;
//! `main.rs` binds it to Tauri window/tray/single-instance events. Device-task survival is a property
//! of this policy, not of the webview — the runtime owns the task (window ≠ device).

/// The argument the OS launcher passes when it starts Kivori at login (see `lib.rs`).
pub const AUTOSTART_ARG: &str = "--autostart";

/// Whether the process was started by the login launcher, so the window stays hidden.
pub fn launched_at_login<I>(args: I) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == AUTOSTART_ARG)
}

/// A lifecycle input from the OS / Tauri layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// The user requested to close the window (title-bar close / Cmd-W).
    CloseRequested,
    /// An explicit Quit (tray/menu "Quit", or an OS quit request).
    QuitRequested,
    /// The app was re-activated: a dock/taskbar click, or a second-instance launch activating this one.
    Reactivated,
    /// The window finished showing.
    WindowShown,
    /// The window finished hiding.
    WindowHidden,
}

/// The action `main.rs` must perform in response to a [`LifecycleEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    /// Hide the window but keep the process (and the device task) alive.
    HideWindow,
    /// Show and focus the single window.
    ShowWindow,
    /// Terminate the process.
    Quit,
    /// Do nothing.
    None,
}

/// Tracks window visibility and whether the process should keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLifecycle {
    window_visible: bool,
    running: bool,
}

impl WindowLifecycle {
    /// A newly launched app: window visible, process running.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            window_visible: true,
            running: true,
        }
    }

    /// A login launch: the process runs (tray only) and the window is hidden until re-activated.
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            window_visible: false,
            running: true,
        }
    }

    /// Whether the window is currently visible.
    #[must_use]
    pub const fn window_visible(&self) -> bool {
        self.window_visible
    }

    /// Whether the process should keep running (false only after an explicit Quit).
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Whether the background device task should run. It is owned by the core runtime and survives
    /// window hide/close and webview loss — stopping only on an explicit Quit (FR-030, SC-007).
    #[must_use]
    pub const fn device_task_should_run(&self) -> bool {
        self.running
    }

    /// Applies a lifecycle event and returns the action to perform.
    pub fn on_event(&mut self, event: LifecycleEvent) -> LifecycleAction {
        match event {
            LifecycleEvent::CloseRequested => {
                if self.window_visible {
                    self.window_visible = false;
                    LifecycleAction::HideWindow
                } else {
                    LifecycleAction::None
                }
            }
            LifecycleEvent::QuitRequested => {
                self.running = false;
                LifecycleAction::Quit
            }
            // Re-activation always surfaces (and focuses) the single window.
            LifecycleEvent::Reactivated => {
                self.window_visible = true;
                LifecycleAction::ShowWindow
            }
            LifecycleEvent::WindowShown => {
                self.window_visible = true;
                LifecycleAction::None
            }
            LifecycleEvent::WindowHidden => {
                self.window_visible = false;
                LifecycleAction::None
            }
        }
    }
}

impl Default for WindowLifecycle {
    fn default() -> Self {
        Self::new()
    }
}
