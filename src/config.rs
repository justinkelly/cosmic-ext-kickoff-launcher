// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) const MIN_CUSTOM_WIDTH: f32 = 400.0;
pub(crate) const MAX_CUSTOM_WIDTH: f32 = 2_000.0;
pub(crate) const MIN_CUSTOM_HEIGHT: f32 = 300.0;
pub(crate) const MAX_CUSTOM_HEIGHT: f32 = 1_600.0;

/// Layout mode for the application listing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutMode {
    #[default]
    Grid,
    List,
    /// Favourites and recents use the grid; other categories use the list.
    Hybrid,
}

// Older releases stored this setting as a boolean.
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
        impl serde::de::Visitor<'_> for Visitor {
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
                    _ => Err(E::custom(format!("unknown layout mode: {v}"))),
                }
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SizePreset {
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
    /// User-defined size.
    Custom,
}

impl SizePreset {
    pub const fn width(self) -> f32 {
        match self {
            SizePreset::Small => 600.0,
            SizePreset::Medium => 800.0,
            SizePreset::MediumSquare => 800.0,
            SizePreset::Large => 1000.0,
            SizePreset::Tall => 500.0,
            SizePreset::Square => 600.0,
            SizePreset::Portrait => 600.0,
            SizePreset::Custom => 0.0,
        }
    }

    pub const fn height(self) -> f32 {
        match self {
            SizePreset::Small => 480.0,
            SizePreset::Medium => 640.0,
            SizePreset::MediumSquare => 800.0,
            SizePreset::Large => 780.0,
            SizePreset::Tall => 900.0,
            SizePreset::Square => 600.0,
            SizePreset::Portrait => 700.0,
            SizePreset::Custom => 0.0,
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

    pub fn effective_width(self, custom: f32) -> f32 {
        if custom.is_finite() && custom > 0.0 {
            custom.clamp(MIN_CUSTOM_WIDTH, MAX_CUSTOM_WIDTH)
        } else if self == SizePreset::Custom {
            MIN_CUSTOM_WIDTH
        } else {
            self.width()
        }
    }

    pub fn effective_height(self, custom: f32) -> f32 {
        if custom.is_finite() && custom > 0.0 {
            custom.clamp(MIN_CUSTOM_HEIGHT, MAX_CUSTOM_HEIGHT)
        } else if self == SizePreset::Custom {
            MIN_CUSTOM_HEIGHT
        } else {
            self.height()
        }
    }
}

/// Applet configuration, managed by cosmic-config (cosmic-settings-daemon keeps
/// live instances in sync via the `dbus-config` feature).
#[derive(Debug, Clone, CosmicConfigEntry, Serialize, Deserialize, PartialEq)]
#[version = 1]
pub(crate) struct AppletConfig {
    /// Stored as `use_grid` for compatibility with the old boolean setting.
    #[serde(rename = "use_grid", default)]
    pub layout_mode: LayoutMode,
    /// Maximum number of columns in the app grid.
    pub grid_columns: usize,
    /// App icon size in pixels.
    pub icon_size: f32,
    /// Menu size preset.
    pub size_preset: SizePreset,
    /// Custom popup width in pixels, or zero to use the preset.
    pub custom_width: f32,
    /// Custom popup height in pixels, or zero to use the preset.
    pub custom_height: f32,
    /// Panel button icon name, or an empty string to use the default.
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

fn default_category() -> String {
    "all".into()
}

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            layout_mode: LayoutMode::Hybrid,
            grid_columns: 6,
            icon_size: 48.0,
            size_preset: SizePreset::Portrait,
            custom_width: 0.0,
            custom_height: 0.0,
            panel_icon: "cosmic-logo".into(),
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
    pub fn max_width(&self) -> f32 {
        self.size_preset.effective_width(self.custom_width)
    }

    pub fn max_height(&self) -> f32 {
        self.size_preset.effective_height(self.custom_height)
    }

    /// Normalize values loaded from disk.
    pub fn sanitize(&mut self) {
        if !(3..=8).contains(&self.grid_columns) {
            self.grid_columns = 6;
        }
        if !self.icon_size.is_finite() || !(24.0..=56.0).contains(&self.icon_size) {
            self.icon_size = 48.0;
        }
        if !self.custom_width.is_finite() || self.custom_width < 0.0 {
            self.custom_width = 0.0;
        } else if self.custom_width > 0.0 {
            self.custom_width = self
                .custom_width
                .clamp(MIN_CUSTOM_WIDTH, MAX_CUSTOM_WIDTH);
        }
        if !self.custom_height.is_finite() || self.custom_height < 0.0 {
            self.custom_height = 0.0;
        } else if self.custom_height > 0.0 {
            self.custom_height = self
                .custom_height
                .clamp(MIN_CUSTOM_HEIGHT, MAX_CUSTOM_HEIGHT);
        }

        deduplicate(&mut self.favourites);
        deduplicate(&mut self.recents);
        self.recents.truncate(self.max_recents);
    }

    pub fn add_recent(&mut self, app_id: &str) {
        self.recents.retain(|id| id != app_id);
        self.recents.insert(0, app_id.to_string());
        if self.recents.len() > self.max_recents {
            self.recents.truncate(self.max_recents);
        }
    }

    pub fn toggle_favourite(&mut self, app_id: &str) -> bool {
        if let Some(pos) = self.favourites.iter().position(|id| id == app_id) {
            self.favourites.remove(pos);
            false
        } else {
            self.favourites.push(app_id.to_string());
            true
        }
    }

    fn legacy_config_path() -> Option<PathBuf> {
        let base = if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(dir)
        } else {
            std::env::var("HOME").ok().map(PathBuf::from)?.join(".config")
        };
        Some(base.join("cosmic-kde-launcher").join("config.toml"))
    }

    pub fn migrate_legacy(context: &cosmic_config::Config) -> Option<Self> {
        let path = Self::legacy_config_path()?;
        let contents = std::fs::read_to_string(&path).ok()?;
        let mut config: AppletConfig = toml::from_str(&contents).ok()?;
        config.sanitize();
        if let Err(err) = config.write_entry(context) {
            tracing::warn!("Failed to write migrated config: {err}");
            return None;
        }
        tracing::info!("Migrated legacy config.toml to cosmic-config");
        if let Err(err) = std::fs::rename(&path, path.with_extension("toml.migrated")) {
            tracing::warn!("Failed to rename legacy config file: {err}");
        }
        Some(config)
    }
}

pub(crate) fn validated_custom_size(width: f32, height: f32) -> Option<(f32, f32)> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((
        width.clamp(MIN_CUSTOM_WIDTH, MAX_CUSTOM_WIDTH),
        height.clamp(MIN_CUSTOM_HEIGHT, MAX_CUSTOM_HEIGHT),
    ))
}

fn deduplicate(values: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::with_capacity(values.len());
    values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_clamps_custom_size_atomically() {
        assert_eq!(validated_custom_size(10.0, 9_000.0), Some((400.0, 1_600.0)));
        assert_eq!(validated_custom_size(f32::NAN, 600.0), None);
        assert_eq!(validated_custom_size(600.0, -1.0), None);
    }

    #[test]
    fn effective_custom_size_never_exceeds_layout_limits() {
        assert_eq!(SizePreset::Custom.effective_width(8_000.0), MAX_CUSTOM_WIDTH);
        assert_eq!(SizePreset::Custom.effective_height(8_000.0), MAX_CUSTOM_HEIGHT);
    }
}
