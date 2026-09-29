// SPDX-License-Identifier: GPL-3.0-only

//! Application model and COSMIC application implementation.

use crate::config::{validated_custom_size, AppletConfig, LayoutMode, SizePreset};
use crate::{apps, dock, fl, launch, panel_icons, power, view};
use cosmic::app::{Core, Task};
use cosmic::applet::token::subscription::{
    activation_token_subscription, TokenRequest, TokenUpdate,
};
use cosmic::cctk::sctk::reexports::calloop::channel::Sender;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{
    event::{self, listen_raw},
    keyboard::{self, key::Named},
    window::Id,
    Limits, Rectangle,
};
use cosmic::surface::action::{app_popup, destroy_popup, LiveSettings};
use cosmic::widget::{nav_bar_toggle, segmented_button};
use cosmic::{Application, Element};
use apps::{ApplicationCategory, ApplicationEntry};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

pub(crate) const APP_ID: &str = "com.github.cosmic-kickoff-launcher";

pub(crate) struct Flags {
    pub window: bool,
}
const LEGACY_APP_ID: &str = "com.github.cosmic-kde-launcher";
pub(crate) const COSMIC_FILES_APP_ID: &str = "com.system76.CosmicFiles.desktop";
pub(crate) const COSMIC_STORE_APP_ID: &str = "com.system76.CosmicStore.desktop";
pub(crate) const COSMIC_SETTINGS_APP_ID: &str = "com.system76.CosmicSettings.desktop";
pub(crate) const GRID_ICON_SIZE: u16 = 64;
pub(crate) const LIST_ICON_SIZE: u16 = 48;
pub(crate) const BAR_ICON_SIZE: u16 = 20;
pub(crate) const SIDEBAR_WIDTH: f32 = 240.0;
pub(crate) const SETTINGS_PANEL_WIDTH: f32 = 320.0;
pub(crate) const CORNER_BADGE_ICON_SIZE: u16 = 16;
pub(crate) static SEARCH_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);
pub(crate) static APP_SCROLL_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);

pub(crate) struct Applet {
    pub(crate) core: Core,
    pub(crate) popup: Option<Id>,
    /// Whether the app was started with `--window`.
    pub(crate) is_window_mode: bool,
    pub(crate) window_width: f32,
    pub(crate) window_height: f32,
    pub(crate) search_field: String,
    pub(crate) search_active: bool,
    pub(crate) all_applications: Vec<Arc<ApplicationEntry>>,
    pub(crate) apps_by_id: HashMap<String, Arc<ApplicationEntry>>,
    pub(crate) available_applications: Vec<Arc<ApplicationEntry>>,
    pub(crate) available_generation: u64,
    pub(crate) available_categories: Vec<ApplicationCategory>,
    pub(crate) selected_category: Option<ApplicationCategory>,
    pub(crate) config_context: Option<cosmic_config::Config>,
    pub(crate) config: AppletConfig,
    pub(crate) custom_width_input: String,
    pub(crate) custom_height_input: String,
    pub(crate) custom_size_selected: bool,
    pub(crate) selected_index: Option<usize>,
    pub(crate) nav_model: segmented_button::SingleSelectModel,
    pub(crate) sidebar_collapsed: bool,
    pub(crate) show_settings: bool,
    pub(crate) hovered_app_index: Option<usize>,
    pub(crate) hovered_pinned_id: Option<String>,
    pub(crate) pinned_apps: Vec<String>,
    pub(crate) app_scroll_y: f32,
    pub(crate) app_viewport_height: f32,
    pub(crate) icon_handles: Arc<view::AppIconHandles>,
    pub(crate) visible_row_cache: Option<view::VisibleRowCache>,
    pub(crate) panel_icon: cosmic::widget::icon::Handle,
    pub(crate) fav_ids: HashSet<String>,
    pub(crate) pinned_ids: HashSet<String>,
    pub(crate) app_refresh_in_progress: bool,
    pub(crate) activation_token_sender: Option<Sender<TokenRequest>>,
    pub(crate) pending_launches: HashMap<String, Arc<ApplicationEntry>>,
    pub(crate) next_launch_request: u64,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    TogglePopup(Rectangle),
    PopupClosed(Id),
    ClosePopup,
    OpenWindow,
    SearchInput(String),
    SearchCleared,
    ToggleSearch,
    LaunchApp(usize),
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    LaunchSelected,
    LayoutMode(LayoutMode),
    ToggleSidebar,
    ToggleSettings,
    SetSizePreset(SizePreset),
    SetCustomWidth(String),
    SetCustomHeight(String),
    ApplyCustomSize,
    SetPanelIcon(String),
    TogglePanelIconSymbolic,
    ToggleShowFavourites,
    ToggleShowRecents,
    ToggleShowBottomBarPinned,
    ToggleShowBottomBarPowerActions,
    LaunchAppById(String),
    ToggleFavourite(usize),
    PinToTray(usize),
    UnpinFromTray(usize),
    PinnedBarHovered(String),
    PinnedBarUnhovered(String),
    SetDefaultCategory(String),
    ToggleSidebarDefault,
    Surface(cosmic::surface::Action),
    PowerAction(power::PowerAction),
    PowerActionDone,
    ActivationToken(TokenUpdate),
    LaunchFinished,
    CategoryActivated(segmented_button::Entity),
    AppsScrolled(f32, f32),
    AppHovered(usize),
    AppUnhovered,
    ClearAppHover,
    RefreshApps,
    RefreshAppsFailed,
    AppsRefreshed(AppCatalog),
    UpdateConfig(AppletConfig),
}

