//! Linux apps from XDG desktop entries: which are running, their icons, and launching one.
//!
//! RUNNING: a process of the current user whose executable is the program of an installed,
//! displayable desktop entry. The id is the process's lowercase executable file name, the key the
//! focus observer and per-app volume use; the name is the entry's `Name`. This is
//! "apps you could build a profile for", not "apps with a window": neither X11 nor Wayland gives a
//! window list that works on both, and a desktop entry is what makes a process an app. Helper
//! processes and apps without an entry are left out.
//!
//! ICONS: the entry's `Icon`, as a PNG only: an absolute path, or `<name>.png` in the user and
//! system icon themes (hicolor first) at the first size that exists, preferring the smallest at
//! least 64 px. SVG-only icons give `None` (no rasterizer ships), and the webview shows a letter.
//!
//! LAUNCH: a desktop entry (by file, or by id or name) through `gio launch`, which runs the entry's
//! `Exec` with its own field-code handling, else the target as a program on `PATH`. Never a shell.
//!
//! Everything here reads small files and runs on a worker thread, never the device thread. The
//! entry index is cached for a minute.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::proc;
use crate::platform::apps::RunningApp;
use crate::platform::foreground::is_own_app;
use crate::platform::ActionError;

/// The most desktop entries read, and the largest one read.
const MAX_ENTRIES: usize = 4000;
const MAX_ENTRY_BYTES: u64 = 64 * 1024;
/// The largest icon file read. It goes to the webview as a `data:` URL.
const MAX_ICON_BYTES: u64 = 512 * 1024;
const INDEX_TTL: Duration = Duration::from_secs(60);
/// Icon sizes in the order tried: the smallest at least 64 px first, then the larger ones, then the
/// smaller ones from large to small.
const SIZES: [&str; 10] = [
    "64x64", "96x96", "128x128", "256x256", "512x512", "48x48", "32x32", "24x24", "22x22", "16x16",
];

/// One displayable `Type=Application` desktop entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DesktopEntry {
    pub path: PathBuf,
    /// The file stem: `org.gnome.Calculator`, `firefox`.
    pub id: String,
    pub name: String,
    /// Lowercase file name of the program `Exec` runs, wrappers like `env` skipped.
    pub program: Option<String>,
    pub icon: Option<String>,
    /// Lowercase `StartupWMClass`.
    pub wm_class: Option<String>,
}

/// Splits `Exec=` into words the way the Desktop Entry spec does: double quotes group, backslash
/// escapes inside them, field codes (`%u`, `%F`...) are dropped, and `%%` is a literal percent.
fn exec_words(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    word.push(next);
                    in_word = true;
                }
            }
            '%' => {
                // `%%` is a literal percent; any other field code stands for files or URLs that
                // Kivori does not pass.
                if chars.next() == Some('%') {
                    word.push('%');
                    in_word = true;
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// The lowercase file name of the program an `Exec` line runs. `env VAR=x prog` and a bare
/// `VAR=x prog` prefix are looked through.
pub(super) fn exec_program(exec: &str) -> Option<String> {
    let mut words = exec_words(exec).into_iter();
    let mut program = words.next()?;
    if proc::exe_basename(&program).as_deref() == Some("env") {
        loop {
            program = words.next()?;
            if !program.contains('=') && !program.starts_with('-') {
                break;
            }
        }
    }
    proc::exe_basename(&program).map(|p| p.to_lowercase())
}

/// Parses one desktop entry. `None` for anything that is not an application a user can start:
/// another `Type`, `NoDisplay`, `Hidden`, or no `Name`.
pub(super) fn parse_desktop_entry(path: &Path, text: &str) -> Option<DesktopEntry> {
    let mut in_group = false;
    let (mut name, mut exec, mut icon, mut wm_class) = (None, None, None, None);
    let (mut kind, mut hidden) = (None, false);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Type" => kind = Some(value.to_string()),
            "Name" => name = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "Icon" => icon = Some(value.to_string()),
            "StartupWMClass" => wm_class = Some(value.to_lowercase()),
            "NoDisplay" | "Hidden" if value == "true" => hidden = true,
            _ => {}
        }
    }
    if kind.as_deref() != Some("Application") || hidden {
        return None;
    }
    let name = name.filter(|n| !n.is_empty())?;
    let id = path.file_stem()?.to_string_lossy().into_owned();
    Some(DesktopEntry {
        path: path.to_path_buf(),
        id,
        name,
        program: exec.as_deref().and_then(exec_program),
        icon: icon.filter(|i| !i.is_empty()),
        wm_class: wm_class.filter(|c| !c.is_empty()),
    })
}

