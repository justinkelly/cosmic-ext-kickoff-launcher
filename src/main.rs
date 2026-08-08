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
    platform_specific::shell::commands::popup::{destroy_popup, get_popup},
    widget::{column, container, row, scrollable, Space},
    window::Id,
    Alignment, Length, Limits,
};
use cosmic::theme;
use cosmic::widget::{
    grid, menu, mouse_area, nav_bar, nav_bar_toggle, segmented_button,
};
use cosmic::{Application, Element};

use apps::{ApplicationCategory, ApplicationEntry};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

const APP_ID: &str = "com.github.cosmic-kde-menu";
const LIST_ICON_SIZE: u16 = 48;
const SIDEBAR_WIDTH: f32 = 220.0;

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
    ToggleLayout,
    ToggleSidebar,
    ToggleSettings,
    SetSizePreset(config::SizePreset),
    SetPanelIcon(String),
    TogglePanelIconSymbolic,
    ToggleShowFavourites,
    ToggleShowRecents,
    ToggleFavourite(usize),
    PinToTray(usize),
    PowerAction(PowerAction),
    CategoryActivated(segmented_button::Entity),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppContextAction {
    ToggleFav(usize),
    PinToTray(usize),
}

impl menu::Action for AppContextAction {
    type Message = Message;
    fn message(&self) -> Self::Message {
        match self {
            AppContextAction::ToggleFav(i) => Message::ToggleFavourite(*i),
            AppContextAction::PinToTray(i) => Message::PinToTray(*i),
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

        let mut applet = Self {
            core,
            popup: None,
            search_field: String::new(),
            search_active: false,
            all_applications: apps.clone(),
            available_applications: apps,
            available_categories: categories,
            selected_category: None,
            config,
            selected_index: None,
            nav_model: segmented_button::SingleSelectModel::default(),
            sidebar_collapsed: false,
            show_settings: false,
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
        // Bottom bar height from theme: icon(20) + 2×padding
        let bottom_bar_height = 20.0 + space_s as f32 * 2.0;
        let bottom_bar_height = bottom_bar_height.max(44.0);
        let search_height = if self.search_active { 44.0 } else { 0.0 };
        // Reserve space for top bar, padding, separator, and bottom power bar.
        let title_height = if self.sidebar_collapsed && self.selected_category.is_some() { space_m as f32 * 2.0 + 20.0 } else { 0.0 };
        // Chrome height from theme: outer padding(space_m×2) + top bar(~30) + search + title + bottom bar + sep(~2)
        let chrome_height = space_m as f32 * 4.0 + search_height + title_height + bottom_bar_height + 30.0;
        let content_height = (menu_height - chrome_height).max(200.0);

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

        let top_bar = container(
            row![
                sidebar_toggle,
                search_toggle_btn,
                Space::new().width(Length::Fill),
                config_btn,
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
                .height(Length::Fixed(content_height))
                .into()
        };

        // ── App area ──
        let app_area: Element<'_, Message> = if self.available_applications.is_empty() {
            container(cosmic::widget::text::body("No applications found."))
                .center_x(Length::Fill).center_y(Length::Fill)
                .height(Length::Fixed(content_height))
                .width(Length::Fill)
                .into()
        } else if self.config.use_grid {
            // Dynamic column count based on available width.
            let mut avail_width = menu_width - (space_m as f32 * 2.0);
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
            let app_rows: Vec<Element<'_, Message>> = self.available_applications
                .chunks(grid_columns)
                .enumerate()
                .map(|(row_idx, chunk)| {
                    let buttons: Vec<Element<'_, Message>> = chunk.iter().enumerate()
                        .map(|(col_idx, app)| {
                            let index = row_idx * grid_columns + col_idx;
                            let icon = app_icon(app, icon_size);
                            let name = truncate_name(&app.name, 16);
                            let is_selected = self.selected_index == Some(index);
                            let is_fav = self.config.is_favourite(&app.id);
                            // Star badge for favourited apps
                            let content: Element<'_, Message> = {
                                let inner: Element<'_, Message> = if is_fav {
                                    let star = cosmic::widget::icon::from_name("starred-symbolic")
                                        .symbolic(true).size(12).icon();
                                    column![
                                        icon,
                                        row![
                                            cosmic::widget::text::caption(name),
                                            Space::new().width(Length::Fixed(space_xxs as f32)),
                                            star,
                                        ].align_y(Alignment::Center),
                                    ]
                                    .align_x(Alignment::Center).spacing(space_xxs)
                                    .into()
                                } else {
                                    column![icon, cosmic::widget::text::caption(name)]
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
                            // Fixed height for uniform grid cells: icon(48) + text(~16) + gaps
                            let cell_height = icon_size + 24.0 + space_xxs as f32 * 2.0;
                            let btn = cosmic::widget::button::custom(content)
                                .on_press(Message::LaunchApp(index))
                                .class(if is_selected { theme::Button::Suggested } else { theme::Button::AppletMenu })
                                .width(Length::Fill)
                                .height(Length::Fixed(cell_height));

                            let fav_label = if is_fav { "Unfavourite" } else { "Favourite" };
                            let ctx_menu = menu::items(
                                &std::collections::HashMap::new(),
                                vec![
                                    menu::Item::Button(fav_label, None, AppContextAction::ToggleFav(index)),
                                    menu::Item::Button("Pin to Tray", None, AppContextAction::PinToTray(index)),
                                ],
                            );

                            cosmic::widget::context_menu(btn, Some(ctx_menu))
                                .into()
                        }).collect();

                    let mut buttons = buttons;
                    let missing = grid_columns - buttons.len();
                    for _ in 0..missing {
                        buttons.push(Space::new().width(Length::Fill).into());
                    }
                    row(buttons).spacing(space_s).width(Length::Fill).into()
                }).collect();

            let app_grid = column(app_rows).spacing(space_s).width(Length::Fill);
            container(
                scrollable(app_grid)
                    .id((*SCROLLABLE_ID).clone())
                    .height(Length::Fixed(content_height)),
            )
            .width(Length::Fill)
            .into()
        } else {
            // List view — Cosmic Store card grid
            let mut list_width = menu_width as usize;
            list_width = list_width.saturating_sub(space_m as usize * 2);
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
                    self.config.is_favourite(&app.id),
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
                .height(Length::Fixed(content_height)),
            )
            .width(Length::Fill)
            .into()
        };

        let app_area: Element<'_, Message> = container(app_area)
            .width(Length::Fill)
            .height(Length::Fixed(content_height))
            .into();

        // ── Settings panel (right side) ──
        let settings_panel: Option<Element<'_, Message>> = if self.show_settings {
            let settings_content = {
                // Grid view button
                let check: Element<'_, Message> = if self.config.use_grid {
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
                .on_press(if self.config.use_grid { Message::ToggleSettings } else { Message::ToggleLayout })
                .class(if self.config.use_grid { theme::Button::Suggested } else { theme::Button::AppletMenu })
                .width(Length::Fill)
                .into();

                // List view button
                let check: Element<'_, Message> = if !self.config.use_grid {
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
                .on_press(if !self.config.use_grid { Message::ToggleSettings } else { Message::ToggleLayout })
                .class(if !self.config.use_grid { theme::Button::Suggested } else { theme::Button::AppletMenu })
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
                    ("distributor-logo-ubuntu", "Ubuntu"),
                    ("distributor-logo-kubuntu", "Kubuntu"),
                    ("distributor-logo-lubuntu", "Lubuntu"),
                    ("distributor-logo-xubuntu", "Xubuntu"),
                    ("distributor-logo-ubuntu-mate", "Ubuntu MATE"),
                    ("distributor-logo-ubuntu-gnome", "Ubuntu GNOME"),
                    ("distributor-logo-fedora", "Fedora"),
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
                                    .align_x(Alignment::Center)
                                    .spacing(2),
                                )
                                .on_press(Message::SetPanelIcon(icon_name.to_string()))
                                .class(if is_active { theme::Button::Suggested } else { theme::Button::AppletMenu })
                                .width(Length::Fill)
                                .into()
                            })
                            .collect();
                        row(btns).spacing(space_xxs).width(Length::Fill).into()
                    })
                    .collect();

                // Custom icon text input
                let icon_input = cosmic::widget::text_input(
                    if self.config.panel_icon.is_empty() { "" } else { &self.config.panel_icon },
                    if self.config.panel_icon.is_empty() { "" } else { &self.config.panel_icon },
                )
                .on_input(Message::SetPanelIcon)
                .width(Length::Fill)
                .padding([space_xxs, space_s]);

                column![
                    cosmic::widget::text::heading("Appearance"),
                    Space::new().height(space_m),
                    grid_btn,
                    list_btn,
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Menu Size"),
                    Space::new().height(space_xxs),
                    column(size_btns).spacing(space_xxs),
                    Space::new().height(space_m),
                    cosmic::widget::text::heading("Panel Icon"),
                    Space::new().height(space_xxs),
                    column(icon_rows).spacing(space_xxs),
                    Space::new().height(space_xxs),
                    cosmic::widget::text::caption("Or type a custom icon name:"),
                    icon_input,
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
                ].spacing(space_xxs).padding(space_s)
            };

            Some(
                container(
                    scrollable(settings_content)
                        .height(Length::Fixed(content_height)),
                )
                .width(Length::Fixed(280.0))
                .height(Length::Fixed(content_height))
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
                        // Small menu: icon only
                        Element::from(icon)
                    } else {
                        row![icon, cosmic::widget::text::body(action.label())]
                            .align_y(Alignment::Center)
                            .spacing(space_xxs)
                            .into()
                    },
                )
                .on_press(Message::PowerAction(action))
                .class(theme::Button::AppletMenu);

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

                container(btn_elem)
                    .width(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            })
            .collect();

        let bottom_bar = container(
            row(power_buttons)
                .spacing(space_s)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .padding([space_xs, space_m]),
        )
        .height(Length::Fixed(bottom_bar_height))
        .width(Length::Fill);

        // ── Main layout ──
        let mut dual_pane = row![]
            .spacing(space_xxs)
            .width(Length::Fill)
            .height(Length::Fixed(content_height));

        if !self.sidebar_collapsed {
            dual_pane = dual_pane.push(nav);
        }
        dual_pane = dual_pane.push(app_area);
        if let Some(sp) = settings_panel {
            dual_pane = dual_pane.push(sp);
        }

        let sep: Element<'_, Message> = cosmic::widget::divider::horizontal::default().into();

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

        let mut layout_col = column![top_bar]
            .spacing(space_m)
            .padding(space_m)
            .width(Length::Fill);

        if let Some(sr) = search_row {
            layout_col = layout_col.push(sr);
        }

        if let Some(ct) = category_title {
            layout_col = layout_col.push(ct);
        }

        let layout_col = layout_col
            .push(dual_pane)
            .push(sep)
            .push(bottom_bar);

        let layout = container(layout_col)
            .width(Length::Fixed(menu_width));

        self.core
            .applet
            .popup_container(layout)
            .limits(
                Limits::NONE
                    .min_width(menu_width)
                    .max_width(menu_width)
                    .min_height(1.0)
                    .max_height(menu_height),
            )
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                }
                self.search_field.clear();
                self.search_active = false;
                self.selected_category = Some(ApplicationCategory::all());
                self.selected_index = None;
                // Use cached apps from init — a background refresh task
                // keeps them up to date without blocking the UI.
                self.available_applications = self.all_applications.clone();
                self.rebuild_nav_model();

                let new_id = Id::unique();
                self.popup = Some(new_id);

                let popup_width = self.config.max_width() as u32;
                let popup_height = self.config.max_height() as u32;

                let mut popup_settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(), new_id,
                    Some((popup_width, popup_height)),
                    None, None,
                );
                let (anchor, gravity) = match self.core.applet.anchor {
                    PanelAnchor::Top => (Anchor::BottomLeft, Gravity::BottomRight),
                    PanelAnchor::Bottom => (Anchor::TopLeft, Gravity::TopRight),
                    PanelAnchor::Left => (Anchor::TopRight, Gravity::BottomRight),
                    PanelAnchor::Right => (Anchor::TopLeft, Gravity::BottomLeft),
                };
                popup_settings.positioner.anchor = anchor;
                popup_settings.positioner.gravity = gravity;
                popup_settings.positioner.size = Some((popup_width, popup_height));
                popup_settings.positioner.size_limits = Limits::NONE
                    .min_width(self.config.max_width()).min_height(380.0)
                    .max_width(self.config.max_width())
                    .max_height(self.config.max_height());

                get_popup(popup_settings)
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) { self.popup = None; }
                Task::none()
            }
            Message::ClosePopup => {
                if let Some(p) = self.popup.take() { return destroy_popup(p); }
                Task::none()
            }
            Message::SearchInput(input) => {
                // Guard: skip if input hasn't changed.
                if input == self.search_field {
                    return Task::none();
                }
                self.search_field = input.clone();
                self.selected_category = None;
                self.selected_index = None;
                self.nav_model.deactivate();
                self.available_applications = if input.is_empty() {
                    self.all_applications.clone()
                } else {
                    apps::filter_apps(&self.all_applications, &input)
                };
                Task::none()
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
                Task::none()
            }
            Message::ToggleSearch => {
                self.search_active = !self.search_active;
                if self.search_active {
                    self.search_field.clear();
                    self.selected_category = None;
                    self.selected_index = None;
                    self.nav_model.deactivate();
                    self.available_applications = self.all_applications.clone();
                    let focus_id = (*SEARCH_ID).clone();
                    return cosmic::widget::text_input::focus(focus_id);
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
                Task::none()
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
                        "favourites" => apps::filter_by_ids(&self.all_applications, &self.config.favourites),
                        "recents" => apps::filter_by_ids(&self.all_applications, &self.config.recents),
                        _ => apps::filter_apps_by_category(&self.all_applications, &cat),
                    };
                }
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
            Message::ToggleLayout => {
                self.config.use_grid = !self.config.use_grid;
                self.config.save();
                self.show_settings = false;
                Task::none()
            }
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                Task::none()
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
                            self.available_applications =
                                apps::filter_by_ids(&self.all_applications, &self.config.favourites);
                        }
                    }
                }
                Task::none()
            }
            Message::PinToTray(index) => {
                if let Some(app) = self.available_applications.get(index) {
                    let app_id = app.id.clone();
                    self.pin_app_to_dock(&app_id);
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
                    return destroy_popup(p);
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

    /// Pin a desktop file to the COSMIC dock/tray.
    fn pin_app_to_dock(&self, desktop_id: &str) {
        let Ok(config) = cosmic::cosmic_config::Config::new("com.system76.CosmicDock", 1) else {
            tracing::warn!("Cannot access CosmicDock config");
            return;
        };
        let pinned: Vec<String> = config.get("pinned_apps").unwrap_or_default();
        if !pinned.iter().any(|p| p == desktop_id) {
            let mut new_pinned = pinned.clone();
            new_pinned.push(desktop_id.to_string());
            if let Err(e) = config.set("pinned_apps", &new_pinned) {
                tracing::warn!("Failed to pin app to dock: {e}");
            } else {
                tracing::info!("Pinned {desktop_id} to dock");
            }
        }
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
            return destroy_popup(p);
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
            cosmic::widget::text::body(&app.name).height(Length::Fixed(20.0)),
            Space::new().width(Length::Fixed(space_xxs as f32)),
            star,
        ].align_y(Alignment::Center).into()
    } else {
        cosmic::widget::text::body(&app.name).height(Length::Fixed(20.0)).into()
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
        .height(Length::Fixed(48.0 + (space_xxs as f32) * 2.0))
        .padding([space_xxs, space_s])
        .class(theme::Container::Card),
    )
    .on_press(Message::LaunchApp(index));

    let fav_label = if is_favourite { "Unfavourite" } else { "Favourite" };
    let ctx_menu = menu::items(
        &HashMap::new(),
        vec![
            menu::Item::Button(fav_label, None, AppContextAction::ToggleFav(index)),
            menu::Item::Button("Pin to Tray", None, AppContextAction::PinToTray(index)),
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

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        return name.to_string();
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
    format!("{}…", &name[..end])
}

fn main() -> cosmic::iced::Result {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "warn")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);
    cosmic::applet::run::<Applet>(())
}