#[derive(Clone, Debug)]
pub(crate) struct AppCatalog {
    pub apps: Vec<Arc<ApplicationEntry>>,
    pub categories: Vec<ApplicationCategory>,
    pub icons: Arc<view::AppIconHandles>,
}

fn reload_apps() -> Task<Message> {
    Task::future(async {
        match tokio::task::spawn_blocking(|| {
            view::warm_icon_option_handles();
            let apps = apps::load_apps();
            let categories = apps::load_categories(&apps);
            let icons = Arc::new(view::resolve_icon_handles(&apps));
            AppCatalog {
                apps,
                categories,
                icons,
            }
        })
        .await
        {
            Ok(catalog) => cosmic::action::app(Message::AppsRefreshed(catalog)),
            Err(err) => {
                tracing::warn!("Failed to reload applications: {err}");
                cosmic::action::app(Message::RefreshAppsFailed)
            }
        }
    })
}

fn message_update(applet: &mut Applet, message: Message) -> Task<Message> {
    match message {
        Message::Surface(action) => {
            return cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(
                action,
            )));
        }
        Message::TogglePopup(bounds) => {
            if let Some(popup) = applet.popup.take() {
                return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(
                    popup,
                ))));
            }
            applet.search_field.clear();
            applet.search_active = false;
            applet.selected_index = None;
            applet.app_scroll_y = 0.0;
            applet.app_viewport_height = 0.0;
            applet.sidebar_collapsed = applet.config.sidebar_collapsed;
            applet.select_default_category();
            applet.apply_category_filter();
            applet.rebuild_nav_model();

            let new_id = Id::unique();
            applet.popup = Some(new_id);

            let popup_width = applet.config.max_width() as u32;
            let popup_height = applet.config.max_height() as u32;
            let max_width = applet.config.max_width();
            let max_height = applet.config.max_height();

            Task::done(cosmic::Action::App(Message::Surface(app_popup::<Applet>(
                |_| LiveSettings::default(),
                move |state: &mut Applet| {
                    state.popup = Some(new_id);
                    let Some(parent) = state.core.main_window_id() else {
                        return state.core.applet.get_popup_settings(
                            Id::NONE,
                            new_id,
                            Some((popup_width, popup_height)),
                            None,
                            None,
                        );
                    };
                    let mut popup_settings = state.core.applet.get_popup_settings(
                        parent,
                        new_id,
                        Some((popup_width, popup_height)),
                        None,
                        None,
                    );
                    popup_settings.positioner.size = Some((popup_width, popup_height));
                    popup_settings.positioner.size_limits = Limits::NONE
                        .min_width(max_width)
                        .min_height(max_height)
                        .max_width(max_width)
                        .max_height(max_height);
                    popup_settings.positioner.anchor_rect = Rectangle {
                        x: bounds.x.round() as i32,
                        y: bounds.y.round() as i32,
                        width: bounds.width.round() as i32,
                        height: bounds.height.round() as i32,
                    };
                    popup_settings
                },
                None,
            ))))
        }
        Message::PopupClosed(id) => {
            if applet.popup == Some(id) {
                applet.popup = None;
            }
            Task::none()
        }
        Message::ClosePopup => {
            if let Some(popup) = applet.popup.take() {
                return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(
                    popup,
                ))));
            }
            Task::none()
        }
        Message::OpenWindow => {
            // Toplevels created through the applet's proxied connection are
            // embedded in the panel, so standalone mode needs a new process.
            let exe = std::env::current_exe()
                .unwrap_or_else(|_| std::path::PathBuf::from("cosmic-ext-kickoff-launcher"));
            let mut command = std::process::Command::new(exe);
            command.arg("--window");
            let spawn = launch::spawn_command(command);
            if let Some(popup) = applet.popup.take() {
                return Task::batch([
                    spawn,
                    Task::done(cosmic::Action::App(Message::Surface(destroy_popup(popup)))),
                ]);
            }
            spawn
        }
        Message::SearchInput(input) => {
            if input == applet.search_field {
                return Task::none();
            }
            applet.search_field = input;
            applet.selected_index = None;
            applet.hovered_app_index = None;
            applet.set_available_applications(if applet.search_field.is_empty() {
                applet.all_applications.clone()
            } else {
                apps::filter_apps(&applet.all_applications, &applet.search_field)
            });
            applet.select_all_category();
            applet.reset_app_scroll()
        }
        Message::SearchCleared => {
            applet.search_active = false;
            applet.show_all_apps();
            applet.reset_app_scroll()
        }
        Message::ToggleSearch => {
            applet.search_active = !applet.search_active;
            applet.show_all_apps();
            let reset = applet.reset_app_scroll();
            if applet.search_active {
                let focus_id = (*SEARCH_ID).clone();
                let focus = cosmic::widget::text_input::focus(focus_id);
                return Task::batch([focus, reset]);
            }
            reset
        }
        Message::CategoryActivated(entity) => {
            applet.hovered_app_index = None;
            applet.search_field.clear();
            applet.search_active = false;
            applet.selected_index = None;
            applet.nav_model.activate(entity);
            if let Some(cat) = applet.nav_model.data::<ApplicationCategory>(entity) {
                applet.selected_category = Some(cat.clone());
            }
            applet.apply_category_filter();
            applet.reset_app_scroll()
        }
        Message::AppsScrolled(y, viewport_height) => {
            let scroll_changed = (applet.app_scroll_y - y).abs() > f32::EPSILON;
            let viewport_changed =
                (applet.app_viewport_height - viewport_height).abs() > f32::EPSILON;
            if !scroll_changed && !viewport_changed {
                return Task::none();
            }
            applet.app_scroll_y = y;
            applet.app_viewport_height = viewport_height;
            if scroll_changed
                && applet.hovered_app_index.is_some_and(|index| {
                    !view::index_in_visible_rows(applet, index)
                })
            {
                applet.hovered_app_index = None;
            }
            Task::none()
        }
        Message::AppHovered(index) => {
            applet.hovered_app_index = Some(index);
            Task::none()
        }
        Message::AppUnhovered => {
            applet.hovered_app_index = None;
            Task::none()
        }
        Message::ClearAppHover => {
            applet.hovered_app_index = None;
            applet.hovered_pinned_id = None;
            Task::none()
        }
        Message::PinnedBarHovered(id) => {
            applet.hovered_pinned_id = Some(id);
            Task::none()
        }
        Message::PinnedBarUnhovered(id) => {
            if applet.hovered_pinned_id.as_deref() == Some(id.as_str()) {
                applet.hovered_pinned_id = None;
            }
            Task::none()
        }
        Message::LaunchApp(index) => {
            applet.hovered_app_index = None;
            if let Some(app) = applet.available_applications.get(index).cloned() {
                return applet.launch_application(app);
            }
            Task::none()
        }
        Message::SelectLeft => applet.move_selection(-1),
        Message::SelectRight => applet.move_selection(1),
        Message::SelectUp => applet.move_selection(-(view::navigation_columns(applet) as isize)),
        Message::SelectDown => applet.move_selection(view::navigation_columns(applet) as isize),
        Message::LaunchSelected => {
            if let Some(index) = applet.selected_index {
                if let Some(app) = applet.available_applications.get(index).cloned() {
                    return applet.launch_application(app);
                }
            }
            Task::none()
        }
        Message::LayoutMode(mode) => {
            applet.config.layout_mode = mode;
            applet.save_config();
            applet.reset_app_scroll()
        }
        Message::ToggleSidebar => {
            applet.sidebar_collapsed = !applet.sidebar_collapsed;
            applet.reset_app_scroll()
        }
        Message::ToggleSettings => {
            applet.show_settings = !applet.show_settings;
            Task::none()
        }
        Message::SetSizePreset(preset) => {
            if preset == SizePreset::Custom {
                applet.custom_size_selected = true;
                if applet.config.custom_width > 0.0 && applet.custom_width_input.is_empty() {
                    applet.custom_width_input = applet.config.custom_width.to_string();
                }
                if applet.config.custom_height > 0.0 && applet.custom_height_input.is_empty() {
                    applet.custom_height_input = applet.config.custom_height.to_string();
                }
                Task::none()
            } else {
                applet.custom_size_selected = false;
                applet.config.size_preset = preset;
                applet.config.custom_width = 0.0;
                applet.config.custom_height = 0.0;
                applet.custom_width_input.clear();
                applet.custom_height_input.clear();
                applet.save_config();
                Task::none()
            }
        }
        Message::SetCustomWidth(value) => {
            applet.custom_width_input = value;
            Task::none()
        }
        Message::SetCustomHeight(value) => {
            applet.custom_height_input = value;
            Task::none()
        }
        Message::ApplyCustomSize => {
            let parsed = applet
                .custom_width_input
                .trim()
                .parse::<f32>()
                .ok()
                .zip(applet.custom_height_input.trim().parse::<f32>().ok())
                .and_then(|(width, height)| validated_custom_size(width, height));
            if let Some((width, height)) = parsed {
                applet.config.custom_width = width;
                applet.config.custom_height = height;
                applet.custom_width_input = width.to_string();
                applet.custom_height_input = height.to_string();
                applet.custom_size_selected = false;
                applet.config.size_preset = SizePreset::Custom;
                applet.save_config();
                Task::none()
            } else {
                Task::none()
            }
        }
        Message::SetPanelIcon(icon) => {
            applet.config.panel_icon = icon;
            applet.panel_icon = panel_icon_handle(&applet.config);
            applet.save_config();
            Task::none()
        }
        Message::TogglePanelIconSymbolic => {
            applet.config.panel_icon_symbolic = !applet.config.panel_icon_symbolic;
            applet.panel_icon = panel_icon_handle(&applet.config);
            applet.save_config();
            Task::none()
        }
        Message::ToggleShowFavourites => {
            applet.config.show_favourites = !applet.config.show_favourites;
            applet.save_config();
            applet.rebuild_nav_model();
            Task::none()
        }
        Message::ToggleShowRecents => {
            applet.config.show_recents = !applet.config.show_recents;
            applet.save_config();
            applet.rebuild_nav_model();
            Task::none()
        }
        Message::ToggleShowBottomBarPinned => {
            applet.config.show_bottom_bar_pinned = !applet.config.show_bottom_bar_pinned;
            applet.save_config();
            Task::none()
        }
        Message::ToggleShowBottomBarPowerActions => {
            applet.config.show_bottom_bar_power_actions =
                !applet.config.show_bottom_bar_power_actions;
            applet.save_config();
            Task::none()
        }
        Message::LaunchAppById(id) => {
            if let Some(app) = applet.apps_by_id.get(&id).cloned() {
                return applet.launch_application(app);
            }
            let program = match id.as_str() {
                COSMIC_FILES_APP_ID => "cosmic-files",
                COSMIC_STORE_APP_ID => "cosmic-store",
                COSMIC_SETTINGS_APP_ID => "cosmic-settings",
                _ => return Task::none(),
            };
            launch::spawn_command(std::process::Command::new(program))
        }
        Message::ToggleSidebarDefault => {
            applet.config.sidebar_collapsed = !applet.config.sidebar_collapsed;
            applet.save_config();
            Task::none()
        }
        Message::SetDefaultCategory(cat) => {
            applet.config.default_category = cat;
            applet.save_config();
            Task::none()
        }
        Message::ToggleFavourite(index) => {
            if let Some(app) = applet.available_applications.get(index) {
                let app_id = app.id.clone();
                let was_empty = applet.config.favourites.is_empty();
                let now_fav = applet.config.toggle_favourite(&app_id);
                applet.save_config();
                if now_fav {
                    applet.fav_ids.insert(app_id.clone());
                } else {
                    applet.fav_ids.remove(&app_id);
                }
                if was_empty && now_fav && applet.config.default_category == "all" {
                    applet.config.default_category = "favourites".into();
                    applet.save_config();
                }
                applet.rebuild_nav_model();
                if applet
                    .selected_category
                    .as_ref()
                    .is_some_and(|category| category.key == "favourites")
                {
                    let mut favourites =
                        apps::filter_by_ids(&applet.apps_by_id, &applet.config.favourites);
                    favourites.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                    applet.set_available_applications(favourites);
                }
            }
            Task::none()
        }
        Message::PinToTray(index) => {
            if let Some(app) = applet.available_applications.get(index) {
                let app_id = app.id.clone();
                applet.pinned_apps = applet.pin_app_to_dock(&app_id);
                applet.pinned_ids.insert(app_id);
            }
            Task::none()
        }
        Message::UnpinFromTray(index) => {
            if let Some(app) = applet.available_applications.get(index) {
                let app_id = app.id.clone();
                applet.pinned_apps = applet.unpin_app_from_dock(&app_id);
                applet.pinned_ids.remove(&app_id);
            }
            Task::none()
        }
        Message::PowerAction(action) => {
            if let Some(popup) = applet.popup.take() {
                return Task::batch([
                    Task::done(cosmic::Action::App(Message::Surface(destroy_popup(popup)))),
                    power::execute(action),
                ]);
            }
            power::execute(action)
        }
        Message::PowerActionDone => Task::none(),
        Message::ActivationToken(update) => applet.handle_activation_token(update),
        Message::LaunchFinished => Task::none(),
        Message::RefreshApps => {
            if applet.app_refresh_in_progress {
                return Task::none();
            }
            applet.app_refresh_in_progress = true;
            reload_apps()
        }
        Message::RefreshAppsFailed => {
            applet.app_refresh_in_progress = false;
            Task::none()
        }
        Message::AppsRefreshed(catalog) => {
            applet.app_refresh_in_progress = false;
            let pinned_apps = dock::load_pinned_apps_with_defaults();
            applet.pinned_ids = pinned_apps.iter().cloned().collect();
            applet.pinned_apps = pinned_apps;
            if applet.all_applications == catalog.apps
                && applet.available_categories == catalog.categories
            {
                return Task::none();
            }
            applet.icon_handles = catalog.icons;
            applet.all_applications = catalog.apps;
            applet.apps_by_id = apps::index_by_id(&applet.all_applications);
            applet.available_categories = catalog.categories;
            if applet.is_window_mode && applet.selected_category.is_none() {
                applet.select_default_category();
            }
            applet.rebuild_nav_model();
            if applet.popup.is_some() || applet.is_window_mode {
                applet.apply_category_filter();
                if let Some(idx) = applet.selected_index {
                    if idx >= applet.available_applications.len() {
                        applet.selected_index = None;
                    }
                }
            }
            Task::none()
        }
        Message::UpdateConfig(config) => {
            let mut config = config;
            config.sanitize();
            applet.sidebar_collapsed = config.sidebar_collapsed;
            applet.config = config;
            applet.panel_icon = panel_icon_handle(&applet.config);
            applet.fav_ids = applet.config.favourites.iter().cloned().collect();
            if applet.config.size_preset == SizePreset::Custom {
                applet.custom_width_input = applet.config.custom_width.to_string();
                applet.custom_height_input = applet.config.custom_height.to_string();
            }
            applet.rebuild_nav_model();
            if applet.popup.is_some() || applet.is_window_mode {
                applet.apply_category_filter();
            }
            Task::none()
        }
    }
}

