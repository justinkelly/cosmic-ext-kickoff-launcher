// SPDX-License-Identifier: GPL-3.0-only

//! Project-owned panel icons embedded in the applet binary.

use crate::app::APP_ID;
use cosmic::widget::icon;

pub(crate) const MOBILE_DOTS: &str = "cosmic-kickoff-mobile-dots";
pub(crate) const APP_DRAWER: &str = "cosmic-kickoff-app-drawer";
pub(crate) const BENTO: &str = "cosmic-kickoff-bento";
pub(crate) const HONEYCOMB: &str = "cosmic-kickoff-honeycomb";
pub(crate) const COSMIC_APP_LIBRARY: &str = "com.system76.CosmicAppLibrary";
pub(crate) const COSMIC_LOGO: &str = "cosmic-kickoff-cosmic-logo";
pub(crate) const SYSTEM76_LOGO: &str = "cosmic-kickoff-system76-logo";
pub(crate) const XFCE_APPLICATIONS_MENU: &str = "cosmic-kickoff-xfce-applications-menu";
pub(crate) const HAIKU_DESKBAR: &str = "cosmic-kickoff-haiku-deskbar";
pub(crate) const KDE2_KICKER: &str = "cosmic-kickoff-kde2-kicker";
pub(crate) const KDE3_CRYSTAL: &str = "cosmic-kickoff-kde3-crystal";
pub(crate) const KDE_CLASSIC: &str = "cosmic-kickoff-kde-classic";
pub(crate) const KDE_BREEZE: &str = "cosmic-kickoff-kde-breeze";
pub(crate) const KDE_OXYGEN: &str = "cosmic-kickoff-kde-oxygen";
pub(crate) const DEBIAN: &str = "cosmic-kickoff-debian";

pub(crate) fn handle(name: &str, symbolic: bool) -> Option<icon::Handle> {
    if name == KDE2_KICKER && !symbolic {
        return Some(icon::from_raster_bytes(include_bytes!(
            "../resources/panel-icons/kde2-kicker.png"
        )));
    }

    let bytes: &'static [u8] = match (name, symbolic) {
        (APP_ID, false) => include_bytes!("../resources/icon.svg"),
        (APP_ID, true) => include_bytes!("../resources/icon-symbolic.svg"),
        (MOBILE_DOTS, false) => {
            include_bytes!("../resources/panel-icons/mobile-dots.svg")
        }
        (MOBILE_DOTS, true) => {
            include_bytes!("../resources/panel-icons/mobile-dots-symbolic.svg")
        }
        (APP_DRAWER, false) => {
            include_bytes!("../resources/panel-icons/app-drawer.svg")
        }
        (APP_DRAWER, true) => {
            include_bytes!("../resources/panel-icons/app-drawer-symbolic.svg")
        }
        (BENTO, false) => include_bytes!("../resources/panel-icons/bento.svg"),
        (BENTO, true) => include_bytes!("../resources/panel-icons/bento-symbolic.svg"),
        (HONEYCOMB, false) => include_bytes!("../resources/panel-icons/honeycomb.svg"),
        (HONEYCOMB, true) => {
            include_bytes!("../resources/panel-icons/honeycomb-symbolic.svg")
        }
        (COSMIC_APP_LIBRARY, false) => {
            include_bytes!("../resources/panel-icons/cosmic-app-library.svg")
        }
        (COSMIC_APP_LIBRARY, true) => {
            include_bytes!("../resources/panel-icons/cosmic-app-library-symbolic.svg")
        }
        (COSMIC_LOGO, false) => include_bytes!("../resources/icons/cosmic-logo.svg"),
        (COSMIC_LOGO, true) => include_bytes!("../resources/icons/cosmic-logo-mono.svg"),
        (SYSTEM76_LOGO, false) => include_bytes!("../resources/icons/system76-logo.svg"),
        (SYSTEM76_LOGO, true) => {
            include_bytes!("../resources/icons/system76-logo-mono.svg")
        }
        (XFCE_APPLICATIONS_MENU, false) => {
            include_bytes!("../resources/panel-icons/xfce-applications-menu.svg")
        }
        (XFCE_APPLICATIONS_MENU, true) => {
            include_bytes!("../resources/panel-icons/xfce-applications-menu-symbolic.svg")
        }
        (HAIKU_DESKBAR, false) => {
            include_bytes!("../resources/panel-icons/haiku-deskbar.svg")
        }
        (HAIKU_DESKBAR, true) => {
            include_bytes!("../resources/panel-icons/haiku-deskbar-symbolic.svg")
        }
        (KDE2_KICKER, true) => {
            include_bytes!("../resources/panel-icons/kde-breeze-symbolic.svg")
        }
        (KDE3_CRYSTAL, false) => {
            include_bytes!("../resources/panel-icons/kde3-crystal.svg")
        }
        (KDE3_CRYSTAL, true) => {
            include_bytes!("../resources/panel-icons/kde-breeze-symbolic.svg")
        }
        (KDE_CLASSIC, false) => include_bytes!("../resources/icons/kde-classic.svg"),
        (KDE_CLASSIC, true) => {
            include_bytes!("../resources/panel-icons/kde-breeze-symbolic.svg")
        }
        (KDE_BREEZE, false) => {
            include_bytes!("../resources/panel-icons/kde-breeze.svg")
        }
        (KDE_BREEZE, true) => {
            include_bytes!("../resources/panel-icons/kde-breeze-symbolic.svg")
        }
        (KDE_OXYGEN, false) => include_bytes!("../resources/icons/kde-oxygen.svg"),
        // Oxygen predates symbolic icons; use KDE's current small-size
        // symbolic launcher glyph rather than flattening its blue tile.
        (KDE_OXYGEN, true) => {
            include_bytes!("../resources/panel-icons/kde-breeze-symbolic.svg")
        }
        (DEBIAN, _) => include_bytes!("../resources/panel-icons/debian.svg"),
        _ => return None,
    };

    let mut handle = icon::from_svg_bytes(bytes);
    handle.symbolic = symbolic;
    Some(handle)
}
