// SPDX-License-Identifier: GPL-3.0-only

use crate::fl;
use i18n_embed::unic_langid::LanguageIdentifier;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// A parsed desktop application entry.
#[derive(Debug, Clone)]
pub struct ApplicationEntry {
    /// Desktop file ID (e.g. "firefox.desktop")
    pub id: String,
    /// Display name
    pub name: String,
    /// Lowercased name — precomputed for fast filtering.
    pub name_lower: String,
    /// Executable command (with %f/%u placeholders removed)
    pub exec: Option<String>,
    /// Icon name or path
    pub icon: Option<String>,
    /// Categories from the desktop file
    pub categories: Vec<String>,
    /// Generic name / description
    pub description: Option<String>,
    /// Lowercased description — precomputed for fast filtering.
    pub desc_lower: Option<String>,
    /// Whether the app runs in a terminal
    pub is_terminal: bool,
    /// Path to the desktop file
    pub path: PathBuf,
}

/// A category derived from desktop file Categories keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCategory {
    /// The desktop Category key (e.g. "Network")
    pub key: String,
    /// Display name (human-readable)
    pub display_name: String,
    /// Icon name for the category
    pub icon_name: String,
}

impl ApplicationCategory {
    pub fn all() -> Self {
        ApplicationCategory {
            key: "all".to_string(),
            display_name: fl!("cat-all"),
            icon_name: "applications-system-symbolic".to_string(),
        }
    }

    pub fn favourites() -> Self {
        ApplicationCategory {
            key: "favourites".to_string(),
            display_name: fl!("favourites"),
            icon_name: "starred-symbolic".to_string(),
        }
    }

    pub fn recents() -> Self {
        ApplicationCategory {
            key: "recents".to_string(),
            display_name: fl!("recents"),
            icon_name: "document-open-recent-symbolic".to_string(),
        }
    }
}

/// Get the list of directories to search for .desktop files,
/// ordered from LOWEST to HIGHEST priority so that the dedup logic
/// (which keeps the last occurrence) preserves the highest-priority entry.
pub fn get_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let push = |dirs: &mut Vec<PathBuf>, path: PathBuf| {
        if !dirs.iter().any(|existing| existing == &path) {
            dirs.push(path);
        }
    };

    // Process from lowest to highest priority. `load_apps` keeps the last
    // entry for a duplicate desktop ID, so this order follows the XDG rule
    // that earlier data directories have higher priority.
    push(
        &mut dirs,
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    );
    push(
        &mut dirs,
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    );

    // XDG_DATA_DIRS (colon-separated). The first entry has the highest
    // preference per the XDG spec, so we reverse them: lowest-priority
    // entries first so higher-priority ones (processed later) win during
    // deduplication.
    let mut has_xdg_data_dirs = false;
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        let mut entries: Vec<PathBuf> = data_dirs
            .split(':')
            .map(|d| d.trim())
            .filter(|d| !d.is_empty())
            .map(|d| PathBuf::from(d).join("applications"))
            .collect();
        entries.reverse();
        for entry in entries {
            push(&mut dirs, entry);
        }
        has_xdg_data_dirs = !data_dirs.trim().is_empty();
    }

    // Fallback defaults if XDG_DATA_DIRS is empty
    if !has_xdg_data_dirs {
        push(&mut dirs, PathBuf::from("/usr/share/applications"));
        push(&mut dirs, PathBuf::from("/usr/local/share/applications"));
    }

    // Flatpak applications (user) — lower priority than per-user native
    // desktop files, but higher than system-wide entries.
    if let Ok(home) = std::env::var("HOME") {
        push(
            &mut dirs,
            PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"),
        );
    }

    // XDG_DATA_HOME (or ~/.local/share) — HIGHEST priority, appended last
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        push(&mut dirs, PathBuf::from(data_home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        push(
            &mut dirs,
            PathBuf::from(home).join(".local/share/applications"),
        );
    }

    tracing::debug!("Searching for .desktop files in: {:?}", dirs);

    dirs
}

/// Load all desktop applications from XDG data directories.
pub fn load_apps() -> Vec<Arc<ApplicationEntry>> {
    let languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let mut apps: Vec<Arc<ApplicationEntry>> = Vec::new();
    let data_dirs = get_data_dirs();

    let mut total_files = 0u64;

    for apps_dir in &data_dirs {
        if !apps_dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(apps_dir) else {
            tracing::warn!("Cannot read directory: {:?}", apps_dir);
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "desktop") {
                continue;
            }
            total_files += 1;

            match parse_desktop_file(&path, &languages) {
                Ok(Some(app)) => {
                    if app.name.is_empty() {
                        continue;
                    }
                    apps.push(Arc::new(app));
                }
                Ok(None) => {
                    // Hidden or not a launcher — silently skip
                }
                Err(e) => {
                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                }
            }
        }
    }

    tracing::info!(
        "Scanned {} .desktop files, loaded {} applications",
        total_files,
        apps.len()
    );

    // Deduplicate by desktop file name — keep the LAST occurrence since
    // higher-precedence directories (user ~/.local) are appended later.
    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<Arc<ApplicationEntry>> = Vec::with_capacity(apps.len());
    for app in apps.into_iter().rev() {
        if seen.insert(app.id.clone()) {
            deduped.push(app);
        }
    }
    deduped.reverse();
    let mut apps = deduped;

    // Sort by precomputed lowercase name
    apps.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));

    apps
}

