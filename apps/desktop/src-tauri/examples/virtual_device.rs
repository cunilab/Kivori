//! Opt-in development app. It registers only sandbox commands, never production app commands.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

fn smoke(mut sandbox: Sandbox, output: PathBuf) -> Result<(), String> {
    settle(&mut sandbox, Duration::from_secs(1))?;
    let initial = sandbox.snapshot();
    if !initial.connected || initial.frame.is_none() {
        return Err("Real firmware handshake/frame missing".into());
    }
    for _ in 0..3 {
        sandbox.input("clockwise")?;
        settle(&mut sandbox, Duration::from_millis(100))?;
    }
    settle(&mut sandbox, Duration::from_millis(400))?;
    let turned = sandbox.snapshot();
    if turned.volume != 56 || turned.detents != 3 {
        return Err(format!(
            "Expected fake volume56 after three detents, got {} ({} firmware detents)",
            turned.volume, turned.detents
        ));
    }
    let volume_frame = turned.frame.as_ref().ok_or("No firmware volume frame")?;
    // Canonical overlay: 188px inner width, solid observed fill at 56%, unlike hollow preview.
    let filled = 188 * 56 / 100;
    if !(26..26 + filled).all(|x| volume_frame[185 * 240 + x] == 0xffff)
        || volume_frame[185 * 240 + 26 + filled] == 0xffff
    {
        return Err("Firmware pixels do not show the confirmed 56% volume overlay".into());
    }
    sandbox.input("press")?;
    settle(&mut sandbox, Duration::from_millis(120))?;
    sandbox.input("release")?;
    settle(&mut sandbox, Duration::from_millis(650))?;
    let result = sandbox.snapshot();
    if result.media_triggers != 1 {
        return Err(format!(
            "Expected exactly one fake media trigger, got {}",
            result.media_triggers
        ));
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
    println!("PASS KIVORI-VIRTUAL-DEVICE: real firmware handshake, three raw detents → fake volume56, raw press/release → one fake media trigger; firmware frame captured. Simulated device/OS, real elapsed time; no physical evidence.");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let runner = PathBuf::from(
        args.next()
            .ok_or("usage: virtual_device <runner> [--smoke <frame.ppm> | --connection-smoke]")?,
    );
    let mut connection_smoke = false;
    let smoke_output = match args.next() {
        None => None,
        Some(flag) if flag == "--connection-smoke" => {
            connection_smoke = true;
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
            virtual_connection
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
