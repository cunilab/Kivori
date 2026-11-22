//! Live GNOME diagnostic; installation requires an explicit --install argument.
#[cfg(target_os = "linux")]
fn main() {
    use kivori_desktop::platform::linux::gnome;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    println!("GNOME focus status: {:?}", gnome::status());
    if std::env::args().any(|arg| arg == "--install") {
        println!("GNOME focus install: {:?}", gnome::install());
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("GNOME focus diagnostics require Linux.");
}
