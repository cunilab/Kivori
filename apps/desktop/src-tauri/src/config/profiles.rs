//! Edits to the user's profiles and the apps they match: the rules that keep one app in exactly
//! one profile. Pure functions over [`ConfigFile`]; the store validates and saves the result.
//!
//! A built-in profile's app list is its built-in ids, minus `removed_apps`, plus `added_apps`. A
//! custom profile lists its own `apps`. Resolution lets any explicit claim win over a built-in id,
//! so moving a built-in app to another profile only needs the claim there.

use super::resolve::{app_id, profile_name};
use super::schema::{
    ConfigFile, CustomProfile, ProfileId, ProfileOverride, ProfileRef, MAX_CUSTOM_PROFILES,
    MAX_PROFILE_APPS, PROFILE_NAME_MAX,
};
use crate::desk::profile::builtins;

/// The lowest `custom-<n>` id not in use.
fn next_id(file: &ConfigFile) -> String {
    (1..)
        .map(|n| format!("custom-{n}"))
        .find(|id| file.custom_profiles.iter().all(|p| p.id != *id))
        .expect("an unused id exists")
}

/// A profile name for an app: its display name, cut to the panel's limit with an ellipsis, or the
/// app id without `.exe` when it has none.
#[must_use]
pub fn name_for_app(name: &str, app: &str) -> String {
    let flat = |text: &str| {
        text.split_whitespace()
            .filter(|word| !word.chars().any(char::is_control))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let name = flat(name);
    let name = if name.is_empty() {
        flat(app.strip_suffix(".exe").unwrap_or(app))
    } else {
        name
    };
    if name.chars().count() > PROFILE_NAME_MAX {
        let mut cut: String = name.chars().take(PROFILE_NAME_MAX - 1).collect();
        cut.push('\u{2026}');
        cut
    } else {
        name
    }
}

fn custom_mut<'a>(
    file: &'a mut ConfigFile,
    id: &str,
) -> Result<&'a mut CustomProfile, &'static str> {
    file.custom_profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("unknown profile")
}

/// Applies `edit` to `profile`'s bindings. A built-in profile keeps sparse overrides (`resetting`
/// removes one, and an empty entry is dropped); a custom profile always holds every slot, so a
/// single control cannot be reset, only the whole profile.
///
/// # Errors
/// A fixed reason for an unknown profile or for resetting one control of a custom profile.
pub fn edit_bindings(
    file: &mut ConfigFile,
    profile: &ProfileRef,
    resetting: bool,
    edit: impl FnOnce(&mut ProfileOverride),
) -> Result<(), &'static str> {
    match profile {
        ProfileRef::Builtin(id) => {
            let over = file.profiles.entry(*id).or_default();
            edit(over);
            if over.is_empty() {
                file.profiles.remove(id);
            }
        }
        ProfileRef::Custom(_) if resetting => {
            return Err("a custom profile is reset as a whole, back to General");
        }
        ProfileRef::Custom(id) => edit(&mut custom_mut(file, id)?.bindings),
    }
    Ok(())
}

/// Moves `app` into `target`, out of whichever profile held it. Moving to General takes it out of
/// the built-in profile that lists it by default (an app only a custom profile held simply
/// returns to wherever it would match without it).
///
/// # Errors
/// A fixed reason for an invalid app, an unknown profile, or a profile with too many apps.
pub fn assign_app(
    file: &mut ConfigFile,
    target: &ProfileRef,
    app: &str,
) -> Result<(), &'static str> {
    let app = app_id(app)?;
    if let ProfileRef::Custom(id) = target {
        custom_mut(file, id)?;
    }
    for over in file.profiles.values_mut() {
        over.added_apps.retain(|a| *a != app);
        over.removed_apps.retain(|a| *a != app);
    }
    for custom in &mut file.custom_profiles {
        custom.apps.retain(|a| *a != app);
    }
    let default_owner = builtins()
        .into_iter()
        .find(|p| p.ids.contains(&app))
        .and_then(|p| match p.id {
            ProfileRef::Builtin(id) => Some(id),
            ProfileRef::Custom(_) => None,
        });
    match target {
        ProfileRef::Builtin(ProfileId::General) => {
            if let Some(owner) = default_owner {
                file.profiles
                    .entry(owner)
                    .or_default()
                    .removed_apps
                    .push(app);
            }
        }
        ProfileRef::Builtin(id) if default_owner == Some(*id) => {}
        ProfileRef::Builtin(id) => {
            let over = file.profiles.entry(*id).or_default();
            if over.added_apps.len() >= MAX_PROFILE_APPS {
                return Err("a profile lists at most 32 apps");
            }
            over.added_apps.push(app);
        }
        ProfileRef::Custom(id) => {
            let custom = custom_mut(file, id)?;
            if custom.apps.len() >= MAX_PROFILE_APPS {
                return Err("a profile lists at most 32 apps");
            }
            custom.apps.push(app);
        }
    }
    file.profiles.retain(|_, over| !over.is_empty());
    Ok(())
}