/// The directories desktop entries live in, in precedence order: the user's, then
/// `XDG_DATA_DIRS`, then where Flatpak and Snap export theirs.
fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => dirs.push(PathBuf::from(dir)),
        None => dirs.extend(home.as_ref().map(|h| h.join(".local/share"))),
    }
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    dirs.extend(
        system
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    dirs.extend(home.map(|h| h.join(".local/share/flatpak/exports/share")));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop"));
    dirs
}

/// Every displayable entry under `dirs/applications`, the first directory winning on a repeated id
/// (a user's override of a system entry).
fn load_entries(dirs: &[PathBuf]) -> Vec<DesktopEntry> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();
    for dir in dirs {
        let Ok(read) = std::fs::read_dir(dir.join("applications")) else {
            continue;
        };
        let mut files: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
            .collect();
        files.sort();
        for path in files {
            if entries.len() >= MAX_ENTRIES {
                return entries;
            }
            let Some(id) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            if seen.contains(&id) {
                continue;
            }
            // Mark the id even when this copy is hidden: a hidden override hides the system entry.
            seen.insert(id);
            if std::fs::metadata(&path).map_or(true, |m| m.len() > MAX_ENTRY_BYTES) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(entry) = parse_desktop_entry(&path, &text) {
                entries.push(entry);
            }
        }
    }
    entries
}

type Index = (Instant, Arc<Vec<DesktopEntry>>);
static INDEX: Mutex<Option<Index>> = Mutex::new(None);

fn entries() -> Arc<Vec<DesktopEntry>> {
    let mut slot = INDEX.lock().expect("desktop entry index");
    match slot.as_ref() {
        Some((at, entries)) if at.elapsed() < INDEX_TTL => Arc::clone(entries),
        _ => {
            let fresh = Arc::new(load_entries(&data_dirs()));
            *slot = Some((Instant::now(), Arc::clone(&fresh)));
            fresh
        }
    }
}

/// The entry a program or app id belongs to: its `Exec` program, `StartupWMClass` or file stem.
fn entry_for<'a>(entries: &'a [DesktopEntry], name: &str) -> Option<&'a DesktopEntry> {
    let name = name.to_lowercase();
    entries
        .iter()
        .find(|e| e.program.as_deref() == Some(name.as_str()))
        .or_else(|| {
            entries
                .iter()
                .find(|e| e.wm_class.as_deref() == Some(name.as_str()))
        })
        .or_else(|| entries.iter().find(|e| e.id.to_lowercase() == name))
}

/// Apps running now: processes of this user that match a desktop entry, Kivori itself left out.
#[must_use]
pub fn running_apps() -> Vec<RunningApp> {
    let entries = entries();
    // SAFETY: `getuid` has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let Ok(read) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut apps: Vec<RunningApp> = Vec::new();
    for pid in read
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
    {
        if std::fs::metadata(format!("/proc/{pid}")).map_or(true, |m| m.uid() != uid) {
            continue;
        }
        // Only the executable counts: a shell script that runs an app has the script's name as
        // `comm`, but the app's window belongs to the child, which is listed on its own.
        let Some(id) = proc::exe_name(pid).map(|n| n.to_lowercase()) else {
            continue;
        };
        let Some(entry) = entry_for(&entries, &id) else {
            continue;
        };
        if !is_own_app(&id) && !apps.iter().any(|a| a.id == id) {
            apps.push(RunningApp {
                id,
                name: entry.name.clone(),
            });
        }
    }
    apps
}

