// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Layout mode for the application listing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Icon grid layout (all apps)
    #[default]
    Grid,
    /// List layout with cards (all apps)
    List,
    /// Hybrid: Favourites + Recents in grid, everything else in list
    Hybrid,
}

impl LayoutMode {
    pub fn label(&self) -> &'static str {
        match self {
            LayoutMode::Grid => "Grid View",
            LayoutMode::List => "List View",
            LayoutMode::Hybrid => "Hybrid View",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            LayoutMode::Grid => "view-grid-symbolic",
            LayoutMode::List => "view-list-symbolic",
            LayoutMode::Hybrid => "view-grid-symbolic", // grid icon for hybrid
        }
    }

    /// Next mode in the cycle: Grid → List → Hybrid → Grid
    pub fn next(self) -> Self {
        match self {
            LayoutMode::Grid => LayoutMode::List,
            LayoutMode::List => LayoutMode::Hybrid,
            LayoutMode::Hybrid => LayoutMode::Grid,
        }
    }
}

// Custom serde: accepts both bool (old configs) and string (new configs).
impl Serialize for LayoutMode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            LayoutMode::Grid => "grid",
            LayoutMode::List => "list",
            LayoutMode::Hybrid => "hybrid",
        })
    }
}

impl<'de> Deserialize<'de> for LayoutMode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = LayoutMode;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"grid\", \"list\", \"hybrid\", or boolean")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<LayoutMode, E> {
                Ok(if v { LayoutMode::Grid } else { LayoutMode::List })
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<LayoutMode, E> {
                match v {
                    "grid" => Ok(LayoutMode::Grid),
                    "list" => Ok(LayoutMode::List),
                    "hybrid" => Ok(LayoutMode::Hybrid),
                    _ => Err(E::custom(format!("unknown layout mode: {}", v))),
                }
            }
        }
        d.deserialize_any(Visitor)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizePreset {
    /// Small landscape 600×480.
    Small,
    /// Medium landscape 800×640.
    Medium,
    /// Medium square 800×800.
    #[serde(alias = "MediumTall")]
    MediumSquare,
    /// Large landscape 1000×780.
    Large,
    /// Tall portrait 500×900.
    Tall,
    /// Square 600×600.
    Square,
    /// Portrait 600×700.
    Portrait,
    /// User-defined size via custom_width / custom_height.
    Custom,
}

impl SizePreset {
    pub fn width(&self) -> f32 {
        match self {
            SizePreset::Small => 600.0,
            SizePreset::Medium => 800.0,
            SizePreset::MediumSquare => 800.0,
            SizePreset::Large => 1000.0,
            SizePreset::Tall => 500.0,
            SizePreset::Square => 600.0,
            SizePreset::Portrait => 600.0,
            SizePreset::Custom => 0.0, // uses custom_width
        }
    }
    pub fn height(&self) -> f32 {
        match self {
            SizePreset::Small => 480.0,
            SizePreset::Medium => 640.0,
            SizePreset::MediumSquare => 800.0,
            SizePreset::Large => 780.0,
            SizePreset::Tall => 900.0,
            SizePreset::Square => 600.0,
            SizePreset::Portrait => 700.0,
            SizePreset::Custom => 0.0, // uses custom_height
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            SizePreset::Small => "Small",
            SizePreset::Medium => "Medium",
            SizePreset::MediumSquare => "Medium Square",
            SizePreset::Large => "Large",
            SizePreset::Tall => "Tall",
            SizePreset::Square => "Square",
            SizePreset::Portrait => "Portrait",
            SizePreset::Custom => "Custom",
        }
    }
    pub const ALL: [SizePreset; 8] = [
        SizePreset::Small,
        SizePreset::Medium,
        SizePreset::MediumSquare,
        SizePreset::Large,
        SizePreset::Tall,
        SizePreset::Square,
        SizePreset::Portrait,
        SizePreset::Custom,
    ];

    /// Override pixel width (0 = use preset).
    pub fn effective_width(&self, custom: f32) -> f32 {
        match self {
            SizePreset::Custom => custom.max(400.0),
            _ => if custom > 0.0 { custom } else { self.width() },
        }
    }
    /// Override pixel height (0 = use preset).
    pub fn effective_height(&self, custom: f32) -> f32 {
        match self {
            SizePreset::Custom => custom.max(300.0),
            _ => if custom > 0.0 { custom } else { self.height() },
        }
    }
}