/// Creates a custom profile for `app` (moving it from any profile that held it) with `bindings`
/// (a copy of General's, made by the caller). Returns the new profile's id.
///
/// # Errors
/// A fixed reason for an invalid app or name, or when 16 custom profiles exist.
pub fn add_custom_profile(
    file: &mut ConfigFile,
    app: &str,
    name: &str,
    bindings: ProfileOverride,
) -> Result<String, &'static str> {
    if file.custom_profiles.len() >= MAX_CUSTOM_PROFILES {
        return Err("at most 16 custom profiles");
    }
    let app = app_id(app)?;
    let id = next_id(file);
    file.custom_profiles.push(CustomProfile {
        id: id.clone(),
        name: profile_name(&name_for_app(name, &app))?,
        apps: Vec::new(),
        in_pin_cycle: false,
        bindings,
    });
    assign_app(file, &ProfileRef::Custom(id.clone()), &app)?;
    Ok(id)
}

/// Deletes a custom profile; its apps stop matching it.
///
/// # Errors
/// A fixed reason for an unknown or built-in profile.
pub fn delete_profile(file: &mut ConfigFile, id: &ProfileRef) -> Result<(), &'static str> {
    let ProfileRef::Custom(id) = id else {
        return Err("a built-in profile cannot be deleted");
    };
    custom_mut(file, id)?;
    file.custom_profiles.retain(|p| p.id != *id);
    Ok(())
}

/// Renames a custom profile.
///
/// # Errors
/// A fixed reason for an invalid name, or an unknown or built-in profile.
pub fn rename_profile(
    file: &mut ConfigFile,
    id: &ProfileRef,
    name: &str,
) -> Result<(), &'static str> {
    let ProfileRef::Custom(id) = id else {
        return Err("a built-in profile keeps its name");
    };
    let name = profile_name(name)?;
    custom_mut(file, id)?.name = name;
    Ok(())
}