/// Theme directories to search for an icon: hicolor first, then every other theme.
fn theme_dirs(icon_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut themes: Vec<PathBuf> = Vec::new();
    for root in icon_roots {
        let hicolor = root.join("hicolor");
        if hicolor.is_dir() {
            themes.push(hicolor);
        }
    }
    for root in icon_roots {
        let Ok(read) = std::fs::read_dir(root) else {
            continue;
        };
        let mut others: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "hicolor"))
            .collect();
        others.sort();
        // Bounded: a machine with a hundred themes still costs a fixed number of lookups.
        themes.extend(others.into_iter().take(16));
    }
    themes
}

fn icon_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".icons"));
    }
    roots.extend(data_dirs().into_iter().map(|d| d.join("icons")));
    roots
}

fn read_png(path: &Path) -> Option<Vec<u8>> {
    if path.extension().is_none_or(|e| e != "png") {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_ICON_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    bytes
        .starts_with(&[0x89, b'P', b'N', b'G'])
        .then_some(bytes)
}

/// The PNG of the icon called `icon` (a theme name or an absolute path), looked up in `roots`.
fn find_icon(icon: &str, roots: &[PathBuf]) -> Option<Vec<u8>> {
    if icon.starts_with('/') {
        return read_png(Path::new(icon));
    }
    if icon.is_empty() || icon.contains('/') || icon.contains("..") {
        return None;
    }
    let file = format!("{icon}.png");
    for theme in theme_dirs(roots) {
        for size in SIZES {
            if let Some(png) = read_png(&theme.join(size).join("apps").join(&file)) {
                return Some(png);
            }
        }
    }
    // Pixmaps and flat icon directories: `<root>/<name>.png`.
    for root in roots {
        let parent = root.parent().map(|p| p.join("pixmaps"));
        for dir in [Some(root.clone()), parent].into_iter().flatten() {
            if let Some(png) = read_png(&dir.join(&file)) {
                return Some(png);
            }
        }
    }
    None
}

/// The icon of the app with id `id` (lowercase executable name) as PNG bytes; `None` when it has
/// no desktop entry or no PNG icon.
#[must_use]
pub fn icon_png(id: &str) -> Option<Vec<u8>> {
    let entries = entries();
    let roots = icon_roots();
    // An app without an entry may still ship an icon named like its program.
    let name = entry_for(&entries, id)
        .and_then(|entry| entry.icon.clone())
        .unwrap_or_else(|| id.to_string());
    find_icon(&name, &roots)
}

/// What to run for a launch target.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Launch {
    /// A desktop entry file, run by `gio launch`.
    Entry(PathBuf),
    /// A program on `PATH`, or a path to one.
    Program(String),
}

/// Decides how to launch `target` (already validated): a `.desktop` file path, else the entry whose
/// id or name equals it (case-insensitively), else the program itself.
fn plan_launch(target: &str, entries: &[DesktopEntry]) -> Launch {
    let path = Path::new(target);
    if path.extension().is_some_and(|e| e == "desktop") && path.is_file() {
        return Launch::Entry(path.to_path_buf());
    }
    let lower = target.to_lowercase();
    let lower = lower.strip_suffix(".desktop").unwrap_or(&lower);
    entries
        .iter()
        .find(|e| e.id.to_lowercase() == lower || e.name.to_lowercase() == lower)
        .map_or_else(
            || Launch::Program(target.to_string()),
            |e| Launch::Entry(e.path.clone()),
        )
}

fn run(command: &mut Command) -> Result<(), ActionError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command
        .status()
        .map_err(|e| ActionError::Failed(e.kind().to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(ActionError::Failed("application not found".into()))
    }
}

