// SPDX-License-Identifier: GPL-3.0-only

//! Bundled icon assets for panel-icon options that don't exist in every
//! icon theme (e.g. distro logos are missing from the COSMIC icon theme),
//! plus name → handle resolution shared by the panel button and the
//! settings dropdown.

use cosmic::widget::icon::{self, Handle};
use rust_embed::RustEmbed;

/// Icon assets embedded in the binary.
///
/// Layout: `resources/icons/<name>.svg` is the coloured variant and
/// `resources/icons/<name>-mono.svg` the monochrome (glyph-only, tinted at
/// draw time) variant.
#[derive(RustEmbed)]
#[folder = "resources/icons/"]
struct BundledIcons;

/// Option names that are resolved from bundled assets instead of the icon
/// theme, so they render regardless of which icon theme is installed.
pub const BUNDLED_NAMES: &[&str] = &[
    "distributor-logo-debian",
    "start-here-ubuntu",
    "distributor-logo-archlinux",
    "start-here-fedora",
    "distributor-logo-pop-os",
    "distributor-logo-manjaro",
    "distributor-logo-opensuse",
    "distributor-logo-linux-mint",
    "cosmic-logo",
    "system76-logo",
    "kde",
    "gnome-logo",
    "rust-logo",
];

/// Resolve a panel-icon option name to an icon handle.
///
/// Bundled names use the embedded SVG (the monochrome glyph variant when
/// `monochrome` is set); everything else goes through the icon theme, with
/// the handle's `symbolic` flag set for monochrome so the theme tints it.
pub fn option_handle(name: &str, monochrome: bool) -> Handle {
    if BUNDLED_NAMES.contains(&name) {
        let file = if monochrome {
            format!("{name}-mono.svg")
        } else {
            format!("{name}.svg")
        };
        if let Some(file) = BundledIcons::get(&file) {
            let mut handle = icon::from_svg_bytes(file.data);
            handle.symbolic = monochrome;
            return handle;
        }
    }
    icon::from_name(name).symbolic(monochrome).handle()
}
