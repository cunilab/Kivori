//! Window-lifecycle policy tests (T092; FR-030, SC-007). No real OS window.

use kivori_desktop::window_lifecycle::{
    launched_at_login, LifecycleAction, LifecycleEvent, WindowLifecycle,
};

#[test]
fn close_hides_window_but_keeps_process_and_device_task() {
    let mut lc = WindowLifecycle::new();
    assert!(lc.window_visible());
    assert_eq!(
        lc.on_event(LifecycleEvent::CloseRequested),
        LifecycleAction::HideWindow
    );
    assert!(!lc.window_visible());
    assert!(lc.is_running(), "closing the window never ends the process");
    assert!(
        lc.device_task_should_run(),
        "the device task survives a window close (SC-007)"
    );
}

#[test]
fn quit_terminates_and_stops_the_device_task() {
    let mut lc = WindowLifecycle::new();
    assert_eq!(
        lc.on_event(LifecycleEvent::QuitRequested),
        LifecycleAction::Quit
    );
    assert!(!lc.is_running());
    assert!(
        !lc.device_task_should_run(),
        "only an explicit Quit stops the device task"
    );
}

#[test]
fn reactivate_shows_the_hidden_window() {
    let mut lc = WindowLifecycle::new();
    lc.on_event(LifecycleEvent::CloseRequested);
    assert!(!lc.window_visible());
    assert_eq!(
        lc.on_event(LifecycleEvent::Reactivated),
        LifecycleAction::ShowWindow
    );
    assert!(lc.window_visible());
}

#[test]
fn second_instance_activation_surfaces_the_existing_window() {
    // Single-instance: a second launch arrives as `Reactivated` and must surface the one window
    // rather than start a new process.
    let mut lc = WindowLifecycle::new();
    lc.on_event(LifecycleEvent::CloseRequested);
    assert_eq!(
        lc.on_event(LifecycleEvent::Reactivated),
        LifecycleAction::ShowWindow
    );
    assert!(lc.window_visible());
    assert!(lc.is_running());
}

#[test]
fn device_task_survives_simulated_webview_loss() {
    // "Webview loss" = the window is hidden / torn down; the native device task must keep running.
    let mut lc = WindowLifecycle::new();
    lc.on_event(LifecycleEvent::WindowHidden);
    assert!(!lc.window_visible());
    assert!(lc.device_task_should_run());
    // Repeated hide/show churn never stops it.
    lc.on_event(LifecycleEvent::WindowShown);
    lc.on_event(LifecycleEvent::CloseRequested);
    assert!(lc.device_task_should_run());
}

#[test]
fn close_when_already_hidden_is_a_noop() {
    let mut lc = WindowLifecycle::new();
    lc.on_event(LifecycleEvent::CloseRequested);
    assert_eq!(
        lc.on_event(LifecycleEvent::CloseRequested),
        LifecycleAction::None
    );
    assert!(!lc.window_visible());
    assert!(lc.is_running());
}

#[test]
fn hidden_start_runs_without_a_window_and_reactivation_shows_it() {
    let mut lc = WindowLifecycle::hidden();
    assert!(!lc.window_visible());
    assert!(lc.is_running());
    assert!(lc.device_task_should_run());
    assert_eq!(
        lc.on_event(LifecycleEvent::Reactivated),
        LifecycleAction::ShowWindow
    );
    assert!(lc.window_visible());
}

#[test]
fn close_on_a_hidden_start_is_a_noop() {
    let mut lc = WindowLifecycle::hidden();
    assert_eq!(
        lc.on_event(LifecycleEvent::CloseRequested),
        LifecycleAction::None
    );
    assert!(!lc.window_visible());
    assert!(lc.is_running());
}

#[test]
fn autostart_argument_is_recognised_anywhere_and_only_exactly() {
    assert!(launched_at_login(["kivori-desktop", "--autostart"]));
    assert!(launched_at_login(["--autostart"]));
    assert!(!launched_at_login(["kivori-desktop"]));
    assert!(!launched_at_login(Vec::<String>::new()));
    assert!(!launched_at_login(["--autostart=1", "autostart", "--auto"]));
}