/// Parse a single desktop file. Returns:
/// - `Ok(Some(app))` if it's a valid launcher
/// - `Ok(None)` if it should be skipped (NoDisplay, Hidden, no Exec, etc.)
/// - `Err(msg)` on parse failure
fn parse_desktop_file(
    path: &std::path::Path,
    languages: &[LanguageIdentifier],
) -> Result<Option<ApplicationEntry>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read error: {}", e))?;
    let content = String::from_utf8_lossy(&bytes);

    // Parse the [Desktop Entry] section into a map — single scan, no per-key re-searching.
    let entries = parse_section(&content, "Desktop Entry");
    if entries.is_empty() {
        return Ok(None);
    }

    let name = match localized(&entries, "Name", languages) {
        Some(n) if !n.is_empty() => n,
        _ => return Ok(None),
    };

    // Skip hidden entries
    if entries
        .get("NoDisplay")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        return Ok(None);
    }
    if entries
        .get("Hidden")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        return Ok(None);
    }

    // Skip apps that explicitly exclude COSMIC via NotShowIn
    if let Some(not_show_in) = entries.get("NotShowIn") {
        for de in not_show_in.split(';') {
            let de = de.trim();
            if de.eq_ignore_ascii_case("cosmic") || de.eq_ignore_ascii_case("pop:cosmic") {
                return Ok(None);
            }
        }
    }

    // OnlyShowIn limits the desktop environments the app appears in.
    if let Some(only_show_in) = entries.get("OnlyShowIn") {
        let shown = only_show_in.split(';').any(|de| {
            let de = de.trim();
            de.eq_ignore_ascii_case("cosmic") || de.eq_ignore_ascii_case("pop:cosmic")
        });
        if !shown {
            return Ok(None);
        }
    }

    // Must have Type=Application (or no Type key, which defaults to Application)
    if entries.get("Type").is_some_and(|t| t != "Application") {
        return Ok(None);
    }

    // `Exec` is required for Type=Application. Keeping an entry without it
    // produces a card that can never launch anything.
    let exec = match entries.get("Exec").filter(|exec| !exec.trim().is_empty()) {
        Some(exec) => Some(exec.clone()),
        None => return Ok(None),
    };
    let icon = entries.get("Icon").cloned();
    let categories_str = entries.get("Categories").cloned();
    let comment = localized(&entries, "Comment", languages);
    let terminal = entries
        .get("Terminal")
        .is_some_and(|t| t.eq_ignore_ascii_case("true"));

    let categories: Vec<String> = categories_str
        .map(|s| {
            s.split(';')
                .filter(|c| !c.is_empty())
                .map(|c| c.trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.desktop")
        .to_string();

    // Precompute lowercase fields for fast filtering.
    let name_lower = name.to_lowercase();
    let desc_lower = comment.as_ref().map(|d| d.to_lowercase());

    Ok(Some(ApplicationEntry {
        id: file_name,
        name,
        name_lower,
        exec,
        icon,
        categories,
        description: comment,
        desc_lower,
        is_terminal: terminal,
        path: path.to_path_buf(),
    }))
}

/// Look up a localized desktop-entry key (e.g. `Name[fr]`), falling back from
/// the full language tag to its primary subtag, then to the untagged key.
fn localized(
    entries: &HashMap<String, String>,
    base: &str,
    languages: &[LanguageIdentifier],
) -> Option<String> {
    for lang in languages {
        if let Some(value) = entries.get(&format!("{base}[{lang}]")) {
            if !value.is_empty() {
                return Some(value.clone());
            }
        }
        if let Some(value) = entries.get(&format!("{base}[{}]", lang.language)) {
            if !value.is_empty() {
                return Some(value.clone());
            }
        }
    }
    entries.get(base).cloned()
}

/// Parse a desktop-file section into a `HashMap<key, value>`. Single pass, no allocations per key.
fn parse_section(content: &str, section_name: &str) -> HashMap<String, String> {
    let header = format!("[{}]", section_name);
    let mut map = HashMap::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == header;
            continue;
        }
        if !in_section || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let key = trimmed[..eq].trim().to_string();
            let value = trimmed[eq + 1..].trim().to_string();
            map.insert(key, value);
        }
    }
    map
}

/// Known category keys for desktop file Categories.
pub const KNOWN_CATEGORY_KEYS: &[&str] = &[
    "AudioVideo", "Audio", "Video",
    "Development", "Education", "Game",
    "Graphics", "Network", "Office",
    "Science", "Settings", "System", "Utility",
];