impl Application for Applet {
    type Executor = cosmic::executor::multi::Executor;
    type Flags = Flags;
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let (config_context, mut config) =
            match cosmic_config::Config::new(Self::APP_ID, AppletConfig::VERSION) {
                Ok(context) => {
                    let mut config = match AppletConfig::get_entry(&context) {
                        Ok(config) => config,
                        Err((_errors, config)) => config,
                    };
                    if config == AppletConfig::default() {
                        if let Ok(legacy_context) =
                            cosmic_config::Config::new(LEGACY_APP_ID, AppletConfig::VERSION)
                        {
                            if let Ok(legacy) = AppletConfig::get_entry(&legacy_context) {
                                config = legacy;
                                let _ = config.write_entry(&context);
                            }
                        }
                    }
                    if config == AppletConfig::default() {
                        if let Some(legacy) = AppletConfig::migrate_legacy(&context) {
                            config = legacy;
                        }
                    }
                    (Some(context), config)
                }
                Err(err) => {
                    tracing::warn!("Failed to open config context: {err}");
                    (None, AppletConfig::default())
                }
            };
        config.sanitize();

        let pinned_apps = dock::load_pinned_apps_with_defaults();
        let pinned_ids = pinned_apps.iter().cloned().collect();
        let fav_ids = config.favourites.iter().cloned().collect();

