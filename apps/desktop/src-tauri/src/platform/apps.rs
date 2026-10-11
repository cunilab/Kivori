//! Apps on this computer for the profile list: which are running ("pick a running app") and what
//! their icons look like. Only Windows and macOS have a backend; elsewhere the list is empty and
//! no icon is found, and the webview shows a letter instead.
//!
//! Both calls do OS work that may take a moment, so callers run them off the UI thread and the
//! focus poller (`ipc::commands`). Icons are cached per app id.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::png::png_data_url;

/// An app with a window open now. `id` is the foreground id (Windows: lowercase executable file
/// name; macOS: lowercase bundle id), `name` is for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningApp {
    pub id: String,
    pub name: String,
}

/// The running apps, Kivori itself and protected or unreadable ones left out, by name.
#[must_use]
pub fn running_apps() -> Vec<RunningApp> {
    #[cfg(target_os = "macos")]
    let mut apps = super::macos::apps::running_apps();
    #[cfg(windows)]
    let mut apps = super::windows::apps::running_apps();
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut apps: Vec<RunningApp> = Vec::new();
    apps.sort_by_cached_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.id == b.id);
    apps
}

/// How long a failed lookup is remembered: the app may start later.
const MISS_TTL: Duration = Duration::from_secs(30);
/// The most cached entries; past it the cache starts over.
const CACHE_MAX: usize = 256;

type Cache = HashMap<String, (Option<String>, Instant)>;
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// The app's icon as a PNG `data:` URL, `None` when this OS or app gives none.
#[must_use]
pub fn icon_data_url(id: &str) -> Option<String> {
    let id = id.trim().to_lowercase();
    if id.is_empty()
        || id.len() > 128
        || id.chars().any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return None;
    }
    cached_or(&id, || icon_png(&id).map(|png| png_data_url(&png)))
}

fn cached_or(id: &str, load: impl FnOnce() -> Option<String>) -> Option<String> {
    let hit = |cache: &Option<Cache>| {
        cache
            .as_ref()
            .and_then(|c| c.get(id))
            .filter(|(icon, at)| icon.is_some() || at.elapsed() < MISS_TTL)
            .map(|(icon, _)| icon.clone())
    };
    if let Some(found) = hit(&CACHE.lock().expect("icon cache")) {
        return found;
    }
    // Not under the lock: reading an icon can take a while and must not stall other lookups.
    let icon = load();
    let mut cache = CACHE.lock().expect("icon cache");
    let map = cache.get_or_insert_with(HashMap::new);
    if map.len() >= CACHE_MAX {
        map.clear();
    }
    map.insert(id.to_string(), (icon.clone(), Instant::now()));
    icon
}

fn icon_png(id: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        super::macos::apps::icon_png(id)
    }
    #[cfg(windows)]
    {
        super::windows::apps::icon_png(id)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = id;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_found_icon_is_loaded_once_and_a_miss_is_remembered_briefly() {
        let mut loads = 0;
        let mut load = |icon: Option<&str>| {
            loads += 1;
            icon.map(str::to_string)
        };
        assert_eq!(
            cached_or("test-hit", || load(Some("png"))),
            Some("png".into())
        );
        assert_eq!(
            cached_or("test-hit", || load(Some("other"))),
            Some("png".into())
        );
        assert_eq!(cached_or("test-miss", || load(None)), None);
        assert_eq!(cached_or("test-miss", || load(Some("late"))), None);
        assert_eq!(loads, 2);
    }

    #[test]
    fn ids_that_are_not_app_names_find_nothing() {
        for bad in ["", "   ", r"C:\a\b.exe", "a/b", "x\n"] {
            assert_eq!(icon_data_url(bad), None, "{bad:?}");
        }
    }
}
