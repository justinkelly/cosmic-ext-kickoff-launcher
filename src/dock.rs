// SPDX-License-Identifier: GPL-3.0-only

//! Integration with the COSMIC dock.

use crate::app::{Applet, COSMIC_FILES_APP_ID, COSMIC_SETTINGS_APP_ID};
use cosmic::cosmic_config::{self, ConfigGet, ConfigSet};

impl Applet {
    /// Pin a desktop file and return the updated dock configuration.
    pub(crate) fn pin_app_to_dock(&self, desktop_id: &str) -> Vec<String> {
        let Ok(config) = cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
            tracing::warn!("Cannot access CosmicDock config");
            return self.pinned_apps.clone();
        };
        let mut pinned: Vec<String> = config.get("pinned_apps").unwrap_or_default();
        if !pinned.iter().any(|p| p == desktop_id) {
            pinned.push(desktop_id.to_string());
            if let Err(e) = config.set("pinned_apps", &pinned) {
                tracing::warn!("Failed to pin app to dock: {e}");
            } else {
                tracing::info!("Pinned {desktop_id} to dock");
            }
        }
        pinned
    }

    /// Unpin a desktop file and return the updated dock configuration.
    pub(crate) fn unpin_app_from_dock(&self, desktop_id: &str) -> Vec<String> {
        let Ok(config) = cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
            tracing::warn!("Cannot access CosmicDock config");
            return self.pinned_apps.clone();
        };
        let mut pinned: Vec<String> = config.get("pinned_apps").unwrap_or_default();
        pinned.retain(|p| p != desktop_id);
        if let Err(e) = config.set("pinned_apps", &pinned) {
            tracing::warn!("Failed to unpin app from dock: {e}");
        } else {
            tracing::info!("Unpinned {desktop_id} from dock");
        }
        pinned
    }
}

/// Read pinned apps, adding Files and Settings as local display defaults.
pub(crate) fn load_pinned_apps_with_defaults() -> Vec<String> {
    const DEFAULTS: [&str; 2] = [COSMIC_FILES_APP_ID, COSMIC_SETTINGS_APP_ID];
    let Ok(config) = cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
        return DEFAULTS.iter().map(|id| (*id).to_string()).collect();
    };
    let pinned: Vec<String> = config.get("pinned_apps").unwrap_or_default();

    let mut ordered: Vec<String> = Vec::with_capacity(pinned.len() + DEFAULTS.len());
    for &id in &DEFAULTS {
        if !pinned.iter().any(|p| p == id) {
            ordered.push(id.to_string());
        }
    }
    ordered.extend(pinned);
    ordered
}