/// Puts a custom profile in or out of the middle button's pin cycle.
///
/// # Errors
/// A fixed reason for an unknown or built-in profile (built-ins are always in the cycle).
pub fn set_pin_cycle(
    file: &mut ConfigFile,
    id: &ProfileRef,
    in_pin_cycle: bool,
) -> Result<(), &'static str> {
    let ProfileRef::Custom(id) = id else {
        return Err("a built-in profile is always in the pin cycle");
    };
    custom_mut(file, id)?.in_pin_cycle = in_pin_cycle;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_custom(file: &mut ConfigFile, app: &str) -> ProfileRef {
        let id = add_custom_profile(file, app, "", ProfileOverride::default()).unwrap();
        ProfileRef::Custom(id)
    }

    #[test]
    fn names_follow_the_app_and_never_pass_the_panel_limit() {
        assert_eq!(name_for_app("Figma", "figma.exe"), "Figma");
        assert_eq!(name_for_app("  ", "figma.exe"), "figma");
        assert_eq!(name_for_app("A   B", "x"), "A B");
        let long = "x".repeat(40);
        let cut = name_for_app(&long, "x");
        assert_eq!(cut.chars().count(), 32);
        assert!(cut.ends_with('\u{2026}'));
    }

    #[test]
    fn a_custom_profile_takes_the_app_and_ids_are_reused_lowest_first() {
        let mut file = ConfigFile::default();
        let a = with_custom(&mut file, "Figma.exe");
        let b = with_custom(&mut file, "obs64.exe");
        assert_eq!((a.token(), b.token()), ("custom-1", "custom-2"));
        assert_eq!(file.custom_profiles[0].apps, ["figma.exe"]);
        delete_profile(&mut file, &a).unwrap();
        assert_eq!(with_custom(&mut file, "x.exe").token(), "custom-1");
    }

    #[test]
    fn one_app_belongs_to_one_profile() {
        let mut file = ConfigFile::default();
        let a = with_custom(&mut file, "figma.exe");
        let b = with_custom(&mut file, "obs64.exe");
        assign_app(&mut file, &b, "FIGMA.exe").unwrap();
        assert!(file.custom_profiles[0].apps.is_empty());
        assert_eq!(file.custom_profiles[1].apps, ["obs64.exe", "figma.exe"]);
        assign_app(
            &mut file,
            &ProfileRef::Builtin(ProfileId::Code),
            "figma.exe",
        )
        .unwrap();
        assert!(file.custom_profiles[1].apps == ["obs64.exe"]);
        assert_eq!(file.profiles[&ProfileId::Code].added_apps, ["figma.exe"]);
        // General drops it again.
        assign_app(
            &mut file,
            &ProfileRef::Builtin(ProfileId::General),
            "figma.exe",
        )
        .unwrap();
        assert!(file.profiles.is_empty());
        let _ = a;
    }

    #[test]
    fn a_built_in_app_moves_out_and_back() {
        let mut file = ConfigFile::default();
        let code = ProfileRef::Builtin(ProfileId::Code);
        let general = ProfileRef::Builtin(ProfileId::General);
        let browser = ProfileRef::Builtin(ProfileId::Browser);
        // chrome.exe leaves Browser for Code, then for General, then comes home.
        assign_app(&mut file, &code, "chrome.exe").unwrap();
        assert_eq!(file.profiles[&ProfileId::Code].added_apps, ["chrome.exe"]);
        assign_app(&mut file, &general, "chrome.exe").unwrap();
        assert!(!file.profiles.contains_key(&ProfileId::Code));
        assert_eq!(
            file.profiles[&ProfileId::Browser].removed_apps,
            ["chrome.exe"]
        );
        assign_app(&mut file, &browser, "chrome.exe").unwrap();
        assert!(
            file.profiles.is_empty(),
            "back to the built-in list: nothing stored"
        );
    }

    #[test]
    fn limits_and_bad_input_are_refused_with_fixed_reasons() {
        let mut file = ConfigFile::default();
        for n in 0..MAX_CUSTOM_PROFILES {
            with_custom(&mut file, &format!("app{n}.exe"));
        }
        assert_eq!(
            add_custom_profile(&mut file, "one-more.exe", "", ProfileOverride::default()),
            Err("at most 16 custom profiles")
        );
        let id = ProfileRef::Custom("custom-1".into());
        for n in 0..MAX_PROFILE_APPS - 1 {
            assign_app(&mut file, &id, &format!("more{n}.exe")).unwrap();
        }
        assert_eq!(
            assign_app(&mut file, &id, "overflow.exe"),
            Err("a profile lists at most 32 apps")
        );
        assert!(assign_app(&mut file, &id, "").is_err());
        assert!(assign_app(&mut file, &id, r"C:\x\y.exe").is_err());
        assert_eq!(
            assign_app(&mut file, &ProfileRef::Custom("custom-99".into()), "a.exe"),
            Err("unknown profile")
        );
    }

    #[test]
    fn rename_validates_and_built_ins_are_fixed() {
        let mut file = ConfigFile::default();
        let id = with_custom(&mut file, "figma.exe");
        rename_profile(&mut file, &id, "  Design  ").unwrap();
        assert_eq!(file.custom_profiles[0].name, "Design");
        for bad in ["", "   ", &"x".repeat(33), "a\nb"] {
            assert!(rename_profile(&mut file, &id, bad).is_err(), "{bad:?}");
        }
        assert!(rename_profile(&mut file, &id, &"x".repeat(32)).is_ok());
        let code = ProfileRef::Builtin(ProfileId::Code);
        assert!(rename_profile(&mut file, &code, "Mine").is_err());
        assert!(delete_profile(&mut file, &code).is_err());
        assert!(set_pin_cycle(&mut file, &code, true).is_err());
        set_pin_cycle(&mut file, &id, true).unwrap();
        assert!(file.custom_profiles[0].in_pin_cycle);
    }
}
