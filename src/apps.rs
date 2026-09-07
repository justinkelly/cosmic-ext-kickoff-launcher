// SPDX-License-Identifier: GPL-3.0-only

use crate::fl;
use freedesktop_desktop_entry as fde;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

/// A parsed desktop application entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplicationEntry {
    /// Desktop file ID (for example, `firefox.desktop`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Lowercase name used by search.
    pub name_lower: String,
    /// Raw desktop-entry executable command.
    pub exec: Option<String>,
    /// Icon name or path.
    pub icon: Option<Arc<str>>,
    /// Categories from the desktop file.
    pub categories: Vec<String>,
    /// Generic name or description.
    pub description: Option<String>,
    /// Lowercase description used by search.
    pub desc_lower: Option<String>,
    /// Lowercased desktop-entry keywords used by menu search.
    pub keywords_lower: Vec<String>,
    /// Whether the app runs in a terminal.
    pub is_terminal: bool,
    /// Prefer the standard `org.freedesktop.Application` activation interface.
    pub dbus_activatable: bool,
    /// Path to the desktop file.
    pub path: PathBuf,
    /// Optional working directory from the desktop entry's `Path` key.
    pub working_dir: Option<PathBuf>,
}

/// A category derived from desktop file Categories keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplicationCategory {
    /// The desktop category key.
    pub key: String,
    /// Localized display name.
    pub display_name: String,
    /// Icon name for the category.
    pub icon_name: String,
}

impl ApplicationCategory {
    pub(crate) fn all() -> Self {
        ApplicationCategory {
            key: "all".to_string(),
            display_name: fl!("cat-all"),
            icon_name: "applications-system-symbolic".to_string(),
        }
    }

    pub(crate) fn favourites() -> Self {
        ApplicationCategory {
            key: "favourites".to_string(),
            display_name: fl!("favourites"),
            icon_name: "starred-symbolic".to_string(),
        }
    }

    pub(crate) fn recents() -> Self {
        ApplicationCategory {
            key: "recents".to_string(),
            display_name: fl!("recents"),
            icon_name: "document-open-recent-symbolic".to_string(),
        }
    }
}

/// Load all desktop applications from XDG data directories.
pub(crate) fn load_apps() -> Vec<Arc<ApplicationEntry>> {
    let languages: Vec<String> = i18n_embed::DesktopLanguageRequester::requested_languages()
        .into_iter()
        .map(|language| language.to_string())
        .collect();
    load_apps_from_dirs(fde::default_paths().collect(), &languages)
}

fn load_apps_from_dirs(
    data_dirs: Vec<PathBuf>,
    languages: &[String],
) -> Vec<Arc<ApplicationEntry>> {
    let current_desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_else(|_| "COSMIC".to_string())
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    load_apps_from_dirs_for_desktops(data_dirs, languages, &current_desktops)
}

fn load_apps_from_dirs_for_desktops(
    data_dirs: Vec<PathBuf>,
    languages: &[String],
    current_desktops: &[String],
) -> Vec<Arc<ApplicationEntry>> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    let mut total_files = 0usize;
    let executable_paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();

    for desktop_entry in fde::Iter::new(data_dirs.into_iter()).entries(Some(languages)) {
        total_files += 1;
        // Hidden entries still claim their ID and mask lower-precedence files.
        let id = format!("{}.desktop", desktop_entry.id());
        if !seen.insert(id.clone()) {
            continue;
        }
        if desktop_entry.hidden()
            || desktop_entry.no_display()
            || desktop_entry.type_().is_some_and(|kind| kind != "Application")
            || !desktop_visible(&desktop_entry, &current_desktops)
        {
            continue;
        }

        let exec = desktop_entry
            .exec()
            .filter(|exec| !exec.trim().is_empty())
            .map(ToOwned::to_owned);
        let dbus_activatable = desktop_entry.dbus_activatable();
        if exec.is_none() && !dbus_activatable {
            continue;
        }
        if desktop_entry
            .try_exec()
            .is_some_and(|program| !program_available(program, &executable_paths))
        {
            continue;
        }
        let Some(name) = desktop_entry
            .name(languages)
            .filter(|name| !name.trim().is_empty())
            .map(|name| name.into_owned())
        else {
            continue;
        };
        let description = desktop_entry
            .comment(languages)
            .or_else(|| desktop_entry.generic_name(languages))
            .map(|value| value.into_owned());
        let keywords_lower = desktop_entry
            .keywords(languages)
            .unwrap_or_default()
            .into_iter()
            .filter(|keyword| !keyword.is_empty())
            .map(|keyword| keyword.to_lowercase())
            .collect();
        let entry = ApplicationEntry {
            name_lower: name.to_lowercase(),
            desc_lower: description.as_ref().map(|value| value.to_lowercase()),
            id,
            name,
            exec,
            icon: desktop_entry.icon().map(Arc::from),
            categories: desktop_entry
                .categories()
                .unwrap_or_default()
                .into_iter()
                .filter(|category| !category.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            description,
            keywords_lower,
            is_terminal: desktop_entry.terminal(),
            dbus_activatable,
            path: desktop_entry.path.clone(),
            working_dir: desktop_entry.path().map(PathBuf::from),
        };
        apps.push(Arc::new(entry));
    }

    apps.sort_by(|a, b| {
        a.name_lower
            .cmp(&b.name_lower)
            .then_with(|| a.id.cmp(&b.id))
    });
    tracing::info!("Scanned {total_files} desktop files, loaded {} applications", apps.len());
    apps
}

