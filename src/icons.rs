// SPDX-License-Identifier: GPL-3.0-only

//! Bundled icon assets for panel-icon options that don't exist in every
//! icon theme (e.g. distro logos are missing from the COSMIC icon theme),
//! plus name → handle resolution shared by the panel button and the
//! settings dropdown.
//!
//! KDE and distribution artwork is sourced from KDE's official clipart and
//! distribution-logo collections: <https://kde.org/stuff/clipart/> and
//! <https://kde.org/content/distributions/logos/>. KDE Classic is retained as
//! a clearly labelled historical option; KDE advises against using legacy
//! marks for new KDE-related content.

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
    "kde-official",
    "kde-oxygen",
    "kde-plasma",
    "kde-classic",
    "kubuntu",
    "kde-neon",
    "gnome-logo",
    "rust-logo",
];

/// Resolve a panel-icon option name to an icon handle.
///
/// Bundled names use the embedded SVG (the monochrome glyph variant when
/// `monochrome` is set); everything else goes through the icon theme, with
/// the handle's `symbolic` flag set for monochrome so the theme tints it.
pub fn option_handle(name: &str, monochrome: bool) -> Handle {
    // Keep existing configurations using `kde` working, while upgrading
    // Keep existing configurations using `kde` working, while upgrading
    // them to the current official KDE logo artwork.
    let name = if name == "kde" { "kde-official" } else { name };

    if BUNDLED_NAMES.contains(&name) {
        let color_file = format!("{name}.svg");
        let asset = if monochrome {
            BundledIcons::get(&format!("{name}-mono.svg"))
                .map(|file| (file, true))
                .or_else(|| BundledIcons::get(&color_file).map(|file| (file, false)))
        } else {
            BundledIcons::get(&color_file).map(|file| (file, false))
        };
        if let Some((file, symbolic)) = asset {
            let mut handle = icon::from_svg_bytes(file.data);
            handle.symbolic = symbolic;
            return handle;
        }
    }
    icon::from_name(name).symbolic(monochrome).handle()
}
