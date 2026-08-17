// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod apps;
mod config;
mod dock;
mod i18n;
mod launch;
mod power;
mod view;

use app::Applet;
use config::AppletConfig;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::{Limits, Size};

fn main() -> cosmic::iced::Result {
    // Initialize logging through `tracing`, honouring RUST_LOG (default: warn).
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default tracing subscriber failed");

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // `--window` opens the menu as a standalone window. It must run as a
    // regular application on the real compositor: the applet's connection is
    // proxied by the panel, which embeds any toplevel it creates at (0,0)
    // inside the panel. The fixed min/max size makes cosmic-comp classify the
    // window as a dialog, keeping it floating instead of tiling it.
    if std::env::args().any(|a| a == "--window") {
        let size = menu_window_size();
        cosmic::app::run::<Applet>(
            cosmic::app::Settings::default()
                .size(size)
                .size_limits(
                    Limits::NONE
                        .min_width(size.width)
                        .min_height(size.height)
                        .max_width(size.width)
                        .max_height(size.height),
                )
                .resizable(None),
            (),
        )
    } else {
        // Starts the applet's event loop with `()` as the application's flags.
        cosmic::applet::run::<Applet>(())
    }
}

/// Menu size for the standalone window, read from the user's config.
fn menu_window_size() -> Size {
    let config = cosmic::cosmic_config::Config::new(app::APP_ID, AppletConfig::VERSION)
        .ok()
        .map(|context| {
            AppletConfig::get_entry(&context)
                .unwrap_or_else(|(_errors, config)| config)
        })
        .unwrap_or_default();
    Size::new(config.max_width(), config.max_height())
}
