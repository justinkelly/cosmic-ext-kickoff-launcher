// SPDX-License-Identifier: GPL-3.0-only

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
    #[allow(dead_code)]
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
            display_name: "All".to_string(),
            icon_name: "applications-system-symbolic".to_string(),
        }
    }

    pub fn favourites() -> Self {
        ApplicationCategory {
            key: "favourites".to_string(),
            display_name: "Favourites".to_string(),
            icon_name: "starred-symbolic".to_string(),
        }
    }

    pub fn recents() -> Self {
        ApplicationCategory {
            key: "recents".to_string(),
            display_name: "Recents".to_string(),
            icon_name: "document-open-recent-symbolic".to_string(),
        }
    }
}

/// Get the list of directories to search for .desktop files.
fn get_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // XDG_DATA_DIRS (colon-separated)
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            let dir = dir.trim();
            if !dir.is_empty() {
                dirs.push(PathBuf::from(dir).join("applications"));
            }
        }
    }

    // Fallback defaults if XDG_DATA_DIRS is empty
    if dirs.is_empty() {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }

    // Flatpak applications (system)
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    // Flatpak applications (user)
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    }

    // Snap applications
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    // XDG_DATA_HOME (or ~/.local/share)
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }

    tracing::info!("Searching for .desktop files in: {:?}", dirs);

    dirs
}

/// Load all desktop applications from XDG data directories.
pub fn load_apps() -> Vec<Arc<ApplicationEntry>> {
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

            match parse_desktop_file(&path) {
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
fn parse_desktop_file(path: &std::path::Path) -> Result<Option<ApplicationEntry>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read error: {}", e))?;
    let content = String::from_utf8_lossy(&bytes);

    // Must have a [Desktop Entry] section
    if !content.contains("[Desktop Entry]") {
        return Ok(None);
    }

    // Parse the [Desktop Entry] section into a map — single scan, no per-key re-searching.
    let entries = parse_section(&content, "Desktop Entry");

    let name = match entries.get("Name") {
        Some(n) if !n.is_empty() => n.clone(),
        _ => return Ok(None),
    };

    // Skip hidden entries
    if entries.get("NoDisplay").map(|v| v.to_lowercase() == "true").unwrap_or(false) {
        return Ok(None);
    }
    if entries.get("Hidden").map(|v| v.to_lowercase() == "true").unwrap_or(false) {
        return Ok(None);
    }

    // Skip apps that explicitly exclude COSMIC via NotShowIn
    if let Some(not_show_in) = entries.get("NotShowIn") {
        let nsi = not_show_in.to_lowercase();
        for de in nsi.split(';') {
            let de = de.trim();
            if de == "cosmic" || de == "pop:cosmic" {
                return Ok(None);
            }
        }
    }

    // Must have Type=Application (or no Type key, which defaults to Application)
    if entries.get("Type").map_or(false, |t| t != "Application") {
        return Ok(None);
    }

    let exec = entries.get("Exec").cloned();
    let icon = entries.get("Icon").cloned();
    let categories_str = entries.get("Categories").cloned();
    let comment = entries.get("Comment").cloned();
    let terminal = entries.get("Terminal")
        .map(|t| t.to_lowercase() == "true")
        .unwrap_or(false);

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

/// Parse a desktop-file section into a `HashMap<key, value>`. Single pass, no allocations per key.
fn parse_section(content: &str, section_name: &str) -> HashMap<String, String> {
    let header = format!("[{}]", section_name);
    let section_start = match content.find(&header) {
        Some(pos) => pos,
        None => return HashMap::new(),
    };
    let after_header = section_start + header.len();
    let section_end = content[after_header..]
        .find("\n[")
        .map(|pos| after_header + pos)
        .unwrap_or(content.len());

    let mut map = HashMap::new();
    for line in content[after_header..section_end].lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
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
const CATEGORY_GROUPS: &[(&str, &str, &str, &[&str])] = &[
    ("Multimedia", "Multimedia", "applications-multimedia-symbolic", &["AudioVideo", "Audio", "Video"]),
    ("Development", "Development", "applications-engineering-symbolic", &["Development"]),
    ("Education", "Education", "applications-education-symbolic", &["Education"]),
    ("Games", "Games", "applications-games-symbolic", &["Game"]),
    ("Graphics", "Graphics", "applications-graphics-symbolic", &["Graphics"]),
    ("Internet", "Internet", "applications-internet-symbolic", &["Network"]),
    ("Office", "Office", "applications-office-symbolic", &["Office"]),
    ("Science", "Science", "applications-science-symbolic", &["Science"]),
    ("Settings", "Settings", "preferences-system-symbolic", &["Settings"]),
    ("System", "System", "applications-system-symbolic", &["System", "Utility"]),
];

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
    for (group_key, display_name, icon_name, source_keys) in CATEGORY_GROUPS {
        let total: usize = source_keys.iter()
            .map(|k| counts.get(*k).copied().unwrap_or(0))
            .sum();
        if total > 0 {
            categories.push(ApplicationCategory {
                key: group_key.to_string(),
                display_name: display_name.to_string(),
                icon_name: icon_name.to_string(),
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
            display_name: "Other".to_string(),
            icon_name: "applications-other-symbolic".to_string(),
        });
    }

    categories
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
    let source_keys: &[&str] = CATEGORY_GROUPS.iter()
        .find(|(key, _, _, _)| *key == category.key)
        .map(|(_, _, _, keys)| *keys)
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