        let window_width = config.max_width();
        let window_height = config.max_height();
        let panel_icon = panel_icon_handle(&config);
        let mut applet = Self {
            core,
            popup: None,
            is_window_mode: flags.window,
            window_width,
            window_height,
            search_field: String::new(),
            search_active: false,
            all_applications: Vec::new(),
            apps_by_id: HashMap::new(),
            available_applications: Vec::new(),
            available_generation: 0,
            available_categories: Vec::new(),
            selected_category: None,
            config_context,
            config,
            custom_width_input: String::new(),
            custom_height_input: String::new(),
            custom_size_selected: false,
            selected_index: None,
            nav_model: segmented_button::SingleSelectModel::default(),
            sidebar_collapsed: false,
            show_settings: false,
            hovered_app_index: None,
            hovered_pinned_id: None,
            pinned_apps: pinned_apps.clone(),
            app_scroll_y: 0.0,
            app_viewport_height: 0.0,
            icon_handles: Arc::new(HashMap::new()),
            visible_row_cache: None,
            panel_icon,
            fav_ids,
            pinned_ids,
            app_refresh_in_progress: true,
            activation_token_sender: None,
            pending_launches: HashMap::new(),
            next_launch_request: 0,
        };
        applet.sidebar_collapsed = applet.config.sidebar_collapsed;
        if applet.config.size_preset == SizePreset::Custom {
            applet.custom_width_input = applet.config.custom_width.to_string();
            applet.custom_height_input = applet.config.custom_height.to_string();
        }
        applet.rebuild_nav_model();
        applet.sync_visible_rows();

