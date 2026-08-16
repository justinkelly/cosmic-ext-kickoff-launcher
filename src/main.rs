// SPDX-License-Identifier: GPL-3.0-only

mod apps;
mod config;

use config::AppletConfig;

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::ConfigGet;
use cosmic::cosmic_config::ConfigSet;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{
    event::listen_raw,
    keyboard::key::Named,
    widget::{column, container, row, scrollable, stack, Space},
    window::Id,
    Alignment, Length, Limits,
};
use cosmic::surface::action::{app_popup, destroy_popup, LiveSettings};
use cosmic::theme;
use cosmic::widget::{
    dropdown, mouse_area, nav_bar, nav_bar_toggle, segmented_button,
};
use cosmic::{Application, Element};

use apps::{ApplicationCategory, ApplicationEntry};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

const APP_ID: &str = "com.github.cosmic-kde-launcher";
const COSMIC_FILES_APP_ID: &str = "com.system76.CosmicFiles.desktop";
const COSMIC_SETTINGS_APP_ID: &str = "com.system76.CosmicSettings.desktop";
const GRID_ICON_SIZE: u16 = 64;
const LIST_ICON_SIZE: u16 = 48;
const SIDEBAR_WIDTH: f32 = 240.0;
const CORNER_BADGE_ICON_SIZE: u16 = 12;
static SEARCH_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);
static SCROLLABLE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Logout,
    Suspend,
    Restart,
    Shutdown,
}

impl PowerAction {
    fn icon_name(&self) -> &'static str {
        match self {
            PowerAction::Lock => "system-lock-screen-symbolic",
            PowerAction::Logout => "system-log-out-symbolic",
            PowerAction::Suspend => "system-suspend-symbolic",
            PowerAction::Restart => "system-reboot-symbolic",
            PowerAction::Shutdown => "system-shutdown-symbolic",
        }
    }
    fn label(&self) -> &'static str {
        match self {
            PowerAction::Lock => "Lock",
            PowerAction::Logout => "Log Out",
            PowerAction::Suspend => "Suspend",
            PowerAction::Restart => "Restart",
            PowerAction::Shutdown => "Shut Down",
        }
    }

    /// Bottom-bar actions matching the standard COSMIC session/power applet.
    const BOTTOM_BAR: [Self; 5] = [
        Self::Lock,
        Self::Logout,
        Self::Suspend,
        Self::Restart,
        Self::Shutdown,
    ];
}

pub struct Applet {
    core: Core,
    popup: Option<Id>,
    is_window_mode: bool,
    search_field: String,
    search_active: bool,
    all_applications: Vec<Arc<ApplicationEntry>>,
    available_applications: Vec<Arc<ApplicationEntry>>,
    available_categories: Vec<ApplicationCategory>,
    selected_category: Option<ApplicationCategory>,
    config: AppletConfig,
    custom_width_input: String,
    custom_height_input: String,
    selected_index: Option<usize>,
    nav_model: segmented_button::SingleSelectModel,
    sidebar_collapsed: bool,
    show_settings: bool,
    /// App index currently under the pointer (for hover action icons).
    hovered_app_index: Option<usize>,
    /// Pinned app ID under the pointer in the bottom bar.
    hovered_pinned_id: Option<String>,
    pinned_apps: Vec<String>,
    /// Scroll offset (px) of the app-grid scrollable, tracked for virtualized
    /// rendering so only rows in the viewport are built each view.
    grid_scroll_y: f32,
    /// Height (px) of the app-grid viewport, from the scrollable's viewport.
    grid_viewport_h: f32,
    icon_cache: RefCell<HashMap<(String, u16), cosmic::widget::icon::Icon>>,
    cached_fav_ids: RefCell<HashSet<String>>,
    cached_pinned_ids: RefCell<HashSet<String>>,
    /// Set of .desktop file basenames known at last scan — used to detect
    /// new/removed apps without a full re-parse.
    known_desktop_ids: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ClosePopup,
    LaunchWindow,
    SearchInput(String),
    SearchCleared,
    ToggleSearch,
    LaunchApp(usize),
    SelectNext,
    SelectPrevious,
    LaunchSelected,
    ToggleLayout(config::LayoutMode),
    ToggleSidebar,
    ToggleSettings,
    SetSizePreset(config::SizePreset),
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
    PowerAction(PowerAction),
    CategoryActivated(segmented_button::Entity),
    /// App grid scrolled — carries the absolute vertical offset (px) and the
    /// viewport height (px), used to render only visible rows.
    GridScrolled(f32, f32),
    WindowResized(cosmic::iced::Size),
    AppHovered(usize),
    AppUnhovered(usize),
    ClearAppHover,
    /// Periodic app-list refresh — detects newly installed/removed apps.
    RefreshApps,
    /// Result of a background app-list reload.
    AppsRefreshed(Vec<Arc<ApplicationEntry>>, Vec<ApplicationCategory>),
}

impl Application for Applet {
    type Executor = cosmic::executor::multi::Executor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config = AppletConfig::load();
        let apps = apps::load_apps();
        let categories = apps::load_categories(&apps);
        tracing::info!("Preloaded {} apps, {} categories", apps.len(), categories.len());