/// Pre-built set for O(1) known-category lookups.
static KNOWN_KEYS_SET: std::sync::LazyLock<std::collections::HashSet<&'static str>> =
    std::sync::LazyLock::new(|| KNOWN_CATEGORY_KEYS.iter().copied().collect());

/// Consolidated category groups mapping source keys → display group.
const CATEGORY_GROUPS: &[(&str, &str, &[&str])] = &[
    ("Multimedia", "applications-multimedia-symbolic", &["AudioVideo", "Audio", "Video"]),
    ("Development", "applications-engineering-symbolic", &["Development"]),
    ("Education", "applications-education-symbolic", &["Education"]),
    ("Games", "applications-games-symbolic", &["Game"]),
    ("Graphics", "applications-graphics-symbolic", &["Graphics"]),
    ("Internet", "applications-internet-symbolic", &["Network"]),
    ("Office", "applications-office-symbolic", &["Office"]),
    ("Science", "applications-science-symbolic", &["Science"]),
    ("Settings", "preferences-system-symbolic", &["Settings"]),
    ("System", "applications-system-symbolic", &["System", "Utility"]),
];

/// Localized display name for a consolidated category group.
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

/// Derive categories from the loaded applications (consolidated groups).
pub fn load_categories(apps: &[Arc<ApplicationEntry>]) -> Vec<ApplicationCategory> {
    // Count apps per raw .desktop category key
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for app in apps {
        for cat in &app.categories {
            *counts.entry(cat.clone()).or_default() += 1;
        }
    }

    let mut categories: Vec<ApplicationCategory> = Vec::new();

    // Build consolidated groups
    for (group_key, icon_name, source_keys) in CATEGORY_GROUPS {
        let total: usize = source_keys
            .iter()
            .map(|k| counts.get(*k).copied().unwrap_or(0))
            .sum();
        if total > 0 {
            categories.push(ApplicationCategory {
                key: (*group_key).to_string(),
                display_name: group_display_name(group_key),
                icon_name: (*icon_name).to_string(),
            });
        }
    }

    // "Other" category for apps with unrecognized categories or none
    let has_other = apps.iter().any(|app| {
        app.categories.is_empty()
            || app.categories.iter().any(|c| !KNOWN_KEYS_SET.contains(c.as_str()))
    });
    if has_other {
        categories.push(ApplicationCategory {
            key: "Other".to_string(),
            display_name: fl!("cat-other"),
            icon_name: "applications-other-symbolic".to_string(),
        });
    }

    categories
}

/// Quickly list all .desktop file basenames from data dirs (no parsing).
/// Used to detect new/removed apps without a full re-parse.
pub fn list_desktop_ids() -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for dir in &get_data_dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "desktop") {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                ids.insert(name.to_string());
            }
        }
    }
    ids
}

/// Filter apps by search query (case-insensitive match on precomputed lowercase fields).
pub fn filter_apps(apps: &[Arc<ApplicationEntry>], query: &str) -> Vec<Arc<ApplicationEntry>> {
    if query.is_empty() {
        return apps.to_vec();
    }

    let query_lower = query.to_lowercase();
    apps.iter()
        .filter(|app| {
            app.name_lower.contains(&query_lower)
                || app.desc_lower.as_ref().map_or(false, |d| d.contains(&query_lower))
        })
        .cloned()
        .collect()
}

/// Filter apps by a specific category group.
pub fn filter_apps_by_category(
    apps: &[Arc<ApplicationEntry>],
    category: &ApplicationCategory,
) -> Vec<Arc<ApplicationEntry>> {
    if category.key == "all" {
        return apps.to_vec();
    }

    if category.key == "Other" {
        return apps.iter()
            .filter(|app| {
                app.categories.is_empty()
                    || app.categories.iter().any(|c| !KNOWN_KEYS_SET.contains(c.as_str()))
            })
            .cloned()
            .collect();
    }

    // Look up which raw .desktop keys map to this group
    let source_keys: &[&str] = CATEGORY_GROUPS
        .iter()
        .find(|(key, _, _)| *key == category.key)
        .map(|(_, _, keys)| *keys)
        .unwrap_or(&[]);

    if source_keys.is_empty() {
        return apps.iter()
            .filter(|app| app.categories.contains(&category.key))
            .cloned()
            .collect();
    }

    apps.iter()
        .filter(|app| app.categories.iter().any(|c| source_keys.contains(&c.as_str())))
        .cloned()
        .collect()
}

/// Filter apps to only those whose IDs appear in the given list, in list order.
pub fn filter_by_ids(
    apps: &[Arc<ApplicationEntry>],
    ids: &[String],
) -> Vec<Arc<ApplicationEntry>> {
    ids.iter()
        .filter_map(|id| apps.iter().find(|a| a.id == *id).cloned())
        .collect()
}