/// Applet configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppletConfig {
    /// Layout mode: Grid, List, or Hybrid (Favs+Recents grid, rest list).
    /// Stored as `use_grid` in TOML for backward compat (old bool → Grid/List).
    #[serde(rename = "use_grid", default)]
    pub layout_mode: LayoutMode,
    /// Number of columns in the app grid
    pub grid_columns: usize,
    /// App icon size in pixels (24-56)
    pub icon_size: f32,
    /// Menu size preset (Small / Medium / Large)
    pub size_preset: SizePreset,
    /// Custom popup width override in pixels (0 = use size_preset)
    pub custom_width: f32,
    /// Custom popup height override in pixels (0 = use size_preset)
    pub custom_height: f32,
    /// Panel button icon name (empty = use default)
    pub panel_icon: String,
    /// Use symbolic (monochrome) icon in the panel (false = full colour).
    pub panel_icon_symbolic: bool,
    /// Maximum number of recent apps to track (0 = disabled).
    pub max_recents: usize,
    /// Show the Favourites section in the sidebar.
    pub show_favourites: bool,
    /// Show the Recents section in the sidebar.
    pub show_recents: bool,
    /// Recently launched app IDs (most recent first).
    #[serde(default)]
    pub recents: Vec<String>,
    /// Favourited app IDs.
    #[serde(default)]
    pub favourites: Vec<String>,
    /// Default menu category on open: "all", "favourites", or "recents".
    #[serde(default = "default_category")]
    pub default_category: String,
    /// Hide the category sidebar when the menu first opens.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// Show pinned app shortcuts on the bottom bar (far left).
    #[serde(default, alias = "show_bottom_bar_apps")]
    pub show_bottom_bar_pinned: bool,
    /// Show session/power actions (lock, logout, suspend, etc.) on the bottom bar.
    #[serde(default = "default_show_bottom_bar_power_actions")]
    pub show_bottom_bar_power_actions: bool,
}

fn default_show_bottom_bar_power_actions() -> bool {
    true
}

fn default_category() -> String { "all".into() }

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            layout_mode: LayoutMode::Grid,
            grid_columns: 6,
            icon_size: 48.0,
            size_preset: SizePreset::Medium,
            custom_width: 0.0,
            custom_height: 0.0,
            panel_icon: String::new(),
            panel_icon_symbolic: false,
            max_recents: 10,
            show_favourites: true,
            show_recents: true,
            recents: Vec::new(),
            favourites: Vec::new(),
            default_category: "all".into(),
            sidebar_collapsed: false,
            show_bottom_bar_pinned: false,
            show_bottom_bar_power_actions: true,
        }
    }
}

impl AppletConfig {
    /// Get the config file path.
    fn config_path() -> PathBuf {
        let base = if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(dir)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config")
        } else {
            PathBuf::from(".")
        };
        base.join("cosmic-kde-launcher").join("config.toml")
    }

    /// Load config from disk, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config = if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str(&contents) {
                tracing::info!("Loaded config from {:?}", path);
                config
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };
        config.sanitize();
        config
    }

    /// Effective popup width (preset × override).
    pub fn max_width(&self) -> f32 {
        self.size_preset.effective_width(self.custom_width)
    }

    /// Effective popup height (preset × override).
    pub fn max_height(&self) -> f32 {
        self.size_preset.effective_height(self.custom_height)
    }

    /// Ensure values are within reasonable bounds (auto-upgrades stale configs).
    fn sanitize(&mut self) {
        if self.grid_columns < 3 {
            self.grid_columns = 6;
        }
        if self.icon_size < 24.0 || self.icon_size > 56.0 {
            self.icon_size = 48.0;
        }
        // Clamp custom dimensions to reasonable bounds.
        if self.custom_width > 0.0 && self.custom_width < 400.0 {
            self.custom_width = 400.0;
        }
        if self.custom_height > 0.0 && self.custom_height < 300.0 {
            self.custom_height = 300.0;
        }
    }

    /// Save config to disk.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(contents) = toml::to_string_pretty(self) {
            let _ = std::fs::write(&path, contents);
            tracing::info!("Saved config to {:?}", path);
        }
    }

    /// Record an app launch (adds to front of recents, trims to max).
    pub fn add_recent(&mut self, app_id: &str) {
        self.recents.retain(|id| id != app_id);
        self.recents.insert(0, app_id.to_string());
        if self.recents.len() > self.max_recents {
            self.recents.truncate(self.max_recents);
        }
    }

    /// Toggle favourite status for an app. Returns the new state.
    pub fn toggle_favourite(&mut self, app_id: &str) -> bool {
        if let Some(pos) = self.favourites.iter().position(|id| id == app_id) {
            self.favourites.remove(pos);
            false
        } else {
            self.favourites.push(app_id.to_string());
            true
        }
    }
}