        // Load pinned apps from CosmicDock (Files + Settings pinned by default).
        let pinned_apps = load_pinned_apps_with_defaults();

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
            sidebar_collapsed: config.sidebar_collapsed,
            config,
            custom_width_input: String::new(),
            custom_height_input: String::new(),
            selected_index: None,
            nav_model: segmented_button::SingleSelectModel::default(),
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
        for id in &pinned_apps {
            applet.cached_pinned_ids.borrow_mut().insert(id.clone());
        }
        for id in &applet.config.favourites {
            applet.cached_fav_ids.borrow_mut().insert(id.clone());
        }
        applet.rebuild_nav_model();
        (applet, Task::none())
    }

    fn view(&self) -> Element<'_, Message> {
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
        self.build_menu_view(true)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
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
                self.rebuild_nav_model();

                // Apply default category from config
                let default_key = &self.config.default_category;
                let default_cat = match default_key.as_str() {
                    "favourites" => {
                        let mut fav_apps = apps::filter_by_ids(&self.all_applications, &self.config.favourites);
                        fav_apps.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                        self.available_applications = fav_apps;
                        Some(ApplicationCategory::favourites())
                    }
                    "recents" => {
                        let rec_apps = apps::filter_by_ids(&self.all_applications, &self.config.recents);
                        self.available_applications = rec_apps;
                        Some(ApplicationCategory::recents())
                    }
                    _ => {
                        Some(ApplicationCategory::all())
                    }
                };
                self.selected_category = default_cat;
                // Activate the matching nav entity
                if let Some(ref cat) = self.selected_category {
                    let entity = self.nav_model.iter().find(|&entity| {
                        self.nav_model.data::<ApplicationCategory>(entity)
                            .map_or(false, |c| c.key == cat.key)
                    });
                    if let Some(entity) = entity {
                        self.nav_model.activate(entity);
                    }
                }

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
                            state.core.main_window_id().unwrap(), new_id,
                            Some((popup_width, popup_height)),
                            None, None,
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
                if self.popup == Some(id) { self.popup = None; }
                Task::none()
            }
            Message::ClosePopup => {
                if let Some(p) = self.popup.take() { return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p)))); }
                Task::none()
            }
            Message::LaunchWindow => {
                let _ = std::process::Command::new("cosmic-kde-launcher").arg("--window").spawn();
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
                    self.nav_model.data::<ApplicationCategory>(entity)
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
                    self.nav_model.data::<ApplicationCategory>(entity)
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
                        self.nav_model.data::<ApplicationCategory>(entity)
                            .map_or(false, |cat| cat.key == "all")
                    });
                    if let Some(entity) = all_entity {
                        self.nav_model.activate(entity);
                    }
                    let focus_id = (*SEARCH_ID).clone();
                    let focus = cosmic::widget::text_input::focus(focus_id);
                    let reset = self.reset_grid_scroll();
                    return Task::batch([focus, reset]);
                } else {
                    // Closing search — restore "All Applications"
                    self.search_field.clear();
                    self.selected_index = None;
                    self.available_applications = self.all_applications.clone();
                    self.selected_category = Some(ApplicationCategory::all());
                    let all_entity = self.nav_model.iter().find(|&entity| {
                        self.nav_model.data::<ApplicationCategory>(entity)
                            .map_or(false, |cat| cat.key == "all")
                    });
                    if let Some(entity) = all_entity {
                        self.nav_model.activate(entity);
                    }
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
                    let cat = cat.clone();
                    self.selected_category = Some(cat.clone());
                    self.available_applications = match cat.key.as_str() {
                        "favourites" => {
                            let mut favs = apps::filter_by_ids(&self.all_applications, &self.config.favourites);
                            favs.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                            favs
                        }
                        "recents" => apps::filter_by_ids(&self.all_applications, &self.config.recents),
                        _ => apps::filter_apps_by_category(&self.all_applications, &cat),
                    };
                }
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
            Message::WindowResized(size) => {
                if self.is_window_mode && size.width >= 400.0 && size.height >= 300.0
                    && (self.config.size_preset != config::SizePreset::Custom
                        || (size.width - self.config.custom_width).abs() > 1.0
                        || (size.height - self.config.custom_height).abs() > 1.0)
                {
                    self.config.size_preset = config::SizePreset::Custom;
                    self.config.custom_width = size.width;
                    self.config.custom_height = size.height;
                    self.custom_width_input = format!("{}", size.width as u32);
                    self.custom_height_input = format!("{}", size.height as u32);
                    self.config.save();
                }
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
                if self.available_applications.is_empty() { return Task::none(); }
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
            Message::ToggleLayout(mode) => {
                self.config.layout_mode = mode;
                self.config.save();
                self.show_settings = false;
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
                if preset != config::SizePreset::Custom {
                    self.config.custom_width = 0.0;
                    self.config.custom_height = 0.0;
                    self.custom_width_input.clear();
                    self.custom_height_input.clear();
                }
                self.config.save();
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
                if let Ok(w) = self.custom_width_input.parse::<f32>() {
                    if w >= 400.0 {
                        self.config.custom_width = w;
                        self.config.size_preset = config::SizePreset::Custom;
                    }
                }
                if let Ok(h) = self.custom_height_input.parse::<f32>() {
                    if h >= 300.0 {
                        self.config.custom_height = h;
                        self.config.size_preset = config::SizePreset::Custom;
                    }
                }
                self.config.save();
                Task::none()
            }
            Message::SetPanelIcon(icon) => {
                self.config.panel_icon = icon;
                self.config.save();
                Task::none()
            }
            Message::TogglePanelIconSymbolic => {
                self.config.panel_icon_symbolic = !self.config.panel_icon_symbolic;
                self.config.save();
                Task::none()
            }
            Message::ToggleShowFavourites => {
                self.config.show_favourites = !self.config.show_favourites;
                self.config.save();
                self.rebuild_nav_model();
                Task::none()
            }
            Message::ToggleShowRecents => {
                self.config.show_recents = !self.config.show_recents;
                self.config.save();
                self.rebuild_nav_model();
                Task::none()
            }
            Message::ToggleShowBottomBarPinned => {
                self.config.show_bottom_bar_pinned = !self.config.show_bottom_bar_pinned;
                self.config.save();
                Task::none()
            }
            Message::ToggleShowBottomBarPowerActions => {
                self.config.show_bottom_bar_power_actions =
                    !self.config.show_bottom_bar_power_actions;
                self.config.save();
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
                self.config.save();
                Task::none()
            }
            Message::SetDefaultCategory(cat) => {
                self.config.default_category = cat;
                self.config.save();
                Task::none()
            }
            Message::ToggleFavourite(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    let was_empty = self.config.favourites.is_empty();
                    let now_fav = self.config.toggle_favourite(&app_id);
                    self.config.save();
                    if now_fav {
                        self.cached_fav_ids.borrow_mut().insert(app_id.clone());
                    } else {
                        self.cached_fav_ids.borrow_mut().remove(&app_id);
                    }
                    if was_empty && now_fav && self.config.default_category == "all" {
                        self.config.default_category = "favourites".into();
                        self.config.save();
                    }
                    // Rebuild nav to show/hide Favourites entry
                    self.rebuild_nav_model();
                    // If viewing favourites and unfavourited, refresh list
                    if let Some(ref cat) = self.selected_category {
                        if cat.key == "favourites" {
                            let mut favs = apps::filter_by_ids(&self.all_applications, &self.config.favourites);
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
                match action {
                    PowerAction::Lock => {
                        let _ = std::process::Command::new("loginctl")
                            .arg("lock-session")
                            .spawn();
                    }
                    PowerAction::Logout => {
                        if std::process::Command::new("cosmic-osd")
                            .arg("logout")
                            .spawn()
                            .is_err()
                        {
                            let _ = std::process::Command::new("loginctl")
                                .arg("terminate-user")
                                .arg(std::env::var("USER").unwrap_or_default())
                                .spawn();
                        }
                    }
                    PowerAction::Suspend => {
                        let _ = std::process::Command::new("systemctl")
                            .arg("suspend")
                            .spawn();
                    }
                    PowerAction::Restart => {
                        if std::process::Command::new("cosmic-osd")
                            .arg("restart")
                            .spawn()
                            .is_err()
                        {
                            let _ = std::process::Command::new("systemctl")
                                .arg("reboot")
                                .spawn();
                        }
                    }
                    PowerAction::Shutdown => {
                        if std::process::Command::new("cosmic-osd")
                            .arg("shutdown")
                            .spawn()
                            .is_err()
                        {
                            let _ = std::process::Command::new("systemctl")
                                .arg("poweroff")
                                .spawn();
                        }
                    }
                }
                if let Some(p) = self.popup.take() {
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
                }
                Task::none()
            }
            Message::RefreshApps => {
                let current = apps::list_desktop_ids();
                if current == self.known_desktop_ids {
                    return Task::none();
                }
                // Desktop files changed — reload in background, then update state.
                return Task::future(async move {
                    let apps = tokio::task::spawn_blocking(|| {
                        let a = apps::load_apps();
                        let c = apps::load_categories(&a);
                        (a, c)
                    })
                    .await
                    .unwrap_or_default();
                    cosmic::action::app(Message::AppsRefreshed(apps.0, apps.1))
                });
            }
            Message::AppsRefreshed(apps, categories) => {
                self.all_applications = apps;
                self.available_categories = categories;
                self.known_desktop_ids = apps::list_desktop_ids();
                self.rebuild_nav_model();
                // If popup is open, re-apply current view filters.
                if self.popup.is_some() {
                    if self.search_active {
                        let input = self.search_field.clone();
                        self.available_applications = if input.is_empty() {
                            self.all_applications.clone()
                        } else {
                            apps::filter_apps(&self.all_applications, &input)
                        };
                    } else if let Some(ref cat) = self.selected_category {
                        self.available_applications = match cat.key.as_str() {
                            "favourites" => {
                                let mut favs = apps::filter_by_ids(
                                    &self.all_applications,
                                    &self.config.favourites,
                                );
                                favs.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
                                favs
                            }
                            "recents" => apps::filter_by_ids(
                                &self.all_applications,
                                &self.config.recents,
                            ),
                            _ => apps::filter_apps_by_category(&self.all_applications, cat),
                        };
                    } else {
                        self.available_applications = self.all_applications.clone();
                    }
                    // Deselect if the index is now out of bounds.
                    if let Some(idx) = self.selected_index {
                        if idx >= self.available_applications.len() {
                            self.selected_index = None;
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        use cosmic::iced::event;
        use cosmic::iced::keyboard;
        use cosmic::iced::window;
        use std::time::Duration;

        let mut subs = Vec::new();

        // Periodic app-list refresh — detects newly installed/removed apps.
        subs.push(
            cosmic::iced::time::every(Duration::from_secs(10))
                .map(|_| Message::RefreshApps),
        );

        // Track window size changes for responsive layout in windowed mode.
        subs.push(event::listen_with(|event, _status, _window_id| match event {
            cosmic::iced::Event::Window(window::Event::Resized(size)) => {
                Some(Message::WindowResized(size))
            }
            _ => None,
        }));

        // Keyboard shortcuts — only when the popup is open.
        if self.popup.is_some() {
            subs.push(listen_raw(|event, _status, _id| match event {
                cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(key), ..
                }) => match key {
                    Named::Escape => Some(Message::ClosePopup),
                    Named::ArrowDown => Some(Message::SelectNext),
                    Named::ArrowUp => Some(Message::SelectPrevious),
                    Named::ArrowRight => Some(Message::SelectNext),
                    Named::ArrowLeft => Some(Message::SelectPrevious),
                    Named::Enter => Some(Message::LaunchSelected),
                    _ => None,
                },
                _ => None,
            }));
        }

        cosmic::iced::Subscription::batch(subs)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        if !self.is_window_mode { return Vec::new(); }
        let sidebar_toggle: Element<'_, Message> = nav_bar_toggle()
            .on_toggle(Message::ToggleSidebar)
            .active(!self.sidebar_collapsed)
            .into();
        let search_btn: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("system-search-symbolic").symbolic(true).size(18).icon(),
        )
        .on_press(Message::ToggleSearch)
        .class(if self.search_active { theme::Button::Suggested } else { theme::Button::HeaderBar })
        .into();
        vec![sidebar_toggle, search_btn]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        if !self.is_window_mode { return Vec::new(); }
        let config_btn: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("emblem-system-symbolic").symbolic(true).size(18).icon(),
        )
        .on_press(Message::ToggleSettings)
        .class(if self.show_settings { theme::Button::Suggested } else { theme::Button::HeaderBar })
        .into();
        vec![config_btn]
    }
}

impl Applet {

    fn build_menu_view(&self, is_popup: bool) -> Element<'_, Message> {
        let cosmic_theme = theme::active();
        let Spacing { space_xxs, space_xs, space_s, space_m, .. } = cosmic_theme.cosmic().spacing;
        let menu_width = self.config.max_width();
        let menu_height = self.config.max_height();
        let show_bottom_bar_power = self.config.show_bottom_bar_power_actions;
        let show_bottom_bar_pinned =
            self.config.show_bottom_bar_pinned && !self.pinned_apps.is_empty();
        let show_bottom_bar = show_bottom_bar_pinned || show_bottom_bar_power;
        // Bottom bar height: button content (~21px) + button padding (2×space_xxs)
        // + symmetric row padding (2×space_xxs), minimum 44px.
        let bottom_bar_height = if show_bottom_bar {
            (21.0 + space_xxs as f32 * 4.0).max(44.0)
        } else {
            0.0
        };
        tracing::debug!(
            "view_window: menu={}×{} bottom_bar_h={} space_m={} space_s={}",
            menu_width, menu_height, bottom_bar_height, space_m, space_s
        );

        // Calculate the exact height available for the app area so we don't
        // rely on nested Fill inside Shrink chains (which can collapse in the
        // popup_container's autosize wrapper).
        let is_window = !is_popup;
        let outer_pad = space_xs as f32 * 2.0;
        let has_title = self.sidebar_collapsed && self.selected_category.is_some();
        // In window mode the header bar provides the top buttons, so the
        // in-content top_bar is collapsed and contributes no spacing.
        let col_spacing_count: u32 = if is_window { 0 } else { 1 } // top_bar → next
            + if self.search_active { 1 } else { 0 }
            + if has_title { 1 } else { 0 }
            + 1 // → dual_pane
            + if show_bottom_bar { 1 } else { 0 }; // → bottom_bar
        let total_spacing = col_spacing_count as f32 * space_xxs as f32;
        let app_area_height = (menu_height - outer_pad - total_spacing
            - bottom_bar_height)
            .max(200.0);
        tracing::debug!(
            "view_window: outer_pad={} col_spacings={} total_spacing={} app_area_h={}",
            outer_pad, col_spacing_count, total_spacing, app_area_height
        );

        // In window mode the header bar (header_start/header_end) already
        // provides sidebar-toggle, search, and config buttons.  In popup mode
        // we render them as an in-content top bar.
        let top_bar: Element<'_, Message> = if is_window {
            Space::new().width(Length::Shrink).height(Length::Shrink).into()
        } else {
            let sidebar_toggle: Element<'_, Message> = nav_bar_toggle()
                .on_toggle(Message::ToggleSidebar)
                .active(!self.sidebar_collapsed)
                .into();

            let search_toggle_btn: Element<'_, Message> = cosmic::widget::button::custom(
                cosmic::widget::icon::from_name("system-search-symbolic")
                    .symbolic(true).size(18).icon(),
            )
            .on_press(Message::ToggleSearch)
            .class(if self.search_active { theme::Button::Suggested } else { theme::Button::AppletMenu })
            .into();

            let config_btn: Element<'_, Message> = cosmic::widget::button::custom(
                cosmic::widget::icon::from_name("emblem-system-symbolic")
                    .symbolic(true).size(18).icon(),
            )
            .on_press(Message::ToggleSettings)
            .class(if self.show_settings { theme::Button::Suggested } else { theme::Button::AppletMenu })
            .into();

            container(
                row![
                    sidebar_toggle,
                    search_toggle_btn,
                    Space::new().width(Length::Fill),
                    config_btn,
                ]
                .align_y(Alignment::Center)
                .spacing(space_s),
            )
            .width(Length::Fill)
            .into()
        };

        // ── Search bar (collapsible) ──
        let search_row: Option<Element<'_, Message>> = if self.search_active {
            let search = cosmic::widget::search_input("Type to search…", &self.search_field)
                .id((*SEARCH_ID).clone())
                .on_input(Message::SearchInput)
                .on_clear(Message::SearchCleared)
                .width(Length::Fill)
                .padding([space_xxs, space_s]);
            Some(search.into())
        } else {
            None
        };

        // ── Category sidebar (collapsible) ──
        let nav: Element<'_, Message> = if self.sidebar_collapsed {
            Space::new().width(Length::Shrink).height(Length::Shrink).into()
        } else {
            mouse_area(
                container(
                    nav_bar(&self.nav_model, Message::CategoryActivated)
                        .into_container()
                        .width(Length::Fixed(SIDEBAR_WIDTH))
                        .height(Length::Fill)
                        .padding([space_xxs, space_xxs, space_xxs, space_xxs]),
                )
                .width(Length::Fixed(SIDEBAR_WIDTH))
                .height(Length::Fill),
            )
            .on_enter(Message::ClearAppHover)
            .into()
        };

        // ── App area ──
        // Precompute favourite/pinned lookups once per view (O(n) instead of
        // O(apps × favourites) per cell).
        let fav_set = self.cached_fav_ids.borrow();
        let pinned_set = self.cached_pinned_ids.borrow();

        // In Hybrid mode, use grid for Favourites/Recents categories, list otherwise.
        let use_grid = self.config.layout_mode == config::LayoutMode::Grid
            || (self.config.layout_mode == config::LayoutMode::Hybrid
                && matches!(self.selected_category.as_ref(),
                    Some(cat) if cat.key == "favourites" || cat.key == "recents"));
        // Grid view uses the larger package-card icon size (matching cosmic-store).
        let effective_icon_size: f32 = GRID_ICON_SIZE as f32;

        let app_area: Element<'_, Message> = if self.available_applications.is_empty() {
            container(cosmic::widget::text::body("No applications found."))
                .center_x(Length::Fill).center_y(Length::Fill)
                .height(Length::Fill)
                .width(Length::Fill)
                .into()
        } else if use_grid {
            // Dynamic column count based on available width.
            let mut avail_width = menu_width - (space_xs as f32 * 2.0);
            if !self.sidebar_collapsed {
                avail_width -= SIDEBAR_WIDTH + space_m as f32;
            }
            if self.show_settings {
                avail_width -= 300.0;
            }
            // Each grid button needs at least 80px; compute columns.
            let min_btn = 80.0 + space_s as f32;
            let grid_columns = ((avail_width / min_btn) as usize).max(3).min(8);
            let grid_columns = self.config.grid_columns.min(grid_columns);

            // Fixed height for uniform grid cells. Allow 2-3 lines of caption
            // text so long app names wrap instead of being cut off.
            // Caption = 12px font / 17px line-height; budget 3 lines (51px).
            let cell_height = effective_icon_size + 44.0 + space_xxs as f32 * 2.0;
            let row_stride = cell_height + space_s as f32;
            let rows_total = self.available_applications.len().div_ceil(grid_columns);
            let content_h = rows_total as f32 * cell_height
                + (rows_total.saturating_sub(1)) as f32 * space_s as f32;

            // Build one grid row (absolute row index) — used only for visible rows.
            let build_row = |row_idx: usize| -> Element<'_, Message> {
                let start = row_idx * grid_columns;
                let end = ((row_idx + 1) * grid_columns).min(self.available_applications.len());
                let mut buttons: Vec<Element<'_, Message>> = (start..end)
                    .map(|index| {
                        let app = &self.available_applications[index];
                        let icon = self.cached_icon(app, effective_icon_size);
                        let name = truncate_name(&app.name, 32);
                        let is_selected = self.selected_index == Some(index);
                        let is_fav = fav_set.contains(app.id.as_str());
                        let is_pinned = pinned_set.contains(app.id.as_str());
                        let show_actions = self.hovered_app_index == Some(index);
                        let content: Element<'_, Message> = {
                            let inner = column![
                                icon,
                                cosmic::widget::text::caption(name)
                                    .wrapping(cosmic::iced::widget::text::Wrapping::Word),
                            ]
                            .align_x(Alignment::Center)
                            .spacing(space_xxs);
                            let inner = container(inner)
                                .center_x(Length::Fill)
                                .align_y(Alignment::Start)
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .padding([space_xxs, 0, 0, 0]);
                            if let Some(corners) = app_corner_overlay(
                                index,
                                is_fav,
                                is_pinned,
                                space_xxs,
                                show_actions,
                            ) {
                                stack![inner, corners]
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                                    .into()
                            } else {
                                inner.into()
                            }
                        };
                        let btn = cosmic::widget::button::custom(content)
                            .on_press(Message::LaunchApp(index))
                            .class(if is_selected { theme::Button::Suggested } else { theme::Button::AppletMenu })
                            .width(Length::Fill)
                            .height(Length::Fixed(cell_height));

                        mouse_area(btn)
                            .on_enter(Message::AppHovered(index))
                            .on_exit(Message::AppUnhovered(index))
                            .into()
                    })
                    .collect();

                let missing = grid_columns - buttons.len();
                for _ in 0..missing {
                    buttons.push(Space::new().width(Length::Fill).into());
                }
                row(buttons).spacing(space_s).width(Length::Fill).align_y(Alignment::Start).into()
            };

            // Virtualize: only render rows intersecting the viewport (plus one
            // row of overscan on each side), keeping total height identical so
            // the scrollbar and layout are unchanged.
            let viewport_h = if self.grid_viewport_h > 0.0 { self.grid_viewport_h } else { menu_height };
            let scroll = self.grid_scroll_y.clamp(0.0, (content_h - viewport_h).max(0.0));
            // First row whose bottom edge is below the top of the viewport.
            let mut first_row = if scroll <= cell_height {
                0
            } else {
                ((scroll - cell_height) / row_stride).floor() as usize + 1
            };
            first_row = first_row.saturating_sub(1); // overscan above
            // Last row whose top edge is above the bottom of the viewport.
            let mut last_row = ((scroll + viewport_h) / row_stride).ceil() as usize;
            last_row = last_row
                .saturating_sub(1)
                .min(rows_total.saturating_sub(1));
            last_row = (last_row + 1).min(rows_total.saturating_sub(1)); // overscan below

            let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(last_row - first_row + 3);
            if first_row > 0 {
                let top_h = (first_row as f32 * row_stride - space_s as f32).max(0.0);
                rows.push(Space::new().height(Length::Fixed(top_h)).into());
            }
            for row_idx in first_row..=last_row {
                rows.push(build_row(row_idx));
            }
            let bottom_h = content_h - (last_row as f32 * row_stride + cell_height) - space_s as f32;
            if bottom_h > 0.0 {
                rows.push(Space::new().height(Length::Fixed(bottom_h)).into());
            }

            let app_grid = column(rows).spacing(space_s).width(Length::Fill);
            container(
                scrollable(app_grid)
                    .id((*SCROLLABLE_ID).clone())
                    .height(Length::Fill)
                    .on_scroll(|vp| {
                        Message::GridScrolled(vp.absolute_offset().y, vp.bounds().height)
                    }),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            // Virtualized list view — only renders rows intersecting the
            // viewport, matching the grid view's approach for performance.
            let mut list_width = menu_width as usize;
            list_width = list_width.saturating_sub(space_xs as usize * 2);
            if !self.sidebar_collapsed {
                list_width = list_width.saturating_sub(SIDEBAR_WIDTH as usize + space_m as usize);
            }
            if self.show_settings {
                list_width = list_width.saturating_sub(280);
            }

            let GridMetrics {
                cols,
                item_width,
                column_spacing,
            } = list_grid_metrics(space_xxs, space_s, list_width);

            let apps = &self.available_applications;
            let total_rows = if apps.is_empty() || cols == 0 { 0 } else { (apps.len() + cols - 1) / cols };
            let card_height = LIST_ICON_SIZE as f32 + (space_xxs as f32) * 2.0;
            let row_stride = card_height + column_spacing as f32;
            let content_h = if total_rows == 0 { 0.0 } else { total_rows as f32 * row_stride - column_spacing as f32 };

            // Virtualize: only render rows near the viewport.
            let viewport_h = if self.grid_viewport_h > 0.0 { self.grid_viewport_h } else { menu_height };
            let scroll = self.grid_scroll_y.clamp(0.0, (content_h - viewport_h).max(0.0));
            let first_row = if scroll <= row_stride { 0usize } else { ((scroll / row_stride) as usize).saturating_sub(1) };
            let last_row = ((scroll + viewport_h) / row_stride).ceil() as usize;
            let last_row = last_row.min(total_rows.saturating_sub(1)).saturating_add(1).min(total_rows.saturating_sub(1));

            // Pre-compute text column width once per view (same for all cards).
            let text_width = item_width.saturating_sub(
                LIST_ICON_SIZE as usize + space_s as usize * 3
            ) as f32;

            let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(last_row.saturating_sub(first_row) + 3);
            if first_row > 0 {
                let top_h = (first_row as f32 * row_stride - column_spacing as f32).max(0.0);
                rows.push(Space::new().height(Length::Fixed(top_h)).into());
            }
            for row_idx in first_row..=last_row {
                let start = row_idx * cols;
                let end = (start + cols).min(apps.len());
                let mut row_children: Vec<Element<'_, Message>> = Vec::with_capacity(cols);
                for i in start..end {
                    let app = &apps[i];
                    let is_fav = fav_set.contains(app.id.as_str());
                    let is_pinned = pinned_set.contains(app.id.as_str());
                    let is_selected = self.selected_index == Some(i);
                    let show_actions = self.hovered_app_index == Some(i);
                    let icon = self.cached_icon(app, LIST_ICON_SIZE as f32);
                    row_children.push(app_list_card(
                        app,
                        icon,
                        space_xxs,
                        space_s,
                        text_width,
                        item_width,
                        i,
                        is_fav,
                        is_pinned,
                        is_selected,
                        show_actions,
                    ));
                }
                let missing = cols.saturating_sub(row_children.len());
                for _ in 0..missing {
                    row_children.push(Space::new().width(Length::Fixed(item_width as f32)).into());
                }
                rows.push(row(row_children).spacing(column_spacing).width(Length::Fill).into());
            }
            if last_row < total_rows.saturating_sub(1) {
                let bottom_h = content_h - (last_row as f32 * row_stride + card_height) - column_spacing as f32;
                if bottom_h > 0.0 {
                    rows.push(Space::new().height(Length::Fixed(bottom_h)).into());
                }
            }

            container(
                scrollable(column(rows).spacing(column_spacing).width(Length::Fill))
                    .id((*SCROLLABLE_ID).clone())
                    .height(Length::Fill)
                    .on_scroll(|vp| Message::GridScrolled(vp.absolute_offset().y, vp.bounds().height)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        };

        let app_area: Element<'_, Message> = mouse_area(
            container(app_area)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_exit(Message::ClearAppHover)
        .into();

        // ── Settings panel (right side) ──
        let settings_panel: Option<Element<'_, Message>> = if self.show_settings {
            let settings_content = {
                // Grid view button
                let check: Element<'_, Message> = if self.config.layout_mode == config::LayoutMode::Grid {
                    cosmic::widget::icon::from_name("object-select-symbolic")
                        .symbolic(true).size(16).icon().into()
                } else {
                    Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                };
                let grid_btn: Element<'_, Message> = cosmic::widget::button::custom(
                    row![
                        cosmic::widget::icon::from_name("view-grid-symbolic")
                            .symbolic(true).size(16).icon(),
                        cosmic::widget::text::body("Grid View"),
                        Space::new().width(Length::Fill),
                        check,
                    ].align_y(Alignment::Center).spacing(space_s),
                )
                .on_press(if self.config.layout_mode == config::LayoutMode::Grid { Message::ToggleSettings } else { Message::ToggleLayout(config::LayoutMode::Grid) })
                .class(if self.config.layout_mode == config::LayoutMode::Grid { theme::Button::Suggested } else { theme::Button::AppletMenu })
                .width(Length::Fill)
                .into();

                // List view button
                let check: Element<'_, Message> = if self.config.layout_mode == config::LayoutMode::List {
                    cosmic::widget::icon::from_name("object-select-symbolic")
                        .symbolic(true).size(16).icon().into()
                } else {
                    Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                };
                let list_btn: Element<'_, Message> = cosmic::widget::button::custom(
                    row![
                        cosmic::widget::icon::from_name("view-list-symbolic")
                            .symbolic(true).size(16).icon(),
                        cosmic::widget::text::body("List View"),
                        Space::new().width(Length::Fill),
                        check,
                    ].align_y(Alignment::Center).spacing(space_s),
                )
                .on_press(if self.config.layout_mode == config::LayoutMode::List { Message::ToggleSettings } else { Message::ToggleLayout(config::LayoutMode::List) })
                .class(if self.config.layout_mode == config::LayoutMode::List { theme::Button::Suggested } else { theme::Button::AppletMenu })
                .width(Length::Fill)
                .into();

                // Hybrid view button (Favs+Recents grid, rest list)
                let check: Element<'_, Message> = if self.config.layout_mode == config::LayoutMode::Hybrid {
                    cosmic::widget::icon::from_name("object-select-symbolic")
                        .symbolic(true).size(16).icon().into()
                } else {
                    Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                };
                let hybrid_btn: Element<'_, Message> = cosmic::widget::button::custom(
                    row![
                        cosmic::widget::icon::from_name("view-grid-symbolic")
                            .symbolic(true).size(16).icon(),
                        cosmic::widget::text::body("Hybrid View"),
                        Space::new().width(Length::Fill),
                        check,
                    ].align_y(Alignment::Center).spacing(space_s),
                )
                .on_press(if self.config.layout_mode == config::LayoutMode::Hybrid { Message::ToggleSettings } else { Message::ToggleLayout(config::LayoutMode::Hybrid) })
                .class(if self.config.layout_mode == config::LayoutMode::Hybrid { theme::Button::Suggested } else { theme::Button::AppletMenu })
                .width(Length::Fill)
                .into();

                // ── Size preset buttons ──
                let size_btns: Vec<Element<'_, Message>> = config::SizePreset::ALL.iter().map(|&preset| {
                    let is_active = self.config.size_preset == preset && self.config.custom_width == 0.0;
                    let check: Element<'_, Message> = if is_active {
                        cosmic::widget::icon::from_name("object-select-symbolic")
                            .symbolic(true).size(16).icon().into()
                    } else {
                        Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                    };
                    let label = format!("{} ({}×{})", preset.label(), preset.width() as u32, preset.height() as u32);
                    cosmic::widget::button::custom(
                        row![
                            cosmic::widget::text::body(label),
                            Space::new().width(Length::Fill),
                            check,
                        ].align_y(Alignment::Center).spacing(space_s),
                    )
                    .on_press(Message::SetSizePreset(preset))
                    .class(if is_active { theme::Button::Suggested } else { theme::Button::AppletMenu })
                    .width(Length::Fill)
                    .into()
                }).collect();

                // ── Panel icon dropdown ──
                let icon_options: &[(&str, &str)] = &[
                    ("com.system76.CosmicAppLibrary", "COSMIC"),
                    ("application-menu-symbolic", "App Menu"),
                    ("open-menu-symbolic", "Menu"),
                    ("start-here-symbolic", "Start"),
                    ("distributor-logo", "Linux"),
                    ("applications-all-symbolic", "All Apps"),
                    ("kde", "KDE"), ("plasma", "Plasma"), ("kmenu", "K Menu"),
                    ("gnome-main-menu", "GNOME"),
                    ("distributor-logo-pop-os", "Pop!_OS"),
                    ("start-here-ubuntu", "Ubuntu"), ("start-here-kubuntu", "Kubuntu"),
                    ("start-here-lubuntu", "Lubuntu"), ("start-here-xubuntu", "Xubuntu"),
                    ("start-here-ubuntu-mate", "Ubuntu MATE"), ("start-here-ubuntu-gnome", "Ubuntu GNOME"),
                    ("start-here-fedora", "Fedora"), ("distributor-logo-debian", "Debian"),
                    ("distributor-logo-archlinux", "Arch"), ("distributor-logo-manjaro", "Manjaro"),
                    ("distributor-logo-opensuse", "openSUSE"), ("distributor-logo-solus", "Solus"),
                    ("distributor-logo-elementary", "elementary"), ("distributor-logo-linux-mint", "Mint"),
                    ("distributor-logo-slackware", "Slackware"), ("distributor-logo-mageia", "Mageia"),
                    ("applications-system-symbolic", "System"), ("applications-engineering-symbolic", "Dev"),
                    ("applications-games-symbolic", "Games"), ("applications-graphics-symbolic", "Graphics"),
                    ("applications-multimedia-symbolic", "Media"), ("applications-office-symbolic", "Office"),
                    ("applications-science-symbolic", "Science"), ("applications-utilities-symbolic", "Utils"),
                    ("computer-symbolic", "Computer"), ("system-run-symbolic", "Run"),
                    ("system-search-symbolic", "Search"), ("preferences-system-symbolic", "Settings"),
                    ("emblem-system-symbolic", "System"),
                ];
                let current_icon = if self.config.panel_icon.is_empty() {
                    "com.system76.CosmicAppLibrary"
                } else {
                    &self.config.panel_icon
                };
                let labels: Vec<String> = icon_options.iter().map(|(_, l)| l.to_string()).collect();
                let icons: Vec<cosmic::widget::icon::Handle> = icon_options.iter().map(|(name, _)| {
                    cosmic::widget::icon::from_name(*name).symbolic(false).prefer_svg(true).size(24).handle()
                }).collect();
                let selected_idx = icon_options.iter().position(|(name, _)| *name == current_icon);
                let picker = dropdown(
                    labels,
                    selected_idx,
                    move |idx: usize| {
                        let icon_name = icon_options[idx].0.to_string();
                        Message::SetPanelIcon(icon_name)
                    },
                )
                .icons(icons.into())
                .width(Length::Fill)
                .padding([space_xxs, space_s]);


                column![
                    cosmic::widget::text::heading("Appearance"),
                    Space::new().height(space_m),
                    grid_btn,
                    list_btn,
                    hybrid_btn,
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Menu Size"),
                    Space::new().height(space_xxs),
                    column(size_btns).spacing(space_xxs),
                    // Custom size inputs (shown when Custom preset is active)
                    if self.config.size_preset == config::SizePreset::Custom {
                        let width_input = cosmic::widget::text_input("Width (px)…", &self.custom_width_input)
                            .on_input(Message::SetCustomWidth)
                            .width(Length::Fixed(100.0))
                            .padding([space_xxs, space_xxs]);
                        let height_input = cosmic::widget::text_input("Height (px)…", &self.custom_height_input)
                            .on_input(Message::SetCustomHeight)
                            .width(Length::Fixed(100.0))
                            .padding([space_xxs, space_xxs]);
                        let apply_btn: Element<'_, Message> = cosmic::widget::button::standard("Apply")
                            .on_press(Message::ApplyCustomSize)
                            .into();
                        let row: Element<'_, Message> = row![
                            cosmic::widget::text::body("W:"),
                            width_input,
                            Space::new().width(Length::Fixed(space_xxs as f32)),
                            cosmic::widget::text::body("H:"),
                            height_input,
                            Space::new().width(Length::Fixed(space_s as f32)),
                            apply_btn,
                        ].align_y(Alignment::Center).spacing(space_xxs).into();
                        Some(row)
                    } else {
                        None
                    },
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Panel Icon"),
                    Space::new().height(space_xxs),
                    picker,
                    Space::new().height(space_xxs),
                    // Symbolic / coloured toggle
                    {
                        let check: Element<'_, Message> = if self.config.panel_icon_symbolic {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Monochrome icon"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::TogglePanelIconSymbolic)
                        .class(if self.config.panel_icon_symbolic { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Sidebar"),
                    Space::new().height(space_xxs),
                    // Show Favourites toggle
                    {
                        let check: Element<'_, Message> = if self.config.show_favourites {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Show Favourites"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::ToggleShowFavourites)
                        .class(if self.config.show_favourites { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    // Show Recents toggle
                    {
                        let check: Element<'_, Message> = if self.config.show_recents {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Show Recents"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::ToggleShowRecents)
                        .class(if self.config.show_recents { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    // Hide sidebar by default toggle
                    {
                        let check: Element<'_, Message> = if self.config.sidebar_collapsed {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Hide Sidebar by Default"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::ToggleSidebarDefault)
                        .class(if self.config.sidebar_collapsed { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Bottom Bar"),
                    Space::new().height(space_xxs),
                    {
                        let check: Element<'_, Message> = if self.config.show_bottom_bar_pinned {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Show Pinned Apps"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::ToggleShowBottomBarPinned)
                        .class(if self.config.show_bottom_bar_pinned { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    {
                        let check: Element<'_, Message> = if self.config.show_bottom_bar_power_actions {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Show Power Actions"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::ToggleShowBottomBarPowerActions)
                        .class(if self.config.show_bottom_bar_power_actions { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Default Menu"),
                    Space::new().height(space_xxs),
                    // Default category: All Applications
                    {
                        let check: Element<'_, Message> = if self.config.default_category == "all" {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("All Applications"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::SetDefaultCategory("all".into()))
                        .class(if self.config.default_category == "all" { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    // Default category: Favourites
                    {
                        let check: Element<'_, Message> = if self.config.default_category == "favourites" {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Favourites"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::SetDefaultCategory("favourites".into()))
                        .class(if self.config.default_category == "favourites" { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                    // Default category: Recents
                    {
                        let check: Element<'_, Message> = if self.config.default_category == "recents" {
                            cosmic::widget::icon::from_name("object-select-symbolic")
                                .symbolic(true).size(16).icon().into()
                        } else {
                            Space::new().width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).into()
                        };
                        let btn: Element<'_, Message> = cosmic::widget::button::custom(
                            row![
                                cosmic::widget::text::body("Recents"),
                                Space::new().width(Length::Fill),
                                check,
                            ].align_y(Alignment::Center).spacing(space_s),
                        )
                        .on_press(Message::SetDefaultCategory("recents".into()))
                        .class(if self.config.default_category == "recents" { theme::Button::Suggested } else { theme::Button::AppletMenu })
                        .width(Length::Fill)
                        .into();
                        btn
                    },
                ].spacing(space_xxs).padding(space_s)
            };

            Some(
                container(
                    scrollable(settings_content)
                        .height(Length::Fill),
                )
                .width(Length::Fixed(280.0))
                .height(Length::Fill)
                .class(theme::Container::Primary)
                .into()
            )
        } else {
            None
        };

        // ── Bottom bar ──
        let menu_too_small = menu_height < 600.0 || menu_width <= 600.0;
        let bottom_bar: Element<'_, Message> = if show_bottom_bar {
            let mut bottom_row = row![]
                .spacing(space_s)
                .align_y(Alignment::Center)
                .width(Length::Fill);

            if show_bottom_bar_pinned {
                for pinned_id in &self.pinned_apps {
                    let app = self.all_applications.iter().find(|a| a.id == *pinned_id);
                    let label: Cow<'static, str> = if let Some(app) = app {
                        Cow::Owned(truncate_name(&app.name, 24).into_owned())
                    } else {
                        Cow::Owned(
                            pinned_id
                                .strip_suffix(".desktop")
                                .unwrap_or(pinned_id.as_str())
                                .to_string(),
                        )
                    };
                    let hovered = self.hovered_pinned_id.as_deref() == Some(pinned_id.as_str());
                    bottom_row = bottom_row.push(bottom_bar_pinned_item(
                        &self.all_applications,
                        pinned_id,
                        label,
                        hovered,
                        menu_too_small,
                        space_xxs,
                        space_xs,
                    ));
                }
                if show_bottom_bar_power {
                    bottom_row = bottom_row.push(
                        container(
                            cosmic::widget::divider::vertical::default()
                                .height(Length::Fill),
                        )
                        .height(Length::Fixed(bottom_bar_height - space_xxs as f32 * 2.0))
                        .padding([0, space_xxs]),
                    );
                }
            }

            if show_bottom_bar_power {
                for &action in PowerAction::BOTTOM_BAR.iter() {
                    bottom_row = bottom_row.push(bottom_bar_action_button(
                        cosmic::widget::icon::from_name(action.icon_name())
                            .symbolic(true).size(20).icon(),
                        Cow::Borrowed(action.label()),
                        Message::PowerAction(action),
                        false,
                        menu_too_small,
                        space_xxs,
                        space_xs,
                    ));
                }
            }

            container(
                bottom_row.padding([space_xxs, space_xxs, space_xxs, space_xxs]),
            )
            .height(Length::Fixed(bottom_bar_height))
            .width(Length::Fill)
            .class(theme::Container::Primary)
            .into()
        } else {
            Space::new().width(Length::Shrink).height(Length::Shrink).into()
        };

        // ── Main layout ──
        let mut dual_pane = row![]
            .spacing(space_xxs)
            .width(Length::Fill)
            .height(Length::Fill);

        if !self.sidebar_collapsed {
            dual_pane = dual_pane.push(nav);
        }
        dual_pane = dual_pane.push(app_area);

        if let Some(sp) = settings_panel {
            dual_pane = dual_pane.push(sp);
        }

        // ── Category title (shown when sidebar is hidden) ──
        let category_title: Option<Element<'_, Message>> = if self.sidebar_collapsed {
            self.selected_category.as_ref().map(|cat| {
                let icon = cosmic::widget::icon::from_name(
                    std::sync::Arc::from(cat.icon_name.as_str())
                )
                .symbolic(true).size(20).icon();
                container(
                    row![
                        icon,
                        cosmic::widget::text::heading(&cat.display_name),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(space_s),
                )
                .width(Length::Fill)
                .padding([space_xxs, 0])
                .into()
            })
        } else {
            None
        };

        // Build the section above the bottom bar: top_bar + optional search +
        // optional category title + the dual_pane app grid.
        let mut main_col = column![top_bar]
            .spacing(space_xxs)
            .padding([space_xxs, space_xxs, space_xxs, space_xxs])
            .width(Length::Fill);

        if let Some(sr) = search_row {
            main_col = main_col.push(sr);
        }
        if let Some(ct) = category_title {
            main_col = main_col.push(ct);
        }
        let mut main_col = main_col.push(dual_pane);
        if show_bottom_bar {
            main_col = main_col.push(bottom_bar);
        }

        // Outer container: fixed size so the Fill column has a definite height
        // to distribute. The column's own padding provides visual spacing from
        // the popup edges.
        if is_popup {
            // Popup mode: fixed size inside a popup_container (frosted glass).
            let layout = container(main_col)
                .width(Length::Fixed(menu_width))
                .height(Length::Fixed(menu_height));

            self.core
                .applet
                .popup_container(layout)
                .limits(
                    Limits::NONE
                        .min_width(menu_width)
                        .max_width(menu_width)
                        .min_height(menu_height)
                        .max_height(menu_height),
                )
                .into()
        } else {
            // Window mode: fill the entire window, frosted-glass background
            // matching the panel / title-bar appearance.
            container(main_col)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([space_xs, space_xs, 0, space_xs])
                .style(|theme| {
                    let cosmic = theme.cosmic();
                    let bg = cosmic.background(true).base;
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Color::from(bg).into()),
                        text_color: Some(cosmic.background(true).on.into()),
                        icon_color: Some(cosmic.background(true).on.into()),
                        ..Default::default()
                    }
                })
                .into()
        }
    }


    /// Reset the app-grid scroll position and snap the scrollable back to the
    /// top, keeping the tracked offset in sync with the widget's internal one.
    /// The viewport height is also forgotten so the safe fallback (menu height)
    /// is used until the next scroll event re-measures it — this avoids a stale
    /// (smaller) viewport rendering a blank bottom after the layout changes.
    fn reset_grid_scroll(&mut self) -> Task<Message> {
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

    fn cached_icon(&self, app: &ApplicationEntry, size: f32) -> cosmic::widget::icon::Icon {
        let size_u16 = size as u16;
        let icon_name = app.icon.clone().unwrap_or_default();
        let key = (icon_name, size_u16);
        {
            let cache = self.icon_cache.borrow();
            if let Some(icon) = cache.get(&key) {
                return icon.clone();
            }
        }
        let icon = app_icon(app, size);
        self.icon_cache.borrow_mut().insert(key, icon.clone());
        icon
    }

    fn rebuild_nav_model(&mut self) {
        let active_key = self
            .selected_category
            .as_ref()
            .map(|c| c.key.as_str())
            .unwrap_or("all");

        self.nav_model.clear();

        // "All Applications" entry
        let all_icon = cosmic::widget::icon::from_name("user-home-symbolic")
            .symbolic(true).size(16).icon();
        self.nav_model.insert()
            .text("All Applications")
            .icon(all_icon)
            .data(ApplicationCategory::all());

        // "Favourites" entry (if any and enabled)
        if self.config.show_favourites && !self.config.favourites.is_empty() {
            let fav_icon = cosmic::widget::icon::from_name("starred-symbolic")
                .symbolic(true).size(16).icon();
            self.nav_model.insert()
                .text("Favourites")
                .icon(fav_icon)
                .data(ApplicationCategory::favourites());
        }

        // "Recents" entry (if any and enabled)
        if self.config.show_recents && !self.config.recents.is_empty() {
            let rec_icon = cosmic::widget::icon::from_name("document-open-recent-symbolic")
                .symbolic(true).size(16).icon();
            self.nav_model.insert()
                .text("Recents")
                .icon(rec_icon)
                .data(ApplicationCategory::recents());
        }

        // Category entries (with divider above the first one)
        for (i, cat) in self.available_categories.iter().enumerate() {
            let cat_icon = cosmic::widget::icon::from_name(
                std::sync::Arc::from(cat.icon_name.as_str())
            )
            .symbolic(true).size(16).icon();
            let mut entry = self.nav_model.insert()
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

    /// Pin a desktop file to the COSMIC dock/tray. Returns the updated pinned list.
    fn pin_app_to_dock(&self, desktop_id: &str) -> Vec<String> {
        let Ok(config) = cosmic::cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
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

    /// Unpin a desktop file from the COSMIC dock/tray. Returns the updated pinned list.
    fn unpin_app_from_dock(&self, desktop_id: &str) -> Vec<String> {
        let Ok(config) = cosmic::cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
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

    fn launch_application(&mut self, app: Arc<ApplicationEntry>) -> Task<Message> {
        // Track in recents
        self.config.add_recent(&app.id);
        self.config.save();
        // Rebuild nav to show/hide Recents entry
        self.rebuild_nav_model();

        if let Some(exec) = &app.exec {
            // Expand desktop-entry Exec field codes per the spec:
            //   %% → literal %
            //   %f/%F/%u/%U → removed (no files/URLs passed by a launcher)
            //   %i → --icon <icon>
            //   %c → translated name (use Name)
            //   %k → desktop file path
            //   %d/%D/%n/%N/%v/%m → deprecated, removed
            let expanded = expand_exec_fields(exec, &app);
            if let Some(argv) = shlex::split(&expanded) {
                if !argv.is_empty() {
                    let (program, args) = if app.is_terminal {
                        // Prepend cosmic-term wrapper
                        let mut full_args = vec!["cosmic-term".to_string(), "--".to_string()];
                        full_args.extend(argv.clone());
                        ("cosmic-term".to_string(), full_args)
                    } else {
                        (argv[0].clone(), argv)
                    };
                    let mut cmd = std::process::Command::new(&program);
                    if args.len() > 1 {
                        cmd.args(&args[1..]);
                    }
                    // Detach: don't block the applet, ignore exit status.
                    // Errors (e.g. binary not found) are logged and swallowed
                    // because there is no meaningful recovery in a launcher.
                    if let Err(e) = cmd.spawn() {
                        tracing::warn!("Failed to launch '{}': {}", program, e);
                    }
                }
            }
        }
        if let Some(p) = self.popup.take() {
            return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
        }
        Task::none()
    }
}

/// Expand desktop-entry Exec field codes per the Freedesktop spec.
///
/// Field codes handled:
///   `%%`  → literal `%`
///   `%f`  → removed (single file — launcher passes none)
///   `%F`  → removed (file list — launcher passes none)
///   `%u`  → removed (single URL — launcher passes none)
///   `%U`  → removed (URL list — launcher passes none)
///   `%i`  → `--icon <icon_name>`
///   `%c`  → translated name (uses the Name field)
///   `%k`  → desktop file path
///   `%d`, `%D`, `%n`, `%N`, `%v`, `%m` → removed (deprecated)
fn expand_exec_fields(exec: &str, app: &ApplicationEntry) -> String {
    let mut result = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }
        // Peek at the next character after %
        match chars.next() {
            None => {
                // Trailing % — keep as-is (spec says undefined behaviour)
                result.push('%');
            }
            Some('%') => result.push('%'),
            Some('f') | Some('F') | Some('u') | Some('U') => {
                // File/URL placeholders — launcher passes nothing, remove
            }
            Some('i') => {
                if let Some(ref icon) = app.icon {
                    result.push_str("--icon ");
                    result.push_str(icon);
                    result.push(' ');
                }
            }
            Some('c') => {
                // Translated name — use the Name field as a reasonable default
                result.push_str(&shell_escape(&app.name));
            }
            Some('k') => {
                // Desktop file path
                if let Some(path_str) = app.path.to_str() {
                    result.push_str(&shell_escape(path_str));
                }
            }
            Some('d') | Some('D') | Some('n') | Some('N')
            | Some('v') | Some('m') => {
                // Deprecated codes — remove
            }
            Some(other) => {
                // Unknown code — keep as-is per spec
                result.push('%');
                result.push(other);
            }
        }
    }

    result.trim().to_string()
}

/// Minimal shell-escaping for a single argument value (used by %c and %k).
/// Puts the value in single quotes, escaping any embedded single quotes.
fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    // Only escape if needed
    if value.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/') {
        return value.to_string();
    }
    let escaped = value.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

// ── List view helpers (Cosmic Store style) ──

struct GridMetrics {
    cols: usize,
    item_width: usize,
    column_spacing: u16,
}

impl GridMetrics {
    fn new(width: usize, min_width: usize, column_spacing: u16) -> Self {
        let width_m1 = width.saturating_sub(min_width);
        let cols_m1 = width_m1 / (min_width + column_spacing as usize);
        let cols = cols_m1 + 1;
        let item_width = width
            .saturating_sub(cols_m1 * column_spacing as usize)
            .checked_div(cols)
            .unwrap_or(0);
        Self {
            cols,
            item_width,
            column_spacing,
        }
    }
}

fn list_grid_metrics(space_xxs: u16, space_s: u16, width: usize) -> GridMetrics {
    GridMetrics::new(width, 320 + 2 * space_s as usize, space_xxs)
}

/// Ensure Cosmic Files and Settings are pinned to the dock by default (Files first).
fn load_pinned_apps_with_defaults() -> Vec<String> {
    const DEFAULTS: [&str; 2] = [COSMIC_FILES_APP_ID, COSMIC_SETTINGS_APP_ID];
    let Ok(config) = cosmic::cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
        return DEFAULTS.iter().map(|id| (*id).to_string()).collect();
    };
    let mut remaining: Vec<String> = config.get("pinned_apps").unwrap_or_default();
    let before = remaining.clone();
    let mut ordered: Vec<String> = Vec::with_capacity(remaining.len() + DEFAULTS.len());
    for &id in &DEFAULTS {
        if let Some(pos) = remaining.iter().position(|p| p == id) {
            ordered.push(remaining.remove(pos));
        } else {
            ordered.push(id.to_string());
        }
    }
    ordered.extend(remaining);
    if ordered != before {
        let _ = config.set("pinned_apps", &ordered);
    }
    ordered
}

fn bottom_bar_icon_fallback_name(app_id: &str) -> Cow<'static, str> {
    match app_id {
        COSMIC_FILES_APP_ID => Cow::Borrowed("com.system76.CosmicFiles"),
        COSMIC_SETTINGS_APP_ID => Cow::Borrowed("com.system76.CosmicSettings"),
        _ => Cow::Owned(
            app_id
                .strip_suffix(".desktop")
                .unwrap_or(app_id)
                .to_string(),
        ),
    }
}

fn bottom_bar_app_icon_by_id(
    apps: &[Arc<ApplicationEntry>],
    app_id: &str,
    size: u16,
) -> cosmic::widget::icon::Icon {
    if let Some(app) = apps.iter().find(|a| a.id == app_id) {
        return app_icon(app, size as f32);
    }
    let fallback = bottom_bar_icon_fallback_name(app_id);
    cosmic::widget::icon::from_name(fallback)
        .symbolic(false)
        .prefer_svg(true)
        .size(size)
        .fallback(Some(cosmic::widget::icon::IconFallback::Names(vec![
            "application-x-executable".into(),
            "application-default".into(),
        ])))
        .icon()
        .width(Length::Fixed(size as f32))
        .height(Length::Fixed(size as f32))
}

fn bottom_bar_pinned_item(
    apps: &[Arc<ApplicationEntry>],
    pinned_id: &str,
    label: Cow<'static, str>,
    hovered: bool,
    menu_too_small: bool,
    space_xxs: u16,
    space_xs: u16,
) -> Element<'static, Message> {
    let icon = bottom_bar_app_icon_by_id(apps, pinned_id, 20);
    let launch_btn = bottom_bar_action_button(
        icon,
        label,
        Message::LaunchAppById(pinned_id.to_string()),
        !hovered && !menu_too_small,
        menu_too_small,
        space_xxs,
        space_xs,
    );

    let content: Element<'static, Message> = if hovered {
        stack![
            launch_btn,
            container(
                corner_icon_button(
                    corner_unpin_icon(),
                    Message::UnpinFromTrayById(pinned_id.to_string()),
                    "Unpin from tray",
                ),
            )
            .align_x(Alignment::Start)
            .align_y(Alignment::Start)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(space_xxs),
        ]
        .width(Length::Fill)
        .into()
    } else {
        launch_btn
    };

    mouse_area(content)
        .on_enter(Message::PinnedBarHovered(pinned_id.to_string()))
        .on_exit(Message::PinnedBarUnhovered(pinned_id.to_string()))
        .into()
}

fn bottom_bar_action_button(
    icon: cosmic::widget::icon::Icon,
    label: Cow<'static, str>,
    message: Message,
    icon_only: bool,
    menu_too_small: bool,
    space_xxs: u16,
    space_xs: u16,
) -> Element<'static, Message> {
    let btn = cosmic::widget::button::custom(
        if icon_only || menu_too_small {
            Element::from(container(icon).center(Length::Fill))
        } else {
            Element::from(
                container(
                    row![icon, cosmic::widget::text::body(label.clone())]
                        .align_y(Alignment::Center)
                        .spacing(space_xxs),
                )
                .center(Length::Fill),
            )
        },
    )
    .on_press(message)
    .class(theme::Button::AppletMenu)
    .width(Length::Fill)
    .padding([space_xxs, space_xs]);

    if menu_too_small {
        cosmic::widget::tooltip(
            btn,
            cosmic::widget::text::body(label),
            cosmic::widget::tooltip::Position::Top,
        )
        .into()
    } else {
        btn.into()
    }
}

fn corner_fav_icon(is_favourite: bool) -> cosmic::widget::icon::Icon {
    let name = if is_favourite {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    };
    cosmic::widget::icon::from_name(name)
        .symbolic(true)
        .prefer_svg(true)
        .size(CORNER_BADGE_ICON_SIZE)
        .icon()
}

fn corner_pin_icon(is_pinned: bool) -> cosmic::widget::icon::Icon {
    let (name, fallbacks): (&str, &[&str]) = if is_pinned {
        ("window-pin-symbolic", &["pin-symbolic", "xapp-pin-symbolic"])
    } else {
        ("view-pin-symbolic", &["pin-symbolic", "xapp-pin-symbolic"])
    };
    cosmic::widget::icon::from_name(name)
        .symbolic(true)
        .prefer_svg(true)
        .size(CORNER_BADGE_ICON_SIZE)
        .fallback(Some(
            cosmic::widget::icon::IconFallback::Names(
                fallbacks.iter().map(|s| Cow::from(*s)).collect(),
            ),
        ))
        .icon()
}

fn corner_unpin_icon() -> cosmic::widget::icon::Icon {
    cosmic::widget::icon::from_name("window-unpin-symbolic")
        .symbolic(true)
        .prefer_svg(true)
        .size(CORNER_BADGE_ICON_SIZE)
        .fallback(Some(
            cosmic::widget::icon::IconFallback::Names(vec![
                Cow::Borrowed("xapp-unpin-symbolic"),
                Cow::Borrowed("pin-symbolic"),
            ]),
        ))
        .icon()
}

fn corner_icon_button(
    icon: cosmic::widget::icon::Icon,
    message: Message,
    tooltip: &'static str,
) -> Element<'static, Message> {
    cosmic::widget::tooltip(
        mouse_area(icon).on_press(message),
        cosmic::widget::text::body(tooltip),
        cosmic::widget::tooltip::Position::Top,
    )
    .into()
}

/// Pin (top-left) and favourite (top-right) action buttons shown on app hover.
fn app_pin_action_button(index: usize, is_pinned: bool) -> Element<'static, Message> {
    let (pin_msg, tooltip) = if is_pinned {
        (Message::UnpinFromTray(index), "Unpin from tray")
    } else {
        (Message::PinToTray(index), "Pin to tray")
    };
    corner_icon_button(corner_pin_icon(is_pinned), pin_msg, tooltip)
}

fn app_fav_action_button(index: usize, is_favourite: bool) -> Element<'static, Message> {
    let tooltip = if is_favourite {
        "Remove from favourites"
    } else {
        "Add to favourites"
    };
    corner_icon_button(
        corner_fav_icon(is_favourite),
        Message::ToggleFavourite(index),
        tooltip,
    )
}

/// Pin top-left, favourite top-right — icons appear only on app hover.
fn app_corner_overlay(
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
    space_xxs: u16,
    hovered: bool,
) -> Option<Element<'static, Message>> {
    if !hovered {
        return None;
    }

    let pin_left = app_pin_action_button(index, is_pinned);
    let fav_right = app_fav_action_button(index, is_favourite);

    Some(
        container(
            row![
                pin_left,
                Space::new().width(Length::Fill),
                fav_right,
            ]
            .width(Length::Fill)
            .align_y(Alignment::Start),
        )
        .align_y(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(space_xxs)
        .into(),
    )
}

/// Idle card look (matches `Container::Card`). Hover/active uses the
/// left-menu nav highlight background without changing text/icon colour.
fn app_list_card_class(selected: bool) -> theme::Button {
    use cosmic::iced::{Background, Color};
    use cosmic::widget::button::Style;

    let idle = |theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut style = Style::new();
        style.background = Some(Background::Color(component.base.into()));
        style.border_radius = cosmic.corner_radii.radius_s.into();
        style.text_color = Some(component.on.into());
        style.icon_color = Some(component.on.into());
        style
    };

    let highlight = |theme: &cosmic::Theme, alpha: f32| {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut bg: Color = cosmic.palette.neutral_5.into();
        bg.a = alpha;
        let mut style = Style::new();
        style.background = Some(Background::Color(bg));
        style.border_radius = cosmic.corner_radii.radius_s.into();
        style.text_color = Some(component.on.into());
        style.icon_color = Some(component.on.into());
        style
    };

    theme::Button::Custom {
        active: Box::new(move |_focused, theme| {
            if selected {
                highlight(theme, 0.2)
            } else {
                idle(theme)
            }
        }),
        disabled: Box::new(idle),
        // Match NavBar hover alpha (0.3) from libcosmic segmented_button.
        hovered: Box::new(move |_focused, theme| highlight(theme, 0.3)),
        pressed: Box::new(move |_focused, theme| highlight(theme, 0.25)),
    }
}

fn app_list_card<'a>(
    app: &'a ApplicationEntry,
    icon: cosmic::widget::icon::Icon,
    space_xxs: u16,
    space_s: u16,
    text_width: f32,
    width: usize,
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
    is_selected: bool,
    show_actions: bool,
) -> Element<'a, Message> {
    let summary = app
        .description
        .as_deref()
        .map(|d| truncate_name(d, 60))
        .unwrap_or_default();

    let effective_text_width = text_width.max(40.0);

    let name_row: Element<'_, Message> = cosmic::widget::text::body(&app.name)
        .height(Length::Fixed(20.0))
        .width(Length::Fixed(effective_text_width))
        .wrapping(cosmic::iced::widget::text::Wrapping::Word)
        .into();

    let card_height = LIST_ICON_SIZE as f32 + (space_xxs as f32) * 2.0;
    let card_width = width as f32;

    let card_body = row![
        icon,
        column![
            name_row,
            cosmic::widget::text::caption(summary)
                .height(Length::Fixed(28.0))
                .width(Length::Fixed(effective_text_width))
                .wrapping(cosmic::iced::widget::text::Wrapping::Word),
        ]
        .spacing(2),
    ]
    .align_y(Alignment::Center)
    .spacing(space_s)
    .width(Length::Fill);

    let card_content: Element<'a, Message> = if let Some(corners) = app_corner_overlay(
        index,
        is_favourite,
        is_pinned,
        space_xxs,
        show_actions,
    ) {
        stack![card_body, corners]
            .width(Length::Fixed(card_width))
            .height(Length::Fixed(card_height))
            .into()
    } else {
        card_body.into()
    };

    let btn = cosmic::widget::button::custom(card_content)
        .force_enabled(true)
        .padding([space_xxs, space_s])
        .width(Length::Fixed(card_width))
        .height(Length::Fixed(card_height))
        .class(app_list_card_class(is_selected));

    mouse_area(btn)
        .on_enter(Message::AppHovered(index))
        .on_exit(Message::AppUnhovered(index))
        .on_press(Message::LaunchApp(index))
        .into()
}

// ── Icon helpers ──

fn app_icon(app: &ApplicationEntry, size: f32) -> cosmic::widget::icon::Icon {
    let size_u16 = size as u16;
    if let Some(ref name) = app.icon {
        let name: Arc<str> = Arc::from(name.as_str());
        cosmic::widget::icon::from_name(name)
            .symbolic(false).prefer_svg(true).size(size_u16)
            .fallback(Some(cosmic::widget::icon::IconFallback::Names(vec![
                "application-x-executable".into(),
                "application-default".into(),
            ])))
            .icon()
            .width(Length::Fixed(size)).height(Length::Fixed(size))
    } else {
        cosmic::widget::icon::from_name("application-x-executable")
            .symbolic(false).prefer_svg(true).size(size_u16)
            .icon()
            .width(Length::Fixed(size)).height(Length::Fixed(size))
    }
}

fn truncate_name<'a>(name: &'a str, max_chars: usize) -> Cow<'a, str> {
    if name.len() <= max_chars {
        return Cow::Borrowed(name);
    }
    // Use char_indices to find a safe UTF-8 boundary.
    let mut end = 0;
    for (i, (byte_pos, _)) in name.char_indices().enumerate() {
        if i >= max_chars.saturating_sub(1) {
            end = byte_pos;
            break;
        }
    }
    if end == 0 {
        end = name.len();
    }
    Cow::Owned(format!("{}…", &name[..end]))
}

fn main() -> cosmic::iced::Result {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "warn")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);
    let args: Vec<String> = std::env::args().collect();
    let is_window = args.iter().any(|a| a == "--window");
    if is_window {
        cosmic::app::run::<Applet>(cosmic::app::Settings::default(), ())
    } else {
        cosmic::applet::run::<Applet>(())
    }
}
