// SPDX-License-Identifier: GPL-3.0-only

//! Icon handling for the panel button and settings dropdown.
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

/// Names resolved from bundled assets rather than the current icon theme.
pub(crate) const BUNDLED_NAMES: &[&str] = &[
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

pub(crate) fn option_handle(name: &str, monochrome: bool) -> Handle {
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
