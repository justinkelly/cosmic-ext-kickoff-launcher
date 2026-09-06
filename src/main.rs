// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod apps;
mod config;
mod dock;
mod i18n;
mod icons;
mod launch;
mod power;
mod view;

use app::Applet;
use config::{AppletConfig, MIN_CUSTOM_HEIGHT, MIN_CUSTOM_WIDTH};
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::{Limits, Size};

fn main() -> cosmic::iced::Result {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default tracing subscriber failed");

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    // The panel proxies the applet's Wayland connection, so standalone mode
    // must start as a regular application.
    if std::env::args().any(|a| a == "--window") {
        let size = menu_window_size();
        cosmic::app::run::<Applet>(
            cosmic::app::Settings::default()
                .size(size)
                .size_limits(
                    Limits::NONE
                        .min_width(MIN_CUSTOM_WIDTH)
                        .min_height(MIN_CUSTOM_HEIGHT),
                )
                .resizable(Some(8.0)),
            (),
        )
    } else {
        cosmic::applet::run::<Applet>(())
    }
}

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