fn desktop_visible(entry: &fde::DesktopEntry, current_desktops: &[String]) -> bool {
    let matches = |desktop: &str| {
        current_desktops
            .iter()
            .any(|current| current.eq_ignore_ascii_case(desktop))
    };
    if entry
        .only_show_in()
        .is_some_and(|desktops| !desktops.into_iter().filter(|d| !d.is_empty()).any(matches))
    {
        return false;
    }
    !entry
        .not_show_in()
        .is_some_and(|desktops| desktops.into_iter().filter(|d| !d.is_empty()).any(matches))
}

fn program_available(program: &str, executable_paths: &[PathBuf]) -> bool {
    let candidate = PathBuf::from(program);
    if candidate.components().count() > 1 {
        return candidate.is_file();
    }
    executable_paths
        .iter()
        .any(|directory| directory.join(program).is_file())
}

/// Category groups in tie-break order.
const CATEGORY_GROUPS: &[(&str, &str, &[&str])] = &[
    (
        "Multimedia",
        "applications-multimedia-symbolic",
        &[
            "AudioVideo", "Audio", "Video", "Player", "Music", "VideoPlayer", "Recorder",
        ],
    ),
    (
        "Development",
        "applications-engineering-symbolic",
        &[
            "Development", "IDE", "Programming", "Debugger", "RevisionControl",
            "WebDevelopment",
        ],
    ),
    // The COSMIC theme does not provide generic education/internet icons.
    (
        "Education",
        "accessories-dictionary-symbolic",
        &["Education"],
    ),
    (
        "Games",
        "applications-games-symbolic",
        &[
            "Game", "Amusement", "ArcadeGame", "BoardGame", "BlocksGame", "CardGame",
            "RolePlaying", "Simulation", "SportsGame", "StrategyGame",
        ],
    ),
    (
        "Graphics",
        "applications-graphics-symbolic",
        &[
            "Graphics", "2DGraphics", "3DGraphics", "RasterGraphics", "VectorGraphics",
            "Photography", "Viewer",
        ],
    ),
    (
        "Internet",
        "network-wired-symbolic",
        &[
            "Network", "WebBrowser", "Email", "Chat", "IRCClient", "InstantMessaging",
            "Telephony", "VideoConference", "News", "P2P", "RemoteAccess",
        ],
    ),
    (
        "Office",
        "applications-office-symbolic",
        &[
            "Office", "Calendar", "ContactManagement", "WordProcessor", "Spreadsheet",
            "Presentation", "Finance", "Database",
        ],
    ),
    (
        "Science",
        "applications-science-symbolic",
        &[
            "Science", "Math", "Astronomy", "Biology", "Chemistry", "Engineering",
            "Geoscience", "MedicalSoftware", "Physics",
        ],
    ),
    (
        "Settings",
        "preferences-system-symbolic",
        &["Settings", "DesktopSettings", "HardwareSettings", "PackageManager"],
    ),
    (
        "System",
        "computer-symbolic",
        &[
            "System", "Utility", "FileManager", "TerminalEmulator", "Monitor", "Security",
            "Accessibility", "Core", "ConsoleOnly",
        ],
    ),
];

fn primary_category_key(categories: &[String]) -> Option<&'static str> {
    CATEGORY_GROUPS
        .iter()
        .find(|(_, _, source_keys)| {
            categories
                .iter()
                .any(|category| source_keys.contains(&category.as_str()))
        })
        .map(|(key, _, _)| *key)
}