/// Starts `target`. Success means the OS accepted the launch (Execution Confirmed): `gio launch`
/// exits zero once the entry's command was spawned, a program is spawned directly.
///
/// # Errors
/// [`ActionError::Failed`] when nothing could be started.
pub fn launch_target(target: &str) -> Result<(), ActionError> {
    match plan_launch(target, &entries()) {
        Launch::Entry(path) => run(Command::new("gio").arg("launch").arg(path)),
        Launch::Program(program) => Command::new(program)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
            .map_err(|e| ActionError::Failed(e.kind().to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str =
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName=Firefox Web Browser\n\
        Name[de]=Firefox Webbrowser\nExec=env MOZ_ENABLE_WAYLAND=1 /usr/lib/firefox/firefox %u\n\
        Icon=firefox\nStartupWMClass=Firefox\nCategories=Network;\n\
        [Desktop Action new-window]\nName=New Window\nExec=firefox --new-window\n";

    fn entry(path: &str, text: &str) -> DesktopEntry {
        parse_desktop_entry(Path::new(path), text).expect("an application entry")
    }

    #[test]
    fn exec_words_handle_quotes_escapes_and_field_codes() {
        assert_eq!(
            exec_words(r#"/opt/My App/run --flag "a b" %U"#),
            ["/opt/My", "App/run", "--flag", "a b"]
        );
        assert_eq!(
            exec_words(r#""/opt/My App/run" --x=%f 100%%"#),
            ["/opt/My App/run", "--x=", "100%"]
        );
        assert_eq!(exec_words(r#""say \"hi\"""#), [r#"say "hi""#]);
        assert!(exec_words("   ").is_empty());
    }

    #[test]
    fn the_program_is_the_lowercase_file_name_looking_through_env() {
        assert_eq!(exec_program("firefox %u"), Some("firefox".into()));
        assert_eq!(
            exec_program("/usr/share/code/Code --unity-launch %F"),
            Some("code".into())
        );
        assert_eq!(
            exec_program("env MOZ_ENABLE_WAYLAND=1 /usr/lib/firefox/firefox %u"),
            Some("firefox".into())
        );
        assert_eq!(exec_program("env -i FOO=1 spotify"), Some("spotify".into()));
        assert_eq!(exec_program(""), None);
        assert_eq!(exec_program("env A=b"), None);
    }

    #[test]
    fn an_application_entry_parses_from_its_main_group_only() {
        let e = entry("/usr/share/applications/firefox.desktop", FIREFOX);
        assert_eq!(e.id, "firefox");
        assert_eq!(e.name, "Firefox Web Browser", "the unlocalised name");
        assert_eq!(e.program.as_deref(), Some("firefox"));
        assert_eq!(e.icon.as_deref(), Some("firefox"));
        assert_eq!(e.wm_class.as_deref(), Some("firefox"));
    }

    #[test]
    fn entries_that_are_not_user_launchable_apps_are_skipped() {
        let p = Path::new("/x/a.desktop");
        for text in [
            "[Desktop Entry]\nType=Application\nName=A\nNoDisplay=true\n",
            "[Desktop Entry]\nType=Application\nName=A\nHidden=true\n",
            "[Desktop Entry]\nType=Link\nName=A\nURL=https://example.org\n",
            "[Desktop Entry]\nType=Application\nExec=a\n",
            "[Other Group]\nType=Application\nName=A\n",
        ] {
            assert_eq!(parse_desktop_entry(p, text), None, "{text}");
        }
    }

    #[test]
    fn an_app_is_found_by_program_wm_class_or_file_stem() {
        let entries = vec![
            entry("/a/firefox.desktop", FIREFOX),
            entry(
                "/a/org.gnome.Calculator.desktop",
                "[Desktop Entry]\nType=Application\nName=Calculator\nExec=gnome-calculator\n",
            ),
        ];
        assert_eq!(entry_for(&entries, "firefox").unwrap().id, "firefox");
        assert_eq!(
            entry_for(&entries, "GNOME-Calculator").unwrap().name,
            "Calculator"
        );
        assert_eq!(
            entry_for(&entries, "org.gnome.calculator").unwrap().name,
            "Calculator"
        );
        assert!(entry_for(&entries, "nothing").is_none());
    }

    #[test]
    fn a_user_override_hides_the_system_entry_even_when_it_hides_the_app() {
        let tmp = tempfile::tempdir().unwrap();
        let (user, system) = (tmp.path().join("user"), tmp.path().join("system"));
        for dir in [&user, &system] {
            std::fs::create_dir_all(dir.join("applications")).unwrap();
        }
        let write = |dir: &Path, text: &str| {
            std::fs::write(dir.join("applications/app.desktop"), text).unwrap();
        };
        write(
            &system,
            "[Desktop Entry]\nType=Application\nName=System App\nExec=app\n",
        );
        write(
            &user,
            "[Desktop Entry]\nType=Application\nName=User App\nExec=app\nHidden=true\n",
        );
        assert!(load_entries(&[user.clone(), system.clone()]).is_empty());
        write(
            &user,
            "[Desktop Entry]\nType=Application\nName=User App\nExec=app\n",
        );
        let found = load_entries(&[user, system]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "User App");
    }

    #[test]
    fn launch_prefers_an_entry_file_then_an_entry_by_name_then_the_program() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("custom.desktop");
        std::fs::write(&file, "[Desktop Entry]\nType=Application\nName=C\nExec=c\n").unwrap();
        let entries = vec![entry("/a/firefox.desktop", FIREFOX)];
        assert_eq!(
            plan_launch(file.to_str().unwrap(), &entries),
            Launch::Entry(file)
        );
        for target in [
            "firefox",
            "Firefox",
            "firefox.desktop",
            "FIREFOX WEB BROWSER",
        ] {
            assert_eq!(
                plan_launch(target, &entries),
                Launch::Entry(PathBuf::from("/a/firefox.desktop")),
                "{target}"
            );
        }
        assert_eq!(
            plan_launch("/usr/bin/true", &entries),
            Launch::Program("/usr/bin/true".into())
        );
        assert_eq!(
            plan_launch("missing.desktop", &entries),
            Launch::Program("missing.desktop".into()),
            "a .desktop path that is not a file is not an entry"
        );
    }

    #[test]
    fn a_program_that_does_not_exist_is_a_known_failure() {
        assert!(matches!(
            launch_target("kivori-definitely-not-a-program-7f3a"),
            Err(ActionError::Failed(_))
        ));
    }

    fn png(dir: &Path, relative: &str) -> PathBuf {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
        path
    }

    #[test]
    fn icons_prefer_the_smallest_size_of_at_least_64_and_hicolor() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        png(&root, "hicolor/48x48/apps/tool.png");
        png(&root, "hicolor/256x256/apps/tool.png");
        png(&root, "Yaru/64x64/apps/tool.png");
        let found = find_icon("tool", std::slice::from_ref(&root)).unwrap();
        assert_eq!(found[..4], [0x89, b'P', b'N', b'G']);
        // hicolor is searched first, and within it 256 beats 48 (>= 64 wins).
        let hicolor_256 = png(&root, "hicolor/256x256/apps/tool.png");
        std::fs::write(&hicolor_256, [0x89, b'P', b'N', b'G', 9]).unwrap();
        assert_eq!(find_icon("tool", &[root]).unwrap().last(), Some(&9));
    }

    #[test]
    fn icons_by_absolute_path_and_pixmaps_work_and_others_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let abs = png(tmp.path(), "own/icon.png");
        assert!(find_icon(abs.to_str().unwrap(), &[]).is_some());
        let share = tmp.path().join("share");
        png(&share, "pixmaps/legacy.png");
        assert!(find_icon("legacy", &[share.join("icons")]).is_some());
        // Not a PNG, an SVG, a traversal and an unknown name give nothing.
        let fake = tmp.path().join("fake.png");
        std::fs::write(&fake, b"<svg/>").unwrap();
        assert!(find_icon(fake.to_str().unwrap(), &[]).is_none());
        assert!(find_icon("/etc/passwd", &[]).is_none());
        assert!(find_icon("../escape", &[tmp.path().to_path_buf()]).is_none());
        assert!(find_icon("nothing-here", &[tmp.path().to_path_buf()]).is_none());
    }

    /// Real desktop entries and /proc: `cargo test -p kivori-desktop -- --ignored --nocapture linux_apps_now`.
    #[test]
    #[ignore = "reads the live running apps and installed desktop entries"]
    fn linux_apps_now() {
        let apps = running_apps();
        println!("running: {apps:?}");
        println!("entries: {}", entries().len());
        for app in apps.iter().take(3) {
            println!(
                "icon {}: {:?} bytes",
                app.id,
                icon_png(&app.id).map(|p| p.len())
            );
        }
    }
}
