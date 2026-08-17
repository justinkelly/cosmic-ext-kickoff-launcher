// SPDX-License-Identifier: GPL-3.0-only

//! Application model and COSMIC application implementation.

use crate::config::{AppletConfig, LayoutMode, SizePreset};
use crate::{apps, dock, fl, power, view};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{
    event::{self, listen_raw},
    keyboard::{self, key::Named},
    window::Id,
    Limits,
};
use cosmic::surface::action::{app_popup, destroy_popup, LiveSettings};
use cosmic::widget::{nav_bar_toggle, segmented_button};
use cosmic::{Application, Element};
use apps::{ApplicationCategory, ApplicationEntry};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

pub const APP_ID: &str = "com.github.cosmic-kde-launcher";
pub const COSMIC_FILES_APP_ID: &str = "com.system76.CosmicFiles.desktop";
pub const COSMIC_SETTINGS_APP_ID: &str = "com.system76.CosmicSettings.desktop";
pub const GRID_ICON_SIZE: u16 = 64;
pub const LIST_ICON_SIZE: u16 = 48;
pub const SIDEBAR_WIDTH: f32 = 240.0;
pub const SETTINGS_PANEL_WIDTH: f32 = 280.0;
pub const CORNER_BADGE_ICON_SIZE: u16 = 12;
pub static SEARCH_ID: LazyLock<cosmic::widget::Id> = LazyLock::new(cosmic::widget::Id::unique);
pub static SCROLLABLE_ID: LazyLock<cosmic::widget::Id> = LazyLock::new(cosmic::widget::Id::unique);

pub struct Applet {
    pub(crate) core: Core,
    pub(crate) popup: Option<Id>,
    /// True when running as a standalone window (`--window`), false when
    /// running as a panel applet.
    pub(crate) is_window_mode: bool,
    pub(crate) search_field: String,
    pub(crate) search_active: bool,
    pub(crate) all_applications: Vec<Arc<ApplicationEntry>>,
    pub(crate) available_applications: Vec<Arc<ApplicationEntry>>,
    pub(crate) available_categories: Vec<ApplicationCategory>,
    pub(crate) selected_category: Option<ApplicationCategory>,
    /// cosmic-config context used to persist settings.
    pub(crate) config_context: Option<cosmic_config::Config>,
    pub(crate) config: AppletConfig,
    pub(crate) custom_width_input: String,
    pub(crate) custom_height_input: String,
    pub(crate) selected_index: Option<usize>,
    pub(crate) nav_model: segmented_button::SingleSelectModel,
    /// Layout-mode control in the settings panel.
    pub(crate) layout_model: segmented_button::SingleSelectModel,
    pub(crate) sidebar_collapsed: bool,
    pub(crate) show_settings: bool,
    /// App index currently under the pointer (for hover action icons).
    pub(crate) hovered_app_index: Option<usize>,
    /// Pinned app ID under the pointer in the bottom bar.
    pub(crate) hovered_pinned_id: Option<String>,
    pub(crate) pinned_apps: Vec<String>,
    /// Scroll offset (px) of the app-grid scrollable, tracked for virtualized
    /// rendering so only rows in the viewport are built each view.
    pub(crate) grid_scroll_y: f32,
    /// Height (px) of the app-grid viewport, from the scrollable's viewport.
    pub(crate) grid_viewport_h: f32,
    pub(crate) icon_cache: RefCell<HashMap<(String, u16), cosmic::widget::icon::Icon>>,
    pub(crate) cached_fav_ids: RefCell<HashSet<String>>,
    pub(crate) cached_pinned_ids: RefCell<HashSet<String>>,
    /// Set of .desktop file basenames known at last scan — used to detect
    /// new/removed apps without a full re-parse.
    pub(crate) known_desktop_ids: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ClosePopup,
    OpenWindow,
    SearchInput(String),
    SearchCleared,
    ToggleSearch,
    LaunchApp(usize),
    SelectNext,
    SelectPrevious,
    LaunchSelected,
    LayoutActivated(segmented_button::Entity),
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
    UnpinFromTrayById(String),
    PinnedBarHovered(String),
    PinnedBarUnhovered(String),
    SetDefaultCategory(String),
    ToggleSidebarDefault,
    Surface(cosmic::surface::Action),
    PowerAction(power::PowerAction),
    PowerActionDone,
    CategoryActivated(segmented_button::Entity),
    /// App grid scrolled — carries the absolute vertical offset (px) and the
    /// viewport height (px), used to render only visible rows.
    GridScrolled(f32, f32),
    AppHovered(usize),
    AppUnhovered(usize),
    ClearAppHover,
    /// Periodic app-list refresh — detects newly installed/removed apps.
    RefreshApps,
    RefreshAppsFailed,
    /// Result of a background app-list reload.
    AppsRefreshed(Vec<Arc<ApplicationEntry>>, Vec<ApplicationCategory>),
    /// Config updated externally (cosmic-settings-daemon).
    UpdateConfig(AppletConfig),
}

