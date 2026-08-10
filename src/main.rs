// SPDX-License-Identifier: GPL-3.0-only

mod apps;
mod config;

use config::AppletConfig;

use cosmic::app::{Core, Task};
use cosmic::applet::cosmic_panel_config::PanelAnchor;
use cosmic::cctk::sctk::reexports::protocols::xdg::shell::client::xdg_positioner::{
    Anchor, Gravity,
};
use cosmic::cosmic_config::ConfigGet;
use cosmic::cosmic_config::ConfigSet;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{
    event::listen_raw,
    keyboard::key::Named,
    widget::{column, container, row, scrollable, Space},
    window::Id,
    Alignment, Length, Limits,
};
use cosmic::surface::action::{app_popup, app_window, destroy_popup, destroy_window, LiveSettings};
use cosmic::theme;
use cosmic::widget::{
    grid, menu, mouse_area, nav_bar, nav_bar_toggle, segmented_button,
};
use cosmic::{Application, Element};

use apps::{ApplicationCategory, ApplicationEntry};
use std::borrow::Cow;
use std::sync::{Arc, LazyLock};

const APP_ID: &str = "com.github.cosmic-kde-launcher";
const LIST_ICON_SIZE: u16 = 48;
const SIDEBAR_WIDTH: f32 = 220.0;
static SEARCH_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);
static SCROLLABLE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(cosmic::widget::Id::unique);
// Context menus never bind keys, so one shared empty key-bind map is reused
// instead of allocating a fresh HashMap per app cell per view.
static EMPTY_MENU_KEYBINDS: LazyLock<
    std::collections::HashMap<menu::KeyBind, AppContextAction>,
