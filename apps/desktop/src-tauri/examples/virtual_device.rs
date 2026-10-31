//! Opt-in development app. It registers only sandbox commands, never production app commands.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kivori_desktop::ipc::dto::DeviceInternalsDto;
use kivori_desktop::virtual_device::{Sandbox, Snapshot};

struct State(Mutex<Sandbox>);

#[tauri::command]
fn virtual_snapshot(state: tauri::State<'_, State>) -> Result<Snapshot, String> {
    state
        .0
        .lock()
        .map(|sandbox| sandbox.snapshot())
        .map_err(|_| "Virtual device unavailable".into())
}

/// Same name and shape as the shipping app's command, so the Device internals card is reused as is.
#[tauri::command]
fn get_device_internals(state: tauri::State<'_, State>) -> Result<DeviceInternalsDto, String> {
    state
        .0
        .lock()
        .map(|mut sandbox| sandbox.device_internals())
        .map_err(|_| "Virtual device unavailable".into())
}

#[tauri::command]
fn virtual_input(
    control: String,
    generation: u32,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    state
        .0
        .lock()
        .map_err(|_| "Virtual device unavailable")?
        .input_for_generation(&control, generation)
}

#[tauri::command]
fn virtual_connection(action: String, state: tauri::State<'_, State>) -> Result<(), String> {
    state
        .0
        .lock()
        .map_err(|_| "Virtual device unavailable")?
        .connection(&action)
}