        (applet, reload_apps())
    }

    fn view(&self) -> Element<'_, Message> {
        if self.is_window_mode {
            return self.build_menu_view(false);
        }
        let symbolic = self.config.panel_icon_symbolic;
        // libcosmic draws symbolic panel icons smaller than colour ones.
        let suggested = self.core.applet.suggested_size(false);
        let icon_size = suggested.0.min(suggested.1).saturating_sub(6);
        let icon = cosmic::widget::icon(self.panel_icon.clone())
            .size(icon_size)
            .class(if symbolic {
                cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                    color: Some(theme.cosmic().background(theme.transparent).on.into()),
                })
            } else {
                cosmic::theme::Svg::default()
            });
        let button = self
            .core
            .applet
            .button_from_element(icon, false)
            .on_press_with_rectangle(|offset, bounds| {
                Message::TogglePopup(Rectangle {
                    x: bounds.x - offset.x,
                    y: bounds.y - offset.y,
                    width: bounds.width,
                    height: bounds.height,
                })
            });
        self.core
            .applet
            .applet_tooltip(
                button,
                fl!("app-title"),
                self.popup.is_some(),
                Message::Surface,
                None,
            )
            .into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Message> {
        if self.popup == Some(id) {
            self.build_menu_view(true)
        } else {
            cosmic::widget::text("").into()
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        let task = message_update(self, message);
        self.sync_visible_rows();
        task
    }


    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn on_window_resize(&mut self, id: Id, width: f32, height: f32) {
        if self.is_window_mode && self.core.main_window_is(id) {
            self.window_width = width;
            self.window_height = height;
            self.sync_visible_rows();
        }
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        use cosmic::iced::time;
        use std::time::Duration;

        let mut subs = Vec::new();

        subs.push(activation_token_subscription(APP_ID).map(Message::ActivationToken));

        subs.push(
            time::every(Duration::from_secs(300)).map(|_| Message::RefreshApps),
        );

        if self.popup.is_some() || self.is_window_mode {
            subs.push(listen_raw(|event, status, _id| {
                let cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(key),
                    repeat,
                    ..
                }) = event
                else {
                    return None;
                };
                if repeat {
                    return None;
                }
                match key {
                    Named::Escape => Some(Message::ClosePopup),
                    Named::Enter => Some(Message::LaunchSelected),
                    Named::ArrowLeft if status == event::Status::Ignored => {
                        Some(Message::SelectLeft)
                    }
                    Named::ArrowRight if status == event::Status::Ignored => {
                        Some(Message::SelectRight)
                    }
                    Named::ArrowUp if status == event::Status::Ignored => Some(Message::SelectUp),
                    Named::ArrowDown if status == event::Status::Ignored => {
                        Some(Message::SelectDown)
                    }
                    _ => None,
                }
            }));
        }

        subs.push(
            self.core()
                .watch_config::<AppletConfig>(Self::APP_ID)
                .map(|update| {
                    Message::UpdateConfig(update.config)
                }),
        );

        cosmic::iced::Subscription::batch(subs)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        if !self.is_window_mode {
            return Vec::new();
        }
        let sidebar_toggle: Element<'_, Message> = nav_bar_toggle()
            .on_toggle(Message::ToggleSidebar)
            .active(!self.sidebar_collapsed)
            .into();
        let search_btn: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("system-search-symbolic")
                .symbolic(true)
                .size(18)
                .icon(),
        )
        .on_press(Message::ToggleSearch)
        .class(if self.search_active {
            cosmic::theme::Button::Suggested
        } else {
            cosmic::theme::Button::HeaderBar
        })
        .into();
        vec![sidebar_toggle, search_btn]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        if !self.is_window_mode {
            return Vec::new();
        }
        let config_btn: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("emblem-system-symbolic")
                .symbolic(true)
                .size(18)
                .icon(),
        )
        .on_press(Message::ToggleSettings)
        .class(if self.show_settings {
            cosmic::theme::Button::Suggested
        } else {
            cosmic::theme::Button::HeaderBar
        })
        .into();
        vec![config_btn]
    }
}