> = LazyLock::new(Default::default);

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
    window_id: Option<Id>,
    window_maximized: bool,
    search_field: String,
    search_active: bool,
    all_applications: Vec<Arc<ApplicationEntry>>,
    available_applications: Vec<Arc<ApplicationEntry>>,
    available_categories: Vec<ApplicationCategory>,
    selected_category: Option<ApplicationCategory>,
    config: AppletConfig,
    selected_index: Option<usize>,
    nav_model: segmented_button::SingleSelectModel,
    sidebar_collapsed: bool,
    show_settings: bool,
    pinned_apps: Vec<String>,
    /// Scroll offset (px) of the app-grid scrollable, tracked for virtualized
    /// rendering so only rows in the viewport are built each view.
    grid_scroll_y: f32,
    /// Height (px) of the app-grid viewport, from the scrollable's viewport.
    grid_viewport_h: f32,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ClosePopup,
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
    SetPanelIcon(String),
    TogglePanelIconSymbolic,
    ToggleShowFavourites,
    ToggleShowRecents,
    ToggleFavourite(usize),
    PinToTray(usize),
    UnpinFromTray(usize),
    SetDefaultCategory(String),
    ToggleSidebarDefault,
    Surface(cosmic::surface::Action),
    ToggleFullWindow,
    WindowMinimize,
    WindowToggleMaximize,
    PowerAction(PowerAction),
    CategoryActivated(segmented_button::Entity),
    /// App grid scrolled — carries the absolute vertical offset (px) and the
    /// viewport height (px), used to render only visible rows.
    GridScrolled(f32, f32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppContextAction {
    ToggleFav(usize),
    PinToTray(usize),
    UnpinFromTray(usize),
}

impl menu::Action for AppContextAction {
    type Message = Message;
    fn message(&self) -> Self::Message {
        match self {
            AppContextAction::ToggleFav(i) => Message::ToggleFavourite(*i),
            AppContextAction::PinToTray(i) => Message::PinToTray(*i),
            AppContextAction::UnpinFromTray(i) => Message::UnpinFromTray(*i),
        }
    }
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

        // Load currently pinned apps from CosmicDock config
        let pinned_apps: Vec<String> = cosmic::cosmic_config::Config::new("com.system76.CosmicDock", 1)
            .ok()
            .and_then(|cfg| cfg.get("pinned_apps").ok())
            .unwrap_or_default();

        let mut applet = Self {
            core,
            popup: None,
            window_id: None,
            window_maximized: false,
            search_field: String::new(),
            search_active: false,
            all_applications: apps.clone(),
            available_applications: apps,
            available_categories: categories,
            selected_category: None,
            sidebar_collapsed: config.sidebar_collapsed,
            config,
            selected_index: None,
            nav_model: segmented_button::SingleSelectModel::default(),
            show_settings: false,
            pinned_apps,
            grid_scroll_y: 0.0,
            grid_viewport_h: 0.0,
        };
        applet.rebuild_nav_model();
        (applet, Task::none())
    }

    fn view(&self) -> Element<'_, Message> {
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
        let cosmic_theme = theme::active();
        let Spacing { space_xxs, space_xs, space_s, space_m, .. } = cosmic_theme.cosmic().spacing;
        let icon_size = self.config.icon_size;
        let menu_width = self.config.max_width();
        let menu_height = self.config.max_height();
        // Bottom bar height: button content (~21px) + button padding (2×space_xxs)
        // + symmetric row padding (2×space_xxs), minimum 44px.
        let bottom_bar_height = 21.0 + space_xxs as f32 * 4.0;
        let bottom_bar_height = bottom_bar_height.max(44.0);
        tracing::debug!(
            "view_window: menu={}×{} bottom_bar_h={} space_m={} space_s={}",
            menu_width, menu_height, bottom_bar_height, space_m, space_s
        );

        // Calculate the exact height available for the app area so we don't
        // rely on nested Fill inside Shrink chains (which can collapse in the
        // popup_container's autosize wrapper).
        let outer_pad = space_xs as f32 * 2.0;
        let has_title = self.sidebar_collapsed && self.selected_category.is_some();
        let col_spacing_count: u32 = 1 // top_bar → next
            + if self.search_active { 1 } else { 0 }
            + if has_title { 1 } else { 0 }
            + 1 // → dual_pane
            + 1; // → bottom_bar
        let total_spacing = col_spacing_count as f32 * space_xxs as f32;
        let app_area_height = (menu_height - outer_pad - total_spacing
            - bottom_bar_height)
            .max(200.0);
        tracing::debug!(
            "view_window: outer_pad={} col_spacings={} total_spacing={} app_area_h={}",
            outer_pad, col_spacing_count, total_spacing, app_area_height
        );

        // ── Slim top bar: sidebar toggle + search button + config icon ──
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

        let is_window = self.window_id == Some(_id);

        // Window control buttons (minimize, maximize, close)
        let wc_minimize: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("window-minimize-symbolic")
                .symbolic(true).size(16).icon(),
        )
        .on_press(if is_window { Message::WindowMinimize } else { Message::ClosePopup })
        .class(theme::Button::HeaderBar)
        .padding(8)
        .into();

        let wc_maximize: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name(
                if is_window && self.window_maximized {
                    "window-restore-symbolic"
                } else {
                    "window-maximize-symbolic"
                }
            ).symbolic(true).size(16).icon(),
        )
        .on_press(if is_window { Message::WindowToggleMaximize } else { Message::ToggleFullWindow })
        .class(theme::Button::HeaderBar)
        .padding(8)
        .into();

        let wc_close: Element<'_, Message> = cosmic::widget::button::custom(
            cosmic::widget::icon::from_name("window-close-symbolic")
                .symbolic(true).size(16).icon(),
        )
        .on_press(if is_window { Message::ToggleFullWindow } else { Message::ClosePopup })
        .class(theme::Button::HeaderBar)
        .padding(8)
        .into();

        let top_bar = container(
            row![
                sidebar_toggle,
                search_toggle_btn,
                Space::new().width(Length::Fill),
                config_btn,
                wc_minimize,
                wc_maximize,
                wc_close,
            ]
            .align_y(Alignment::Center)
            .spacing(space_s),
        )
        .width(Length::Fill);

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
            nav_bar(&self.nav_model, Message::CategoryActivated)
                .into_container()
                .width(Length::Fixed(SIDEBAR_WIDTH))
                .height(Length::Fill)
                .padding([0, space_xxs, space_xxs, 0])
                .into()
        };

        // ── App area ──
        // Precompute favourite/pinned lookups once per view (O(n) instead of
        // O(apps × favourites) per cell).
        let fav_set: std::collections::HashSet<&str> =
            self.config.favourites.iter().map(String::as_str).collect();
        let pinned_set: std::collections::HashSet<&str> =
            self.pinned_apps.iter().map(String::as_str).collect();

        // In Hybrid mode, use grid for Favourites/Recents categories, list otherwise.
        let use_grid = self.config.layout_mode == config::LayoutMode::Grid
            || (self.config.layout_mode == config::LayoutMode::Hybrid
                && matches!(self.selected_category.as_ref(),
                    Some(cat) if cat.key == "favourites" || cat.key == "recents"));
        // Hybrid grid uses a smaller icon size for compact display.
        let effective_icon_size: f32 = if self.config.layout_mode == config::LayoutMode::Hybrid {
            32.0
        } else {
            icon_size
        };

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
                        let icon = app_icon(app, effective_icon_size);
                        let name = truncate_name(&app.name, 32);
                        let is_selected = self.selected_index == Some(index);
                        let is_fav = fav_set.contains(app.id.as_str());
                        // Star badge for favourited apps
                        let content: Element<'_, Message> = {
                            let inner: Element<'_, Message> = if is_fav {
                                let star = cosmic::widget::icon::from_name("starred-symbolic")
                                    .symbolic(true).size(12).icon();
                                column![
                                    icon,
                                    row![
                                        cosmic::widget::text::caption(name)
                                            .wrapping(cosmic::iced::widget::text::Wrapping::Word),
                                        Space::new().width(Length::Fixed(space_xxs as f32)),
                                        star,
                                    ].align_y(Alignment::Center),
                                ]
                                .align_x(Alignment::Center).spacing(space_xxs)
                                .into()
                            } else {
                                column![
                                    icon,
                                    cosmic::widget::text::caption(name)
                                        .wrapping(cosmic::iced::widget::text::Wrapping::Word),
                                ]
                                    .align_x(Alignment::Center).spacing(space_xxs)
                                    .into()
                            };
                            container(inner)
                                .center_x(Length::Fill)
                                .center_y(Length::Fill)
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .into()
                        };
                        let btn = cosmic::widget::button::custom(content)
                            .on_press(Message::LaunchApp(index))
                            .class(if is_selected { theme::Button::Suggested } else { theme::Button::AppletMenu })
                            .width(Length::Fill)
                            .height(Length::Fixed(cell_height));

                        let fav_label = if is_fav { "Unfavourite" } else { "Favourite" };
                        let is_pinned = pinned_set.contains(app.id.as_str());
                        let pin_label = if is_pinned { "Unpin from Tray" } else { "Pin to Tray" };
                        let pin_action = if is_pinned {
                            AppContextAction::UnpinFromTray(index)
                        } else {
                            AppContextAction::PinToTray(index)
                        };
                        let ctx_menu = menu::items(
                            &EMPTY_MENU_KEYBINDS,
                            vec![
                                menu::Item::Button(fav_label, None, AppContextAction::ToggleFav(index)),
                                menu::Item::Button(pin_label, None, pin_action),
                            ],
                        );

                        cosmic::widget::context_menu(btn, Some(ctx_menu))
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
            // List view — Cosmic Store card grid
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

            let mut app_grid = grid();
            let mut col = 0;
            for (index, app) in self.available_applications.iter().enumerate() {
                if col >= cols {
                    app_grid = app_grid.insert_row();
                    col = 0;
                }
                app_grid = app_grid.push(app_list_card(
                    app,
                    space_xxs,
                    space_s,
                    item_width,
                    index,
                    fav_set.contains(app.id.as_str()),
                    pinned_set.contains(app.id.as_str()),
                ));
                col += 1;
            }

            container(
                scrollable(
                    container(
                        app_grid
                            .column_spacing(column_spacing)
                            .row_spacing(column_spacing)
                            .width(Length::Fill),
                    )
                    .width(Length::Fill),
                )
                .id((*SCROLLABLE_ID).clone())
                .height(Length::Fill)
                .on_scroll(|vp| {
                    Message::GridScrolled(vp.absolute_offset().y, vp.bounds().height)
                }),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        };

        let app_area: Element<'_, Message> = container(app_area)
            .width(Length::Fill)
            .height(Length::Fill)
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

                // ── Panel icon picker ──
                let icon_presets: &[(&str, &str)] = &[
                    // ── COSMIC & Generic ──
                    ("com.system76.CosmicAppLibrary", "COSMIC"),
                    ("application-menu-symbolic", "App Menu"),
                    ("open-menu-symbolic", "Menu"),
                    ("start-here-symbolic", "Start"),
                    ("distributor-logo", "Linux"),
                    ("applications-all-symbolic", "All Apps"),
                    // ── Desktop Environments ──
                    ("kde", "KDE"),
                    ("plasma", "Plasma"),
                    ("kmenu", "K Menu"),
                    ("gnome-main-menu", "GNOME"),
                    // ── Distro Logos ──
                    ("distributor-logo-pop-os", "Pop!_OS"),
                    ("start-here-ubuntu", "Ubuntu"),
                    ("start-here-kubuntu", "Kubuntu"),
                    ("start-here-lubuntu", "Lubuntu"),
                    ("start-here-xubuntu", "Xubuntu"),
                    ("start-here-ubuntu-mate", "Ubuntu MATE"),
                    ("start-here-ubuntu-gnome", "Ubuntu GNOME"),
                    ("start-here-fedora", "Fedora"),
                    ("distributor-logo-debian", "Debian"),
                    ("distributor-logo-archlinux", "Arch"),
                    ("distributor-logo-manjaro", "Manjaro"),
                    ("distributor-logo-opensuse", "openSUSE"),
                    ("distributor-logo-solus", "Solus"),
                    ("distributor-logo-elementary", "elementary"),
                    ("distributor-logo-linux-mint", "Mint"),
                    ("distributor-logo-slackware", "Slackware"),
                    ("distributor-logo-mageia", "Mageia"),
                    // ── Category icons ──
                    ("applications-system-symbolic", "System"),
                    ("applications-engineering-symbolic", "Dev"),
                    ("applications-games-symbolic", "Games"),
                    ("applications-graphics-symbolic", "Graphics"),
                    ("applications-multimedia-symbolic", "Media"),
                    ("applications-office-symbolic", "Office"),
                    ("applications-science-symbolic", "Science"),
                    ("applications-utilities-symbolic", "Utils"),
                    // ── Actions ──
                    ("computer-symbolic", "Computer"),
                    ("system-run-symbolic", "Run"),
                    ("system-search-symbolic", "Search"),
                    ("preferences-system-symbolic", "Settings"),
                    ("emblem-system-symbolic", "System"),
                ];

                // Build icon grid: 3 columns. Build rows by index to avoid Element cloning.
                let col_count = 3;
                let row_count = (icon_presets.len() + col_count - 1) / col_count;
                let icon_rows: Vec<Element<'_, Message>> = (0..row_count)
                    .map(|row_idx| {
                        let start = row_idx * col_count;
                        let end = ((row_idx + 1) * col_count).min(icon_presets.len());
                        let btns: Vec<Element<'_, Message>> = (start..end)
                            .map(|i| {
                                let (icon_name, label) = icon_presets[i];
                                let is_active = self.config.panel_icon == icon_name
                                    || (self.config.panel_icon.is_empty() && icon_name == "com.system76.CosmicAppLibrary");
                                let icon = cosmic::widget::icon::from_name(icon_name)
                                    .symbolic(false).prefer_svg(true).size(36).icon();
                                cosmic::widget::button::custom(
                                    column![
                                        icon,
                                        cosmic::widget::text::caption(label),
                                    ]
                                    .width(Length::Fill)
                                    .align_x(Alignment::Center)
                                    .spacing(2),
                                )
                                .on_press(Message::SetPanelIcon(icon_name.to_string()))
                                .class(if is_active { theme::Button::Suggested } else { theme::Button::AppletMenu })
                                .width(Length::Fill)
                                .into()
                            })
                            .collect();
                        row(btns).spacing(space_xxs).width(Length::Fill).align_y(Alignment::Center).into()
                    })
                    .collect();


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
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Panel Icon"),
                    Space::new().height(space_xxs),
                    column(icon_rows).spacing(space_xxs),
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

        // ── Bottom bar: power actions ──
        let menu_too_small = menu_height < 600.0;
        let power_buttons: Vec<Element<'_, Message>> = PowerAction::BOTTOM_BAR
            .iter()
            .map(|&action| {
                let icon = cosmic::widget::icon::from_name(action.icon_name())
                    .symbolic(true).size(20).icon();
                let btn = cosmic::widget::button::custom(
                    if menu_too_small {
                        // Small menu: icon only, centered in button
                        Element::from(
                            container(icon).center(Length::Fill)
                        )
                    } else {
                        // Center icon and label within the button
                        Element::from(
                            container(
                                row![icon, cosmic::widget::text::body(action.label())]
                                    .align_y(Alignment::Center)
                                    .spacing(space_xxs),
                            )
                            .center(Length::Fill)
                        )
                    },
                )
                .on_press(Message::PowerAction(action))
                .class(theme::Button::AppletMenu)
                .width(Length::Fill)
                .padding([space_xxs, space_xs]);

                let btn_elem: Element<'_, Message> = if menu_too_small {
                    // Wrap in tooltip to show label on hover
                    cosmic::widget::tooltip(
                        btn,
                        cosmic::widget::text::body(action.label()),
                        cosmic::widget::tooltip::Position::Top,
                    )
                    .into()
                } else {
                    btn.into()
                };

                btn_elem
            })
            .collect();

        let bottom_bar = container(
            row(power_buttons)
                .spacing(space_s)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .padding([space_xxs, space_xxs, space_xxs, space_xxs]),
        )
        .height(Length::Fixed(bottom_bar_height))
        .width(Length::Fill)
        .class(theme::Container::Primary);

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
        let main_col = main_col
            .push(dual_pane)
            .push(bottom_bar);

        // Outer container: fixed size so the Fill column has a definite height
        // to distribute. The column's own padding provides visual spacing from
        // the popup edges.
        let is_window = self.window_id == Some(_id);

        if is_window {
            // Window mode: fill the entire window, use a regular container
            // with the theme's background colour.
            container(main_col)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| {
                    let cosmic = theme.cosmic();
                    cosmic::iced::widget::container::Style {
                        background: Some(
                            cosmic::iced::Color::from(cosmic.background(false).base).into(),
                        ),
                        ..Default::default()
                    }
                })
                .into()
        } else {
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
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            Message::TogglePopup => {
                if let Some(wid) = self.window_id.take() {
                    // Window is open — close it and return to applet mode
                    self.window_maximized = false;
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_window(wid))));
                }
                if let Some(p) = self.popup.take() {
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
                }
                self.search_field.clear();
                self.search_active = false;
                self.selected_index = None;
                self.grid_scroll_y = 0.0;
                self.grid_viewport_h = 0.0;
                self.sidebar_collapsed = self.config.sidebar_collapsed;
                // Use cached apps from init — a background refresh task
                // keeps them up to date without blocking the UI.
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
                let anchor = self.core.applet.anchor;
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
                        let (a, g) = match anchor {
                            PanelAnchor::Top => (Anchor::BottomLeft, Gravity::BottomRight),
                            PanelAnchor::Bottom => (Anchor::TopLeft, Gravity::TopRight),
                            PanelAnchor::Left => (Anchor::TopRight, Gravity::BottomRight),
                            PanelAnchor::Right => (Anchor::TopLeft, Gravity::BottomLeft),
                        };
                        popup_settings.positioner.anchor = a;
                        popup_settings.positioner.gravity = g;
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
                if self.window_id == Some(id) { self.window_id = None; }
                Task::none()
            }
            Message::ClosePopup => {
                if let Some(p) = self.popup.take() { return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p)))); }
                Task::none()
            }
            Message::ToggleFullWindow => {
                if let Some(wid) = self.window_id.take() {
                    // Window → popup: close the window first
                    self.window_maximized = false;
                    return Task::done(cosmic::Action::App(Message::Surface(destroy_window(wid))));
                }
                // Popup → window: close popup (if open) and create a full window
                let close_popup = if let Some(p) = self.popup.take() {
                    Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))))
                } else {
                    Task::none()
                };
                let (new_id, action) = app_window::<Applet>(
                    |_| LiveSettings::default(),
                    |_state: &mut Applet| {
                        cosmic::iced::window::Settings {
                            size: cosmic::iced::Size::new(1200.0, 800.0),
                            maximized: true,
                            resizable: true,
                            decorations: false,
                            ..Default::default()
                        }
                    },
                    None,
                );
                self.window_id = Some(new_id);
                self.window_maximized = true;
                let create_window = Task::done(cosmic::Action::App(Message::Surface(action)));
                Task::batch([close_popup, create_window])
            }
            Message::WindowMinimize => {
                if let Some(wid) = self.window_id {
                    return cosmic::command::minimize::<Message>(wid);
                }
                Task::none()
            }
            Message::WindowToggleMaximize => {
                if let Some(wid) = self.window_id {
                    self.window_maximized = !self.window_maximized;
                    return cosmic::command::toggle_maximize::<Message>(wid);
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
                self.grid_scroll_y = y;
                self.grid_viewport_h = viewport_h;
                Task::none()
            }
            Message::LaunchApp(index) => {
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
                self.config.custom_width = 0.0;
                self.config.custom_height = 0.0;
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
                    self.config.toggle_favourite(&app_id);
                    self.config.save();
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
                }
                Task::none()
            }
            Message::UnpinFromTray(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    self.pinned_apps = self.unpin_app_from_dock(&app_id);
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
        }
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        // Only subscribe to keyboard shortcuts when the popup is open.
        if self.popup.is_none() {
            return cosmic::iced::Subscription::none();
        }
        listen_raw(|event, _status, _id| match event {
            cosmic::iced::Event::Keyboard(cosmic::iced::keyboard::Event::KeyPressed {
                key: cosmic::iced::keyboard::Key::Named(key), ..
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
        })
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Applet {
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

    fn rebuild_nav_model(&mut self) {
        self.nav_model.clear();

        // "All Applications" entry
        let all_icon = cosmic::widget::icon::from_name("applications-all-symbolic")
            .symbolic(true).size(16).icon();
        self.nav_model.insert()
            .text("All Applications")
            .icon(all_icon)
            .data(ApplicationCategory::all())
            .activate();

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
            let cleaned_exec = exec
                .replace("%f", "").replace("%F", "")
                .replace("%u", "").replace("%U", "")
                .replace("%i", "").replace("%c", "").replace("%k", "");
            let cleaned_exec = cleaned_exec.trim();
            if !cleaned_exec.is_empty() {
                if app.is_terminal {
                    let _ = std::process::Command::new("sh")
                        .arg("-c").arg(&format!("cosmic-term -- {}", cleaned_exec)).spawn();
                } else {
                    let _ = std::process::Command::new("sh")
                        .arg("-c").arg(cleaned_exec).spawn();
                }
            }
        }
        if let Some(p) = self.popup.take() {
            return Task::done(cosmic::Action::App(Message::Surface(destroy_popup(p))));
        }
        Task::none()
    }
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
    GridMetrics::new(width, 240 + 2 * space_s as usize, space_xxs)
}

fn app_list_card<'a>(
    app: &'a ApplicationEntry,
    space_xxs: u16,
    space_s: u16,
    width: usize,
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
) -> Element<'a, Message> {
    let icon = app_icon(app, LIST_ICON_SIZE as f32);
    let summary = app
        .description
        .as_deref()
        .map(|d| truncate_name(d, 60))
        .unwrap_or_default();

    let name_row: Element<'_, Message> = if is_favourite {
        let star = cosmic::widget::icon::from_name("starred-symbolic")
            .symbolic(true).size(12).icon();
        row![
            cosmic::widget::text::body(&app.name)
                .wrapping(cosmic::iced::widget::text::Wrapping::Word),
            Space::new().width(Length::Fixed(space_xxs as f32)),
            star,
        ].align_y(Alignment::Center).into()
    } else {
        cosmic::widget::text::body(&app.name)
            .wrapping(cosmic::iced::widget::text::Wrapping::Word)
            .into()
    };

    let card = mouse_area(
        container(
            row![
                icon,
                column![
                    name_row,
                    cosmic::widget::text::caption(summary).height(Length::Fixed(28.0)),
                ]
                .spacing(space_xxs),
            ]
            .align_y(Alignment::Center)
            .spacing(space_s),
        )
        .align_y(Alignment::Center)
        .width(Length::Fixed(width as f32))
        .height(Length::Fixed(64.0 + (space_xxs as f32) * 2.0))
        .padding([space_xxs, space_s])
        .class(theme::Container::Card),
    )
    .on_press(Message::LaunchApp(index));

    let fav_label = if is_favourite { "Unfavourite" } else { "Favourite" };
    let pin_label = if is_pinned { "Unpin from Tray" } else { "Pin to Tray" };
    let pin_action = if is_pinned {
        AppContextAction::UnpinFromTray(index)
    } else {
        AppContextAction::PinToTray(index)
    };
    let ctx_menu = menu::items(
        &EMPTY_MENU_KEYBINDS,
        vec![
            menu::Item::Button(fav_label, None, AppContextAction::ToggleFav(index)),
            menu::Item::Button(pin_label, None, pin_action),
        ],
    );

    cosmic::widget::context_menu(card, Some(ctx_menu)).into()
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
    cosmic::applet::run::<Applet>(())
}
