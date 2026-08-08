// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Menu size presets — avoids hardcoded pixel sizes that don't scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizePreset {
    Small,
    Medium,
    Large,
}

impl SizePreset {
    pub fn width(&self) -> f32 {
        match self {
            SizePreset::Small => 600.0,
            SizePreset::Medium => 800.0,
            SizePreset::Large => 1000.0,
        }
    }
    pub fn height(&self) -> f32 {
        match self {
            SizePreset::Small => 480.0,
            SizePreset::Medium => 640.0,
            SizePreset::Large => 780.0,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            SizePreset::Small => "Small",
            SizePreset::Medium => "Medium",
            SizePreset::Large => "Large",
        }
    }
    pub const ALL: [SizePreset; 3] = [SizePreset::Small, SizePreset::Medium, SizePreset::Large];

    /// Override pixel width (0 = use preset).
    pub fn effective_width(&self, custom: f32) -> f32 {
        if custom > 0.0 { custom } else { self.width() }
    }
    /// Override pixel height (0 = use preset).
    pub fn effective_height(&self, custom: f32) -> f32 {
        if custom > 0.0 { custom } else { self.height() }
    }
}

/// Applet configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppletConfig {
    /// Whether to use a grid layout (true) or list layout (false)
    pub use_grid: bool,
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
}

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            use_grid: true,
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
        base.join("cosmic-kde-menu").join("config.toml")
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

    /// Check if an app is favourited.
    pub fn is_favourite(&self, app_id: &str) -> bool {
        self.favourites.iter().any(|id| id == app_id)
    }
}