fn panel_icon_handle(config: &AppletConfig) -> cosmic::widget::icon::Handle {
    let name = if config.panel_icon.is_empty() {
        APP_ID
    } else {
        config.panel_icon.as_str()
    };
    panel_icons::handle(name, config.panel_icon_symbolic).unwrap_or_else(|| {
        cosmic::widget::icon::from_name(name)
            .symbolic(config.panel_icon_symbolic)
            .handle()
    })
}

impl Applet {
    pub(crate) fn save_config(&self) {
        if let Some(context) = &self.config_context {
            if let Err(err) = self.config.write_entry(context) {
                tracing::warn!("Failed to save config: {err}");
            }
        }
    }

    fn set_available_applications(&mut self, apps: Vec<Arc<ApplicationEntry>>) {
        self.available_applications = apps;
        self.available_generation = self.available_generation.wrapping_add(1);
    }

    fn apply_category_filter(&mut self) {
        let apps = if self.search_active {
            if self.search_field.is_empty() {
                self.all_applications.clone()
            } else {
                apps::filter_apps(&self.all_applications, &self.search_field)
            }
        } else if let Some(category) = &self.selected_category {
            match category.key.as_str() {
                "favourites" => {
                    let mut favs = apps::filter_by_ids(&self.apps_by_id, &self.config.favourites);
                    favs.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                    favs
                }
                "recents" => apps::filter_by_ids(&self.apps_by_id, &self.config.recents),
                _ => apps::filter_apps_by_category(&self.all_applications, category),
            }
        } else {
            self.all_applications.clone()
        };
        self.set_available_applications(apps);
    }