impl Application for Applet {
    type Executor = cosmic::executor::multi::Executor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        // Load config from cosmic-config, migrating the legacy TOML if needed.
        let (config_context, mut config) =
            match cosmic_config::Config::new(Self::APP_ID, AppletConfig::VERSION) {
                Ok(context) => {
                    let mut config = match AppletConfig::get_entry(&context) {
                        Ok(config) => config,
                        Err((_errors, config)) => config,
                    };
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

        let apps = apps::load_apps();
        let categories = apps::load_categories(&apps);
        tracing::info!("Preloaded {} apps, {} categories", apps.len(), categories.len());

        // Load pinned apps from CosmicDock (Files + Settings shown by default).
        let pinned_apps = dock::load_pinned_apps_with_defaults();

        let mut applet = Self {
            core,
            popup: None,
            is_window_mode: std::env::args().any(|a| a == "--window"),
            search_field: String::new(),
            search_active: false,
            all_applications: apps.clone(),
            available_applications: apps,
            available_categories: categories,
            selected_category: None,
            config_context,
            config,
            custom_width_input: String::new(),
            custom_height_input: String::new(),            selected_index: None,
            nav_model: segmented_button::SingleSelectModel::default(),
            layout_model: Self::build_layout_model(),
            sidebar_collapsed: false,
            show_settings: false,
            hovered_app_index: None,
            hovered_pinned_id: None,
            pinned_apps: pinned_apps.clone(),
            grid_scroll_y: 0.0,
            grid_viewport_h: 0.0,
            icon_cache: RefCell::new(HashMap::new()),
            cached_fav_ids: RefCell::new(HashSet::new()),
            cached_pinned_ids: RefCell::new(HashSet::new()),
            known_desktop_ids: apps::list_desktop_ids(),
        };
        applet.sidebar_collapsed = applet.config.sidebar_collapsed;
        // Prefill the custom-size inputs when the Custom preset is active.
        if applet.config.size_preset == SizePreset::Custom {
            applet.custom_width_input = applet.config.custom_width.to_string();
            applet.custom_height_input = applet.config.custom_height.to_string();
        }
        for id in &pinned_apps {
            applet.cached_pinned_ids.borrow_mut().insert(id.clone());
        }
        for id in &applet.config.favourites {
            applet.cached_fav_ids.borrow_mut().insert(id.clone());
        }
        // Activate the configured layout mode in the settings control.
        let active_entity = applet.layout_model.iter().find(|&entity| {
            applet
                .layout_model
                .data::<LayoutMode>(entity)
                .is_some_and(|mode| *mode == applet.config.layout_mode)
        });
        if let Some(entity) = active_entity {
            applet.layout_model.activate(entity);
        }
        applet.rebuild_nav_model();

        (applet, Task::none())
    }

    fn view(&self) -> Element<'_, Message> {
        // Standalone window mode: the main window IS the menu.
        if self.is_window_mode {
            return self.build_menu_view(false);
        }
        let icon_name: &str = if self.config.panel_icon.is_empty() {
            "com.system76.CosmicAppLibrary"
        } else {
            &self.config.panel_icon
        };
        // Use icon_button_from_handle with configurable symbolic/coloured.
        let suggested = self.core.applet.suggested_size(true);
        self.core
            .applet
            .icon_button_from_handle(
                cosmic::widget::icon::from_name(icon_name)
                    .symbolic(self.config.panel_icon_symbolic)
                    .size(suggested.0)
                    .into(),
            )
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        // Only the popup is rendered through this path.
        self.build_menu_view(true)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(
                    a,
                )));
            }
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
                }
                self.search_field.clear();
                self.search_active = false;
                self.selected_index = None;
                self.grid_scroll_y = 0.0;
                self.grid_viewport_h = 0.0;
                self.sidebar_collapsed = self.config.sidebar_collapsed;
                // Apps are kept up to date by the periodic RefreshApps subscription.
                self.available_applications = self.all_applications.clone();

                // Apply default category from config
                self.selected_category = Some(match self.config.default_category.as_str() {
                    "favourites" => ApplicationCategory::favourites(),
                    "recents" => ApplicationCategory::recents(),
                    _ => ApplicationCategory::all(),
                });
                self.apply_category_filter();
                self.rebuild_nav_model();

                let new_id = Id::unique();
                self.popup = Some(new_id);

                let popup_width = self.config.max_width() as u32;
                let popup_height = self.config.max_height() as u32;
                let max_width = self.config.max_width();
                let max_height = self.config.max_height();

                Task::done(cosmic::Action::App(Message::Surface(app_popup::<Applet>(
                    |_| LiveSettings::default(),
                    move |state: &mut Applet| {
                        state.popup = Some(new_id);
                        let mut popup_settings = state.core.applet.get_popup_settings(
                            state.core.main_window_id().unwrap(),
                            new_id,
                            Some((popup_width, popup_height)),
                            None,
                            None,
                        );
                        // Use canonical anchor/gravity from get_popup_settings,
                        // but override size and size_limits for our configurable menu.
                        popup_settings.positioner.size = Some((popup_width, popup_height));
                        popup_settings.positioner.size_limits = Limits::NONE
                            .min_width(max_width)
                            .min_height(max_height)
                            .max_width(max_width)
                            .max_height(max_height);
                        popup_settings
                    },
                    None,
                ))))
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                Task::none()
            }
            Message::ClosePopup => {
                if let Some(p) = self.popup.take() {
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
                }
                Task::none()
            }
            Message::OpenWindow => {
                // The applet runs on the panel's proxied Wayland connection:
                // any toplevel created here is embedded at (0,0) inside the
                // panel instead of reaching the compositor. Launch a separate
                // process so the menu opens as a real window on the actual
                // compositor, then close the popup.
                let exe = std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("cosmic-kde-launcher"));
                if let Err(err) = std::process::Command::new(exe).arg("--window").spawn() {
                    tracing::warn!("Failed to launch menu window: {err}");
                }
                if let Some(p) = self.popup.take() {
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
                }
                Task::none()
            }
            Message::SearchInput(input) => {
                // Guard: skip if input hasn't changed.
                if input == self.search_field {
                    return Task::none();
                }
                self.search_field = input.clone();
                self.selected_index = None;
                self.grid_scroll_y = 0.0;
                self.available_applications = if input.is_empty() {
                    self.all_applications.clone()
                } else {
                    apps::filter_apps(&self.all_applications, &input)
                };
                // Keep "All Applications" selected while searching
                self.selected_category = Some(ApplicationCategory::all());
                let all_entity = self.nav_model.iter().find(|&entity| {
                    self.nav_model
                        .data::<ApplicationCategory>(entity)
                        .map_or(false, |cat| cat.key == "all")
                });
                if let Some(entity) = all_entity {
                    self.nav_model.activate(entity);
                }
                self.reset_grid_scroll()
            }
            Message::SearchCleared => {
                self.search_field.clear();
                self.search_active = false;
                self.selected_index = None;
                self.available_applications = self.all_applications.clone();
                self.selected_category = Some(ApplicationCategory::all());
                // Find and activate the "All" entity
                let all_entity = self.nav_model.iter().find(|&entity| {
                    self.nav_model
                        .data::<ApplicationCategory>(entity)
                        .map_or(false, |cat| cat.key == "all")
                });
                if let Some(entity) = all_entity {
                    self.nav_model.activate(entity);
                }
                self.reset_grid_scroll()
            }
            Message::ToggleSearch => {
                self.search_active = !self.search_active;
                if self.search_active {
                    self.search_field.clear();
                    self.selected_index = None;
                    self.available_applications = self.all_applications.clone();
                    // Keep "All Applications" selected in sidebar
                    self.selected_category = Some(ApplicationCategory::all());
                    let all_entity = self.nav_model.iter().find(|&entity| {
                        self.nav_model
                            .data::<ApplicationCategory>(entity)
                            .map_or(false, |cat| cat.key == "all")
                    });
                    if let Some(entity) = all_entity {
                        self.nav_model.activate(entity);
                    }
                    let focus_id = (*SEARCH_ID).clone();
                    let focus = cosmic::widget::text_input::focus(focus_id);
                    let reset = self.reset_grid_scroll();
                    return Task::batch([focus, reset]);
                }
                // Closing search — restore "All Applications"
                self.search_field.clear();
                self.selected_index = None;
                self.available_applications = self.all_applications.clone();
                self.selected_category = Some(ApplicationCategory::all());
                let all_entity = self.nav_model.iter().find(|&entity| {
                    self.nav_model
                        .data::<ApplicationCategory>(entity)
                        .map_or(false, |cat| cat.key == "all")
                });
                if let Some(entity) = all_entity {
                    self.nav_model.activate(entity);
                }
                self.reset_grid_scroll()
            }
            Message::CategoryActivated(entity) => {
                self.hovered_app_index = None;
                self.search_field.clear();
                self.search_active = false;
                self.selected_index = None;
                self.nav_model.activate(entity);
                if let Some(cat) = self.nav_model.data::<ApplicationCategory>(entity) {
                    self.selected_category = Some(cat.clone());
                }
                self.apply_category_filter();
                self.reset_grid_scroll()
            }
            Message::GridScrolled(y, viewport_h) => {
                let scroll_changed = (self.grid_scroll_y - y).abs() > f32::EPSILON;
                self.grid_scroll_y = y;
                self.grid_viewport_h = viewport_h;
                if scroll_changed {
                    self.hovered_app_index = None;
                }
                Task::none()
            }
            Message::AppHovered(index) => {
                self.hovered_app_index = Some(index);
                Task::none()
            }
            Message::AppUnhovered(_index) => {
                self.hovered_app_index = None;
                Task::none()
            }
            Message::ClearAppHover => {
                self.hovered_app_index = None;
                self.hovered_pinned_id = None;
                Task::none()
            }
            Message::PinnedBarHovered(id) => {
                self.hovered_pinned_id = Some(id);
                Task::none()
            }
            Message::PinnedBarUnhovered(id) => {
                if self.hovered_pinned_id.as_deref() == Some(id.as_str()) {
                    self.hovered_pinned_id = None;
                }
                Task::none()
            }
            Message::UnpinFromTrayById(id) => {
                self.pinned_apps = self.unpin_app_from_dock(&id);
                self.cached_pinned_ids.borrow_mut().remove(&id);
                self.hovered_pinned_id = None;
                Task::none()
            }
            Message::LaunchApp(index) => {
                self.hovered_app_index = None;
                if let Some(app) = self.available_applications.get(index).cloned() {
                    return self.launch_application(app);
                }
                Task::none()
            }
            Message::SelectNext => {
                if self.available_applications.is_empty() {
                    return Task::none();
                }
                match self.selected_index {
                    None => self.selected_index = Some(0),
                    Some(i) if i + 1 < self.available_applications.len() => {
                        self.selected_index = Some(i + 1);
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::SelectPrevious => {
                match self.selected_index {
                    Some(0) | None => self.selected_index = None,
                    Some(i) => self.selected_index = Some(i - 1),
                }
                Task::none()
            }
            Message::LaunchSelected => {
                if let Some(i) = self.selected_index {
                    if let Some(app) = self.available_applications.get(i).cloned() {
                        return self.launch_application(app);
                    }
                }
                Task::none()
            }
            Message::LayoutActivated(entity) => {
                if let Some(mode) = self.layout_model.data::<LayoutMode>(entity) {
                    self.config.layout_mode = *mode;
                    self.save_config();
                }
                self.layout_model.activate(entity);
                Task::none()
            }
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                // The category-title row appears/disappears above the app grid,
                // changing the grid viewport height — re-sync the scroll state.
                self.reset_grid_scroll()
            }
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }
            Message::SetSizePreset(preset) => {
                self.config.size_preset = preset;
                if preset == SizePreset::Custom {
                    // Prefill with the last applied custom values, if any.
                    if self.config.custom_width > 0.0 && self.custom_width_input.is_empty() {
                        self.custom_width_input = self.config.custom_width.to_string();
                    }
                    if self.config.custom_height > 0.0 && self.custom_height_input.is_empty() {
                        self.custom_height_input = self.config.custom_height.to_string();
                    }
                } else {
                    // Non-custom presets ignore custom dimensions — clear them
                    // so the preset's own size takes effect.
                    self.config.custom_width = 0.0;
                    self.config.custom_height = 0.0;
                    self.custom_width_input.clear();
                    self.custom_height_input.clear();
                }
                self.save_config();
                self.reset_grid_scroll()
            }
            Message::SetCustomWidth(value) => {
                self.custom_width_input = value;
                Task::none()
            }
            Message::SetCustomHeight(value) => {
                self.custom_height_input = value;
                Task::none()
            }
            Message::ApplyCustomSize => {
                // Parse both inputs; valid values are clamped to the same
                // bounds as config sanitization and echoed back to the inputs.
                let mut applied = false;
                if let Ok(w) = self.custom_width_input.trim().parse::<f32>() {
                    let w = w.max(400.0);
                    self.config.custom_width = w;
                    self.custom_width_input = w.to_string();
                    applied = true;
                }
                if let Ok(h) = self.custom_height_input.trim().parse::<f32>() {
                    let h = h.max(300.0);
                    self.config.custom_height = h;
                    self.custom_height_input = h.to_string();
                    applied = true;
                }
                if applied {
                    self.config.size_preset = SizePreset::Custom;
                    self.save_config();
                    // The view rebuilds with the new popup size, which the
                    // popup's autosize limits propagate to the compositor.
                    self.reset_grid_scroll()
                } else {
                    Task::none()
                }
            }
            Message::SetPanelIcon(icon) => {
                self.config.panel_icon = icon;
                self.save_config();
                Task::none()
            }
            Message::TogglePanelIconSymbolic => {
                self.config.panel_icon_symbolic = !self.config.panel_icon_symbolic;
                self.save_config();
                Task::none()
            }
            Message::ToggleShowFavourites => {
                self.config.show_favourites = !self.config.show_favourites;
                self.save_config();
                self.rebuild_nav_model();
                Task::none()
            }
            Message::ToggleShowRecents => {
                self.config.show_recents = !self.config.show_recents;
                self.save_config();
                self.rebuild_nav_model();
                Task::none()
            }
            Message::ToggleShowBottomBarPinned => {
                self.config.show_bottom_bar_pinned = !self.config.show_bottom_bar_pinned;
                self.save_config();
                Task::none()
            }
            Message::ToggleShowBottomBarPowerActions => {
                self.config.show_bottom_bar_power_actions = !self.config.show_bottom_bar_power_actions;
                self.save_config();
                Task::none()
            }
            Message::LaunchAppById(id) => {
                if let Some(app) = self.all_applications.iter().find(|a| a.id == id).cloned() {
                    return self.launch_application(app);
                }
                let program = match id.as_str() {
                    COSMIC_FILES_APP_ID => "cosmic-files",
                    COSMIC_SETTINGS_APP_ID => "cosmic-settings",
                    _ => return Task::none(),
                };
                if let Err(e) = std::process::Command::new(program).spawn() {
                    tracing::warn!("Failed to launch '{}': {}", program, e);
                }
                Task::none()
            }
            Message::ToggleSidebarDefault => {
                self.config.sidebar_collapsed = !self.config.sidebar_collapsed;
                self.save_config();
                Task::none()
            }
            Message::SetDefaultCategory(cat) => {
                self.config.default_category = cat;
                self.save_config();
                Task::none()
            }
            Message::ToggleFavourite(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    let was_empty = self.config.favourites.is_empty();
                    let now_fav = self.config.toggle_favourite(&app_id);
                    self.save_config();
                    if now_fav {
                        self.cached_fav_ids.borrow_mut().insert(app_id.clone());
                    } else {
                        self.cached_fav_ids.borrow_mut().remove(&app_id);
                    }
                    if was_empty && now_fav && self.config.default_category == "all" {
                        self.config.default_category = "favourites".into();
                        self.save_config();
                    }
                    // Rebuild nav to show/hide Favourites entry
                    self.rebuild_nav_model();
                    // If viewing favourites and unfavourited, refresh list
                    if let Some(ref cat) = self.selected_category {
                        if cat.key == "favourites" {
                            let mut favs = apps::filter_by_ids(
                                &self.all_applications,
                                &self.config.favourites,
                            );
                            favs.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                            self.available_applications = favs;
                        }
                    }
                }
                Task::none()
            }
            Message::PinToTray(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    self.pinned_apps = self.pin_app_to_dock(&app_id);
                    self.cached_pinned_ids.borrow_mut().insert(app_id);
                }
                Task::none()
            }
            Message::UnpinFromTray(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    self.pinned_apps = self.unpin_app_from_dock(&app_id);
                    self.cached_pinned_ids.borrow_mut().remove(&app_id);
                }
                Task::none()
            }
            Message::PowerAction(action) => {
                if let Some(p) = self.popup.take() {
                    return Task::batch([
                        Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p)))),
                        power::execute(action),
                    ]);
                }
                power::execute(action)
            }
            Message::PowerActionDone => Task::none(),
            Message::RefreshApps => {
                let current = apps::list_desktop_ids();
                if current == self.known_desktop_ids {
                    return Task::none();
                }
                // Desktop files changed — reload in background, then update state.
                return Task::future(async move {
                    let (apps, categories) = match tokio::task::spawn_blocking(|| {
                        let a = apps::load_apps();
                        let c = apps::load_categories(&a);
                        (a, c)
                    })
                    .await
                    {
                        Ok((a, c)) => (a, c),
                        Err(err) => {
                            tracing::warn!("App reload task failed: {err}");
                            return cosmic::action::app(Message::RefreshAppsFailed);
                        }
                    };
                    cosmic::action::app(Message::AppsRefreshed(apps, categories))
                });
            }
            Message::RefreshAppsFailed => Task::none(),
            Message::AppsRefreshed(apps, categories) => {
                self.all_applications = apps;
                self.available_categories = categories;
                self.known_desktop_ids = apps::list_desktop_ids();
                self.rebuild_nav_model();
                // If popup is open, re-apply current view filters.
                if self.popup.is_some() {
                    self.apply_category_filter();
                    // Deselect if the index is now out of bounds.
                    if let Some(idx) = self.selected_index {
                        if idx >= self.available_applications.len() {
                            self.selected_index = None;
                        }
                    }
                }
                Task::none()
            }
            Message::UpdateConfig(config) => {
                self.config = config;
                self.rebuild_nav_model();
                if self.popup.is_some() {
                    self.apply_category_filter();
                }
                Task::none()
            }
        }
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        use cosmic::iced::time;
        use std::time::Duration;

        let mut subs = Vec::new();

        // Periodic app-list refresh — detects newly installed/removed apps.
        subs.push(
            time::every(Duration::from_secs(10)).map(|_| Message::RefreshApps),
        );

        // Keyboard shortcuts — only when the popup or a standalone window is open.
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
                    // Only when not captured — don't fight text-input caret movement.
                    Named::ArrowDown | Named::ArrowRight
                        if status == event::Status::Ignored =>
                    {
                        Some(Message::SelectNext)
                    }
                    Named::ArrowUp | Named::ArrowLeft if status == event::Status::Ignored => {
                        Some(Message::SelectPrevious)
                    }
                    _ => None,
                }
            }));
        }

        // Watch for application configuration changes.
        subs.push(
            self.core()
                .watch_config::<AppletConfig>(Self::APP_ID)
                .map(|update| {
                    // for why in update.errors {
                    //     tracing::error!(?why, "app config error");
                    // }
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

impl Applet {
    /// Persist the config through cosmic-config (propagates to other instances
    /// via cosmic-settings-daemon thanks to the `dbus-config` feature).
    pub fn save_config(&self) {
        if let Some(context) = &self.config_context {
            if let Err(err) = self.config.write_entry(context) {
                tracing::warn!("Failed to save config: {err}");
            }
        }
    }

    /// Re-apply the current search/category filter to `available_applications`.
    fn apply_category_filter(&mut self) {
        if self.search_active {
            let input = self.search_field.clone();
            self.available_applications = if input.is_empty() {
                self.all_applications.clone()
            } else {
                apps::filter_apps(&self.all_applications, &input)
            };
        } else if let Some(cat) = self.selected_category.clone() {
            self.available_applications = match cat.key.as_str() {
                "favourites" => {
                    let mut favs =
                        apps::filter_by_ids(&self.all_applications, &self.config.favourites);
                    favs.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                    favs
                }
                "recents" => apps::filter_by_ids(&self.all_applications, &self.config.recents),
                _ => apps::filter_apps_by_category(&self.all_applications, &cat),
            };
        } else {
            self.available_applications = self.all_applications.clone();
        }
    }

    /// Reset the app-grid scroll position and snap the scrollable back to the
    /// top, keeping the tracked offset in sync with the widget's internal one.
    /// The viewport height is also forgotten so the safe fallback (menu height)
    /// is used until the next scroll event re-measures it — this avoids a stale
    /// (smaller) viewport rendering a blank bottom after the layout changes.
    pub fn reset_grid_scroll(&mut self) -> Task<Message> {
        self.grid_scroll_y = 0.0;
        self.grid_viewport_h = 0.0;
        cosmic::iced::widget::scrollable::snap_to(
            (*SCROLLABLE_ID).clone(),
            cosmic::iced::widget::scrollable::RelativeOffset {
                x: Some(0.0),
                y: Some(0.0),
            },
        )
    }

    pub fn cached_icon(&self, app: &ApplicationEntry, size: f32) -> cosmic::widget::icon::Icon {
        let size_u16 = size as u16;
        let icon_name = app.icon.clone().unwrap_or_default();
        let key = (icon_name, size_u16);
        {
            let cache = self.icon_cache.borrow();
            if let Some(icon) = cache.get(&key) {
                return icon.clone();
            }
        }
        let icon = view::app_icon(app, size);
        self.icon_cache.borrow_mut().insert(key, icon.clone());
        icon
    }

    fn build_layout_model() -> segmented_button::SingleSelectModel {
        let mut model = segmented_button::SingleSelectModel::default();

        let icon = |name: &'static str| {
            cosmic::widget::icon::from_name(name)
                .symbolic(true)
                .size(16)
                .icon()
        };

        model
            .insert()
            .text(fl!("grid-view"))
            .icon(icon("view-grid-symbolic"))
            .data(LayoutMode::Grid);
        model
            .insert()
            .text(fl!("list-view"))
            .icon(icon("view-list-symbolic"))
            .data(LayoutMode::List);
        model
            .insert()
            .text(fl!("hybrid-view"))
            .icon(icon("view-grid-symbolic"))
            .data(LayoutMode::Hybrid);

        model
    }

    pub(crate) fn rebuild_nav_model(&mut self) {
        let active_key = self
            .selected_category
            .as_ref()
            .map(|c| c.key.as_str())
            .unwrap_or("all");

        self.nav_model.clear();

        // "All Applications" entry
        let all_icon = cosmic::widget::icon::from_name("user-home-symbolic")
            .symbolic(true)
            .size(16)
            .icon();
        self.nav_model
            .insert()
            .text(fl!("all-applications"))
            .icon(all_icon)
            .data(ApplicationCategory::all());

        // "Favourites" entry (if any and enabled)
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

        // "Recents" entry (if any and enabled)
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

        // Category entries (with divider above the first one)
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

        // Restore the sidebar selection instead of always jumping to All Applications.
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
