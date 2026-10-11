//! Exercise first-open initialization with the Windows UI thread's 1 MiB stack.
#![cfg(feature = "device-studio")]

#[test]
fn first_preview_frame_fits_the_windows_ui_stack() {
    // A stack overflow aborts the process, so isolate this regression from the test runner.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "cold_preview_child", "--nocapture"])
        .env("KIVORI_COLD_PREVIEW_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cold preview failed: {:?}\n{}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cold_preview_child() {
    if std::env::var_os("KIVORI_COLD_PREVIEW_CHILD").is_none() {
        return;
    }
    std::thread::Builder::new()
        .name("windows-ui-stack".into())
        .stack_size(1024 * 1024)
        .spawn(|| {
            eprintln!("initializing bundled assets");
            let blob = kivori_desktop::render::bundled_blob();
            eprintln!("rendering first Device Studio frame");
            let timeline = kivori_desktop::render::animation::AnimationTimeline {
                initial_state: "idle".into(),
                events: vec![],
                action_events: vec![],
                idle_life: None,
            };
            let rgba = kivori_desktop::render::render_animation_rgba(blob, &timeline, 0).unwrap();
            assert_eq!(rgba.len(), 240 * 240 * 4);
            assert!(std::ptr::eq(blob, kivori_desktop::render::bundled_blob()));
        })
        .unwrap()
        .join()
        .unwrap();
}