    fn select_default_category(&mut self) {
        self.selected_category = Some(match self.config.default_category.as_str() {
            "favourites" if self.config.show_favourites && !self.config.favourites.is_empty() => {
                ApplicationCategory::favourites()
            }
            "recents" if self.config.show_recents && !self.config.recents.is_empty() => {
                ApplicationCategory::recents()
            }
            key => self
                .available_categories
                .iter()
                .find(|category| category.key == key)
                .cloned()
                .unwrap_or_else(ApplicationCategory::all),
        });
    }

    fn select_all_category(&mut self) {
        self.selected_category = Some(ApplicationCategory::all());
        let all_entity = self.nav_model.iter().find(|&entity| {
            self.nav_model
                .data::<ApplicationCategory>(entity)
                .is_some_and(|category| category.key == "all")
        });
        if let Some(entity) = all_entity {
            self.nav_model.activate(entity);
        }
    }

    fn show_all_apps(&mut self) {
        self.search_field.clear();
        self.selected_index = None;
        self.hovered_app_index = None;
        self.set_available_applications(self.all_applications.clone());
        self.select_all_category();
    }

    pub(crate) fn reset_app_scroll(&mut self) -> Task<Message> {
        self.app_scroll_y = 0.0;
        self.app_viewport_height = 0.0;
        cosmic::iced::widget::scrollable::snap_to(
            (*APP_SCROLL_ID).clone(),
            cosmic::iced::widget::scrollable::RelativeOffset {
                x: Some(0.0),
                y: Some(0.0),
            },
        )
    }