fn group_display_name(key: &str) -> String {
    match key {
        "Multimedia" => fl!("cat-multimedia"),
        "Development" => fl!("cat-development"),
        "Education" => fl!("cat-education"),
        "Games" => fl!("cat-games"),
        "Graphics" => fl!("cat-graphics"),
        "Internet" => fl!("cat-internet"),
        "Office" => fl!("cat-office"),
        "Science" => fl!("cat-science"),
        "Settings" => fl!("cat-settings"),
        "System" => fl!("cat-system"),
        _ => key.to_string(),
    }
}

pub(crate) fn load_categories(apps: &[Arc<ApplicationEntry>]) -> Vec<ApplicationCategory> {
    let populated: HashSet<_> = apps
        .iter()
        .filter_map(|app| primary_category_key(&app.categories))
        .collect();

    let mut categories = Vec::new();
    for (group_key, icon_name, _source_keys) in CATEGORY_GROUPS {
        if populated.contains(group_key) {
            categories.push(ApplicationCategory {
                key: (*group_key).to_string(),
                display_name: group_display_name(group_key),
                icon_name: (*icon_name).to_string(),
            });
        }
    }

    let has_other = apps
        .iter()
        .any(|app| primary_category_key(&app.categories).is_none());
    if has_other {
        categories.push(ApplicationCategory {
            key: "Other".to_string(),
            display_name: fl!("cat-other"),
            icon_name: "application-x-executable-symbolic".to_string(),
        });
    }

    categories
}

pub(crate) fn filter_apps(
    apps: &[Arc<ApplicationEntry>],
    query: &str,
) -> Vec<Arc<ApplicationEntry>> {
    if query.is_empty() {
        return apps.to_vec();
    }

    let query_lower = query.to_lowercase();
    apps.iter()
        .filter(|app| {
            app.name_lower.contains(&query_lower)
                || app
                    .desc_lower
                    .as_ref()
                    .is_some_and(|description| description.contains(&query_lower))
                || app
                    .keywords_lower
                    .iter()
                    .any(|keyword| keyword.contains(&query_lower))
        })
        .cloned()
        .collect()
}

pub(crate) fn filter_apps_by_category(
    apps: &[Arc<ApplicationEntry>],
    category: &ApplicationCategory,
) -> Vec<Arc<ApplicationEntry>> {
    if category.key == "all" {
        return apps.to_vec();
    }

    if category.key == "Other" {
        return apps.iter()
            .filter(|app| primary_category_key(&app.categories).is_none())
            .cloned()
            .collect();
    }

    apps.iter()
        .filter(|app| primary_category_key(&app.categories) == Some(category.key.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn filter_by_ids(
    apps: &[Arc<ApplicationEntry>],
    ids: &[String],
) -> Vec<Arc<ApplicationEntry>> {
    ids.iter()
        .filter_map(|id| apps.iter().find(|a| a.id == *id).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{load_apps_from_dirs_for_desktops, primary_category_key};
    use std::fs;

    fn categories(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn assigns_each_app_to_one_primary_category() {
        assert_eq!(
            primary_category_key(&categories(&["GTK", "Network", "WebBrowser"])),
            Some("Internet")
        );
        assert_eq!(
            primary_category_key(&categories(&["Graphics", "Viewer", "Qt"])),
            Some("Graphics")
        );
    }

    #[test]
    fn leaves_only_unclassified_apps_in_other() {
        assert_eq!(
            primary_category_key(&categories(&["GTK", "Qt", "KDE"])),
            None
        );
        assert_eq!(
            primary_category_key(&categories(&["FileManager", "System"])),
            Some("System")
        );
    }

    #[test]
    fn recursively_loads_ids_and_hidden_override_masks_lower_precedence() {
        let root = std::env::temp_dir().join(format!(
            "cosmic-kickoff-apps-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let high = root.join("high/applications/nested");
        let low = root.join("low/applications/nested");
        fs::create_dir_all(&high).unwrap();
        fs::create_dir_all(&low).unwrap();
        fs::write(
            high.join("masked.desktop"),
            "[Desktop Entry]\nType=Application\nName=Masked\nHidden=true\n",
        )
        .unwrap();
        fs::write(
            low.join("masked.desktop"),
            "[Desktop Entry]\nType=Application\nName=Must stay hidden\nExec=/bin/true\n",
        )
        .unwrap();
        fs::write(
            low.join("visible.desktop"),
            "[Desktop Entry]\nType=Application\nName=Visible\nExec=/bin/true\nKeywords=needle;\n",
        )
        .unwrap();

        let apps = load_apps_from_dirs_for_desktops(
            vec![root.join("high/applications"), root.join("low/applications")],
            &["en".to_string()],
            &["COSMIC".to_string()],
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "nested-visible.desktop");
        assert_eq!(apps[0].keywords_lower, ["needle"]);

        fs::remove_dir_all(root).unwrap();
    }
}