fn settle(sandbox: &mut Sandbox, duration: Duration) -> Result<(), String> {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        sandbox.tick();
        if let Some(error) = sandbox.snapshot().error {
            return Err(error);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn wait_for(
    sandbox: &mut Sandbox,
    description: &str,
    ready: impl Fn(&Snapshot) -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        sandbox.tick();
        let snapshot = sandbox.snapshot();
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        if ready(&snapshot) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out waiting for {description}: state={}, detents={}, volume={}, media={}",
                snapshot.connection_state,
                snapshot.detents,
                snapshot.volume,
                snapshot.media_triggers
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Asks for the Device internals card's data until the modelled device has answered with figures.
fn wait_for_internals(sandbox: &mut Sandbox) -> Result<DeviceInternalsDto, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        sandbox.tick();
        if let Some(error) = sandbox.snapshot().error {
            return Err(error);
        }
        let internals = sandbox.device_internals();
        if internals
            .snapshot
            .as_ref()
            .is_some_and(|s| s.compose_ms.is_some() && s.fps.is_some())
        {
            return Ok(internals);
        }
        if Instant::now() >= deadline {
            return Err("Timed out waiting for the device internals".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn smoke(mut sandbox: Sandbox, output: PathBuf) -> Result<(), String> {
    wait_for(&mut sandbox, "firmware handshake/frame", |s| {
        s.connected && s.frame.is_some()
    })?;
    let initial = sandbox.snapshot();
    for detents in 1..=3 {
        sandbox.input("clockwise")?;
        wait_for(&mut sandbox, "exact detent/volume result", |s| {
            s.detents == detents && s.volume == 50 + detents as u8 * 2
        })?;
    }
    // Canonical overlay: 188px inner width, solid observed fill at 56%, unlike hollow preview.
    let filled = 188 * 56 / 100;
    wait_for(&mut sandbox, "confirmed firmware volume pixels", |s| {
        s.frame.as_ref().is_some_and(|frame| {
            (26..26 + filled).all(|x| frame[185 * 240 + x] == 0xffff)
                && frame[185 * 240 + 26 + filled] != 0xffff
        })
    })?;
    // Rotary ownership must expire before the independent knob press begins.
    settle(&mut sandbox, Duration::from_millis(300))?;
    sandbox.input("press")?;
    settle(&mut sandbox, Duration::from_millis(120))?;
    sandbox.input("release")?;
    wait_for(&mut sandbox, "fake media action", |s| s.media_triggers == 1)?;
    settle(&mut sandbox, Duration::from_millis(250))?;
    let result = sandbox.snapshot();
    if result.media_triggers != 1 || result.detents != 3 || result.volume != 56 {
        return Err(format!(
            "Expected volume56, three detents and one media trigger; got {}, {}, {}",
            result.volume, result.detents, result.media_triggers
        ));
    }
    // The Device internals card reads the modelled chip figures through the same session path.
    let internals = wait_for_internals(&mut sandbox)?;
    let chip = internals
        .snapshot
        .as_ref()
        .ok_or("Device internals missing")?;
    if !internals.connected
        || !internals.supported
        || chip.chip_model != Some("esp32c3")
        || chip.busy_percent.is_none()
        || !chip.compose_ms.is_some_and(|ms| (20..=200).contains(&ms))
    {
        return Err(format!("Unexpected device internals: {internals:?}"));
    }
    let frame = result.frame.ok_or("Firmware frame missing")?;
    if Some(&frame) == initial.frame.as_ref() {
        return Err("Firmware frame did not change".into());
    }
    let mut ppm = b"P6\n240 240\n255\n".to_vec();
    for pixel in frame {
        ppm.extend_from_slice(&[
            ((pixel >> 11 & 31) * 255 / 31) as u8,
            ((pixel >> 5 & 63) * 255 / 63) as u8,
            ((pixel & 31) * 255 / 31) as u8,
        ]);
    }
    std::fs::write(output, ppm).map_err(|error| error.to_string())?;
    println!("PASS KIVORI-VIRTUAL-DEVICE: real firmware handshake, three raw detents → fake volume56, raw press/release → one fake media trigger; modelled device internals read back; firmware frame captured. Simulated device/OS, real elapsed time; no physical evidence.");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let runner = PathBuf::from(
        args.next()
            .ok_or("usage: virtual_device <runner> [--smoke <frame.ppm> | --connection-smoke | --desktop-smoke | --recovery-smoke | --presence-smoke | --endurance-smoke]")?,
    );
    let mut connection_smoke = false;
    let mut desktop_smoke = false;
    let mut recovery_smoke = false;
    let mut presence_smoke = false;
    let mut endurance_smoke = false;
    let smoke_output = match args.next() {
        None => None,
        Some(flag) if flag == "--connection-smoke" => {
            connection_smoke = true;
            None
        }
        Some(flag) if flag == "--desktop-smoke" => {
            desktop_smoke = true;
            None
        }
        Some(flag) if flag == "--recovery-smoke" => {
            recovery_smoke = true;
            None
        }
        Some(flag) if flag == "--presence-smoke" => {
            presence_smoke = true;
            None
        }
        Some(flag) if flag == "--endurance-smoke" => {
            endurance_smoke = true;
            None
        }
        Some(flag) if flag == "--smoke" => Some(PathBuf::from(
            args.next().ok_or("missing smoke frame output")?,
        )),
        _ => return Err("unknown argument".into()),
    };
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    if endurance_smoke {
        return kivori_desktop::virtual_device::endurance_smoke(&runner).map_err(Into::into);
    }
    if presence_smoke {
        return kivori_desktop::virtual_device::presence_smoke(&runner).map_err(Into::into);
    }
    if recovery_smoke {
        return kivori_desktop::virtual_device::recovery_smoke(&runner).map_err(Into::into);
    }
    if desktop_smoke {
        let settings = tempfile::tempdir()?;
        return kivori_desktop::virtual_device::desktop_smoke(&runner, settings.path())
            .map_err(Into::into);
    }
    if connection_smoke {
        return kivori_desktop::virtual_device::connection_smoke(&runner).map_err(Into::into);
    }
    let sandbox = Sandbox::open(&runner)?;
    if let Some(output) = smoke_output {
        return smoke(sandbox, output).map_err(Into::into);
    }

    let mut context = kivori_desktop::virtual_device::context();
    context.config_mut().app.windows.clear();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker = Arc::new(Mutex::new(None));
    let setup_worker = worker.clone();
    let run_cancel = cancel.clone();
    let app = tauri::Builder::default()
        .manage(State(Mutex::new(sandbox)))
        .invoke_handler(tauri::generate_handler![
            virtual_snapshot,
            virtual_input,
            virtual_connection,
            get_device_internals
        ])
        .setup(move |app| {
            use tauri::Manager;
            let handle = app.handle().clone();
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("virtual-device.html".into()),
            )
            .title("Kivori — Simulated device / Simulated OS")
            .inner_size(520.0, 720.0)
            .min_inner_size(360.0, 420.0)
            .build()?;
            *setup_worker.lock().expect("worker lock") = Some(
                std::thread::Builder::new()
                    .name("virtual-device".into())
                    .spawn(move || {
                        while !run_cancel.load(Ordering::Relaxed) {
                            handle
                                .state::<State>()
                                .0
                                .lock()
                                .expect("sandbox lock")
                                .tick();
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    })?,
            );
            Ok(())
        })
        .build(context)?;
    app.run(|_, _| {});
    cancel.store(true, Ordering::Relaxed);
    if let Some(worker) = worker.lock().expect("worker lock").take() {
        let _ = worker.join();
    }
    Ok(())
}