    fn move_selection(&mut self, offset: isize) -> Task<Message> {
        let len = self.available_applications.len();
        if len == 0 {
            self.selected_index = None;
            return Task::none();
        }
        let current = self.selected_index.unwrap_or(0);
        let selected = current.saturating_add_signed(offset).min(len - 1);
        self.selected_index = Some(selected);

        let y = view::scroll_offset_for_index(self, selected);
        self.app_scroll_y = y;
        cosmic::iced::widget::scrollable::scroll_to(
            (*APP_SCROLL_ID).clone(),
            cosmic::iced::widget::scrollable::AbsoluteOffset {
                x: Some(0.0),
                y: Some(y),
            },
        )
    }

    pub(crate) fn rebuild_nav_model(&mut self) {
        let active_key = self
            .selected_category
            .as_ref()
            .map(|c| c.key.as_str())
            .unwrap_or("all");

        self.nav_model.clear();

        let all_icon = cosmic::widget::icon::from_name("user-home-symbolic")
            .symbolic(true)
            .size(16)
            .icon();
        self.nav_model
            .insert()
            .text(fl!("all-applications"))
            .icon(all_icon)
            .data(ApplicationCategory::all());

        if self.config.show_favourites && !self.config.favourites.is_empty() {
            let fav_icon = cosmic::widget::icon::from_name("starred-symbolic")
                .symbolic(true)
                .size(16)
                .icon();
            self.nav_model
                .insert()
                .text(fl!("favourites"))
                .icon(fav_icon)
                .data(ApplicationCategory::favourites());
        }

        if self.config.show_recents && !self.config.recents.is_empty() {
            let rec_icon = cosmic::widget::icon::from_name("document-open-recent-symbolic")
                .symbolic(true)
                .size(16)
                .icon();
            self.nav_model
                .insert()
                .text(fl!("recents"))
                .icon(rec_icon)
                .data(ApplicationCategory::recents());
        }

        for (i, cat) in self.available_categories.iter().enumerate() {
            let cat_icon = cosmic::widget::icon::from_name(std::sync::Arc::from(
                cat.icon_name.as_str(),
            ))
            .symbolic(true)
            .size(16)
            .icon();
            let mut entry = self
                .nav_model
                .insert()
                .text(cat.display_name.clone())
                .icon(cat_icon)
                .data(cat.clone());
            if i == 0 {
                entry = entry.divider_above(true);
            }
            let _ = entry;
        }

        let restore_entity = self.nav_model.iter().find(|&entity| {
            self.nav_model
                .data::<ApplicationCategory>(entity)
                .is_some_and(|c| c.key == active_key)
        });
        match restore_entity {
            Some(entity) => self.nav_model.activate(entity),
            None => {
                let all_entity = self.nav_model.iter().find(|&entity| {
                    self.nav_model
                        .data::<ApplicationCategory>(entity)
                        .is_some_and(|c| c.key == "all")
                });
                if let Some(entity) = all_entity {
                    self.nav_model.activate(entity);
                    if active_key != "all" {
                        self.selected_category = Some(ApplicationCategory::all());
                    }
                }
            }
        }
    }
}
