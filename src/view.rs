// SPDX-License-Identifier: GPL-3.0-only

//! Menu views and widgets.

use crate::app::{
    Applet, Message, APP_SCROLL_ID, CORNER_BADGE_ICON_SIZE, GRID_ICON_SIZE, LIST_ICON_SIZE,
    SEARCH_ID, SETTINGS_PANEL_WIDTH, SIDEBAR_WIDTH,
};
use crate::config::{LayoutMode, SizePreset};
use crate::power::PowerAction;
use crate::{apps, fl};
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{
    widget::{column, container, row, scrollable, stack, Space},
    Alignment, Color, Length, Limits,
};
use cosmic::theme;
use cosmic::widget::{
    button, dropdown, icon, mouse_area, nav_bar, nav_bar_toggle, search_input, settings,
    text_input, tooltip,
};
use cosmic::Element;
use apps::ApplicationEntry;
use std::borrow::Cow;
use std::sync::{Arc, LazyLock};

/// Panel icon names and display labels.
const ICON_OPTIONS: &[(&str, &str)] = &[
    ("com.system76.CosmicAppLibrary", "COSMIC App Library"),
    ("cosmic-logo", "COSMIC"),
    ("system76-logo", "System76"),
    ("kde-official", "KDE (official)"),
    ("kde-oxygen", "KDE Gear (Oxygen)"),
    ("kde-plasma", "KDE Plasma"),
    ("kubuntu", "Kubuntu"),
    ("kde-neon", "KDE neon"),
    ("kde-classic", "KDE 2 / Classic (legacy)"),
    ("gnome-logo", "GNOME"),
    ("application-menu-symbolic", "App Menu"),
    ("open-menu-symbolic", "Menu"),
    ("start-here-symbolic", "Start"),
    ("distributor-logo-debian", "Debian"),
    ("start-here-ubuntu", "Ubuntu"),
    ("distributor-logo-archlinux", "Arch Linux"),
    ("start-here-fedora", "Fedora"),
    ("distributor-logo-pop-os", "Pop!_OS"),
    ("distributor-logo-manjaro", "Manjaro"),
    ("distributor-logo-opensuse", "openSUSE"),
    ("distributor-logo-linux-mint", "Linux Mint"),
    ("rust-logo", "Rust"),
    ("applications-system-symbolic", "System"),
    ("applications-engineering-symbolic", "Dev"),
    ("applications-games-symbolic", "Games"),
    ("applications-graphics-symbolic", "Graphics"),
    ("applications-multimedia-symbolic", "Media"),
    ("applications-office-symbolic", "Office"),
    ("applications-science-symbolic", "Science"),
    ("applications-utilities-symbolic", "Utils"),
    ("computer-symbolic", "Computer"),
    ("system-run-symbolic", "Run"),
    ("system-search-symbolic", "Search"),
    ("preferences-system-symbolic", "Settings"),
    ("emblem-system-symbolic", "System"),
];

static ICON_OPTION_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    ICON_OPTIONS.iter().map(|(_, label)| (*label).to_string()).collect()
});

static ICON_OPTION_HANDLES: LazyLock<Vec<icon::Handle>> = LazyLock::new(|| {
    ICON_OPTIONS
        .iter()
        .map(|(name, _)| crate::icons::option_handle(name, false))
        .collect()
});

pub(crate) fn warm_icon_option_handles() {
    std::thread::spawn(|| {
        LazyLock::force(&ICON_OPTION_HANDLES);
    });
}

static SIZE_PRESET_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    SizePreset::ALL.iter().map(|preset| size_preset_label(*preset)).collect()
});

const BOTTOM_BAR_CONTENT_HEIGHT: f32 = 21.0;
const BOTTOM_BAR_MIN_HEIGHT: f32 = 44.0;
const GRID_MIN_BUTTON_WIDTH: f32 = 80.0;
const SCROLLBAR_WIDTH: f32 = 8.0;

fn uses_grid_layout(applet: &Applet) -> bool {
    applet.config.layout_mode == LayoutMode::Grid
        || (applet.config.layout_mode == LayoutMode::Hybrid
            && matches!(
                applet.selected_category.as_ref(),
                Some(category) if category.key == "favourites" || category.key == "recents"
            ))
}

fn app_content_width(menu_width: f32, sidebar_visible: bool, spacing: Spacing) -> f32 {
    let sidebar_width = if sidebar_visible {
        SIDEBAR_WIDTH + spacing.space_m as f32
    } else {
        0.0
    };
    (menu_width
        - spacing.space_xxs as f32 * 2.0
        - sidebar_width
        - spacing.space_m as f32
        - SCROLLBAR_WIDTH)
        .max(0.0)
}

pub(crate) fn navigation_columns(applet: &Applet) -> usize {
    let spacing = theme::active().cosmic().spacing;
    let menu_width = if applet.is_window_mode {
        applet.window_width
    } else {
        applet.config.max_width()
    };
    let sidebar_visible = !applet.sidebar_collapsed && menu_width >= 600.0;
    let content_width = app_content_width(menu_width, sidebar_visible, spacing);
    if uses_grid_layout(applet) {
        let fit = ((content_width / (GRID_MIN_BUTTON_WIDTH + spacing.space_s as f32)) as usize)
            .clamp(1, 8);
        applet.config.grid_columns.min(fit).max(1)
    } else {
        list_grid_metrics(
            spacing.space_xxs,
            spacing.space_s,
            content_width as usize,
        )
        .cols
        .max(1)
    }
}

impl Applet {
    pub(crate) fn build_menu_view(&self, is_popup: bool) -> Element<'_, Message> {
        let cosmic_theme = theme::active();
        let Spacing {
            space_xxs,
            space_xs,
            space_s,
            space_m,
            ..
        } = cosmic_theme.cosmic().spacing;
        let is_window = !is_popup;
        let (menu_width, menu_height) = if is_window {
            (self.window_width, self.window_height)
        } else {
            (self.config.max_width(), self.config.max_height())
        };
        let sidebar_visible = !self.sidebar_collapsed && menu_width >= 600.0;
        let show_bottom_bar_power = self.config.show_bottom_bar_power_actions;
        let show_bottom_bar_pinned =
            self.config.show_bottom_bar_pinned && !self.pinned_apps.is_empty();
        let show_bottom_bar = show_bottom_bar_pinned || show_bottom_bar_power;
        let bottom_bar_height = if show_bottom_bar {
            (BOTTOM_BAR_CONTENT_HEIGHT + space_xxs as f32 * 4.0).max(BOTTOM_BAR_MIN_HEIGHT)
        } else {
            0.0
        };
        let top_bar: Option<Element<'_, Message>> = if is_window {
            None
        } else {
            let sidebar_toggle: Element<'_, Message> = nav_bar_toggle()
                .on_toggle(Message::ToggleSidebar)
                .active(sidebar_visible)
                .into();

            let search_toggle_btn: Element<'_, Message> = button::custom(
                icon::from_name("system-search-symbolic")
                    .symbolic(true)
                    .size(18)
                    .icon(),
            )
            .on_press(Message::ToggleSearch)
            .class(if self.search_active {
                theme::Button::Suggested
            } else {
                theme::Button::AppletMenu
            })
            .into();

            let window_btn: Element<'_, Message> = button::custom(
                icon::from_name("window-new-symbolic")
                    .symbolic(true)
                    .size(18)
                    .icon(),
            )
            .on_press(Message::OpenWindow)
            .class(theme::Button::AppletMenu)
            .into();

            let config_btn: Element<'_, Message> = button::custom(
                icon::from_name("emblem-system-symbolic")
                    .symbolic(true)
                    .size(18)
                    .icon(),
            )
            .on_press(Message::ToggleSettings)
            .class(if self.show_settings {
                theme::Button::Suggested
            } else {
                theme::Button::AppletMenu
            })
            .into();

            Some(container(
                row![
                    sidebar_toggle,
                    search_toggle_btn,
                    window_btn,
                    Space::new().width(Length::Fill),
                    config_btn,
                ]
                .align_y(Alignment::Center)
                .spacing(space_s),
            )
            .width(Length::Fill)
            .into())
        };

        let search_row: Option<Element<'_, Message>> = if self.search_active {
            let search = search_input(fl!("search-placeholder"), &self.search_field)
                .id((*SEARCH_ID).clone())
                .on_input(Message::SearchInput)
                .on_clear(Message::SearchCleared)
                .width(Length::Fill)
                .padding([space_xxs, space_s]);
            Some(search.into())
        } else {
            None
        };

        let nav: Option<Element<'_, Message>> = sidebar_visible.then(|| {
            mouse_area(
                nav_bar(&self.nav_model, Message::CategoryActivated)
                    .into_container()
                    .width(Length::Fixed(SIDEBAR_WIDTH))
                    .height(Length::Fill),
            )
            .on_enter(Message::ClearAppHover)
            .into()
        });

        let fav_set = self.cached_fav_ids.borrow();
        let pinned_set = self.cached_pinned_ids.borrow();

        let use_grid = uses_grid_layout(self);
        let app_content_width = app_content_width(
            menu_width,
            sidebar_visible,
            cosmic_theme.cosmic().spacing,
        );
        let grid_icon_size = f32::from(GRID_ICON_SIZE);

        let app_area: Element<'_, Message> = if self.available_applications.is_empty() {
            container(cosmic::widget::text::body(fl!("no-applications")))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .height(Length::Fill)
                .width(Length::Fill)
                .into()
        } else if use_grid {
            let min_btn = GRID_MIN_BUTTON_WIDTH + space_s as f32;
            let grid_columns = self
                .config
                .grid_columns
                .min(((app_content_width / min_btn) as usize).clamp(1, 8));

            let cell_height = grid_icon_size + 44.0 + space_xxs as f32 * 2.0;
            let row_stride = cell_height + space_s as f32;
            let rows_total = self.available_applications.len().div_ceil(grid_columns);
            let content_h = rows_total as f32 * cell_height
                + (rows_total.saturating_sub(1)) as f32 * space_s as f32;

            let build_row = |row_idx: usize| -> Element<'_, Message> {
                let start = row_idx * grid_columns;
                let end = ((row_idx + 1) * grid_columns).min(self.available_applications.len());
                let mut buttons: Vec<Element<'_, Message>> = (start..end)
                    .map(|index| {
                        let app = &self.available_applications[index];
                        let icon = self.cached_icon(app, grid_icon_size);
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
                            if show_actions {
                                stack![
                                    inner,
                                    container(app_action_row(index, is_fav, is_pinned))
                                        .width(Length::Fill)
                                        .height(Length::Fill)
                                        .align_y(Alignment::Start)
                                        .padding(0),
                                ]
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .into()
                            } else {
                                inner.into()
                            }
                        };
                        let btn = button::custom(content)
                            .on_press(Message::LaunchApp(index))
                            .class(app_grid_card_class(is_selected))
                            .width(Length::Fill)
                            .height(Length::Fixed(cell_height));

                        mouse_area(btn)
                            .on_enter(Message::AppHovered(index))
                            .on_exit(Message::AppUnhovered)
                            .into()
                    })
                    .collect();

                let missing = grid_columns - buttons.len();
                for _ in 0..missing {
                    buttons.push(Space::new().width(Length::Fill).into());
                }
                row(buttons)
                    .spacing(space_s)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                    .into()
            };

            // Keep one row mounted above and below the viewport.
            let viewport_h = if self.app_viewport_height > 0.0 {
                self.app_viewport_height
            } else {
                menu_height
            };
            let scroll = self
                .app_scroll_y
                .clamp(0.0, (content_h - viewport_h).max(0.0));
            let mut first_row = if scroll <= cell_height {
                0
            } else {
                ((scroll - cell_height) / row_stride).floor() as usize + 1
            };
            first_row = first_row.saturating_sub(1);
            let mut last_row = ((scroll + viewport_h) / row_stride).ceil() as usize;
            last_row = last_row
                .saturating_sub(1)
                .min(rows_total.saturating_sub(1));
            last_row = (last_row + 1).min(rows_total.saturating_sub(1));
            let mut rows: Vec<Element<'_, Message>> =
                Vec::with_capacity(last_row - first_row + 3);
            if first_row > 0 {
                let top_h = (first_row as f32 * row_stride - space_s as f32).max(0.0);
                rows.push(Space::new().height(Length::Fixed(top_h)).into());
            }
            for row_idx in first_row..=last_row {
                rows.push(build_row(row_idx));
            }
            let bottom_h =
                content_h - (last_row as f32 * row_stride + cell_height) - space_s as f32;
            if bottom_h > 0.0 {
                rows.push(Space::new().height(Length::Fixed(bottom_h)).into());
            }

            let app_grid = container(column(rows).spacing(space_s))
                .padding([0, space_m.saturating_add(SCROLLBAR_WIDTH as u16), 0, 0])
                .width(Length::Fill);
            let scroller = scrollable(app_grid)
                .id((*APP_SCROLL_ID).clone())
                .height(Length::Fill)
                .on_scroll(|vp| {
                    Message::AppsScrolled(vp.absolute_offset().y, vp.bounds().height)
                });
            container(scroller)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            let list_width = app_content_width as usize;

            let GridMetrics {
                cols,
                item_width,
                column_spacing,
            } = list_grid_metrics(space_xxs, space_s, list_width);

            let apps = &self.available_applications;
            let total_rows = if apps.is_empty() || cols == 0 {
                0
            } else {
                (apps.len() + cols - 1) / cols
            };
            let card_height = LIST_ICON_SIZE as f32 + (space_xxs as f32) * 2.0;
            let row_stride = card_height + column_spacing as f32;
            let content_h = if total_rows == 0 {
                0.0
            } else {
                total_rows as f32 * row_stride - column_spacing as f32
            };

            let viewport_h = if self.app_viewport_height > 0.0 {
                self.app_viewport_height
            } else {
                menu_height
            };
            let scroll = self
                .app_scroll_y
                .clamp(0.0, (content_h - viewport_h).max(0.0));
            let first_row = if scroll <= row_stride {
                0usize
            } else {
                ((scroll / row_stride) as usize).saturating_sub(1)
            };
            let last_row = ((scroll + viewport_h) / row_stride).ceil() as usize;
            let last_row = last_row
                .min(total_rows.saturating_sub(1))
                .saturating_add(1)
                .min(total_rows.saturating_sub(1));
            let text_width = item_width.saturating_sub(
                LIST_ICON_SIZE as usize
                    + space_s as usize * 3
                    + ACTION_ROW_HEIGHT as usize * 2,
            ) as f32;
            let card_layout = ListCardLayout {
                space_xxs,
                space_s,
                text_width,
                width: item_width,
            };

            let mut rows: Vec<Element<'_, Message>> =
                Vec::with_capacity(last_row.saturating_sub(first_row) + 3);
            if first_row > 0 {
                let top_h = (first_row as f32 * row_stride - column_spacing as f32).max(0.0);
                rows.push(Space::new().height(Length::Fixed(top_h)).into());
            }
            for row_idx in first_row..=last_row {
                let start = row_idx * cols;
                let end = (start + cols).min(apps.len());
                let mut row_children: Vec<Element<'_, Message>> = Vec::with_capacity(cols);
                for index in start..end {
                    let app = &apps[index];
                    let is_fav = fav_set.contains(app.id.as_str());
                    let is_pinned = pinned_set.contains(app.id.as_str());
                    let is_selected = self.selected_index == Some(index);
                    let show_actions = self.hovered_app_index == Some(index);
                    let icon = self.cached_icon(app, LIST_ICON_SIZE as f32);
                    row_children.push(app_list_card(
                        app,
                        icon,
                        card_layout,
                        AppCardState {
                            index,
                            is_favourite: is_fav,
                            is_pinned,
                            is_selected,
                            show_actions,
                        },
                    ));
                }
                let missing = cols.saturating_sub(row_children.len());
                for _ in 0..missing {
                    row_children
                        .push(Space::new().width(Length::Fixed(item_width as f32)).into());
                }
                rows.push(
                    row(row_children)
                        .spacing(column_spacing)
                        .width(Length::Fill)
                        .into(),
                );
            }
            if last_row < total_rows.saturating_sub(1) {
                let bottom_h = content_h
                    - (last_row as f32 * row_stride + card_height)
                    - column_spacing as f32;
                if bottom_h > 0.0 {
                    rows.push(Space::new().height(Length::Fixed(bottom_h)).into());
                }
            }

            let list_content = container(column(rows).spacing(column_spacing))
                .padding([0, space_m.saturating_add(SCROLLBAR_WIDTH as u16), 0, 0])
                .width(Length::Fill);

            let scroller = scrollable(list_content)
                .id((*APP_SCROLL_ID).clone())
                .height(Length::Fill)
                .on_scroll(|vp| {
                    Message::AppsScrolled(vp.absolute_offset().y, vp.bounds().height)
                });
            container(scroller)
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

        let settings_panel: Option<Element<'_, Message>> = if self.show_settings {
            let layout_radios: Element<'_, Message> = {
                let selected = self.config.layout_mode;
                let choice = |mode: LayoutMode, label: String| {
                    cosmic::widget::radio(
                        cosmic::widget::text::body(label),
                        mode,
                        Some(selected),
                        Message::LayoutMode,
                    )
                };
                row![
                    choice(LayoutMode::Grid, fl!("grid-view")),
                    choice(LayoutMode::List, fl!("list-view")),
                    choice(LayoutMode::Hybrid, fl!("hybrid-view")),
                ]
                .spacing(space_m)
                .align_y(Alignment::Center)
                .into()
            };

            let size_labels = Cow::Borrowed(SIZE_PRESET_LABELS.as_slice());
            let selected_size = {
                let preset = if self.custom_size_selected {
                    SizePreset::Custom
                } else {
                    self.config.size_preset
                };
                SizePreset::ALL
                    .iter()
                    .position(|candidate| *candidate == preset)
            };
            let size_picker: Element<'_, Message> =
                dropdown(size_labels, selected_size, move |idx| {
                    Message::SetSizePreset(SizePreset::ALL[idx])
                })
                .width(Length::Fill)
                .padding([space_xxs, space_xxs])
                .into();

            let custom_size_row: Option<Element<'_, Message>> =
                (self.custom_size_selected || self.config.size_preset == SizePreset::Custom)
                    .then(|| {
                        column![
                            row![
                                text_input(fl!("width-px"), &self.custom_width_input)
                                    .on_input(Message::SetCustomWidth)
                                    .width(Length::Fill)
                                    .padding([space_xxs, space_xxs]),
                                Space::new().width(Length::Fixed(space_xxs as f32)),
                                text_input(fl!("height-px"), &self.custom_height_input)
                                    .on_input(Message::SetCustomHeight)
                                    .width(Length::Fill)
                                    .padding([space_xxs, space_xxs]),
                            ]
                            .spacing(space_xxs)
                            .width(Length::Fill),
                            button::standard(fl!("apply"))
                                .on_press(Message::ApplyCustomSize)
                                .width(Length::Fill)
                        ]
                        .spacing(space_xxs)
                        .width(Length::Fill)
                        .into()
                    });

            let current_icon = match self.config.panel_icon.as_str() {
                "" => "cosmic-logo",
                "kde" => "kde-official",
                name => name,
            };
            let selected_idx = ICON_OPTIONS
                .iter()
                .position(|(name, _)| *name == current_icon);
            let picker: Element<'_, Message> = dropdown(
                ICON_OPTION_LABELS.as_slice(),
                selected_idx,
                move |idx: usize| {
                    let icon_name = ICON_OPTIONS[idx].0.to_string();
                    Message::SetPanelIcon(icon_name)
                },
            )
            .icons(Cow::Borrowed(ICON_OPTION_HANDLES.as_slice()))
            .width(Length::Fill)
            .padding([space_xxs, space_s])
            .into();

            let mut default_keys: Vec<String> = Vec::new();
            let mut default_labels: Vec<String> = Vec::new();
            default_keys.push("all".to_string());
            default_labels.push(fl!("all-applications"));
            if self.config.show_favourites && !self.config.favourites.is_empty() {
                default_keys.push("favourites".to_string());
                default_labels.push(fl!("favourites"));
            }
            if self.config.show_recents && !self.config.recents.is_empty() {
                default_keys.push("recents".to_string());
                default_labels.push(fl!("recents"));
            }
            for cat in &self.available_categories {
                default_keys.push(cat.key.clone());
                default_labels.push(cat.display_name.clone());
            }
            let default_selected = default_keys
                .iter()
                .position(|key| *key == self.config.default_category);
            let default_picker: Element<'_, Message> = dropdown(
                default_labels,
                default_selected,
                move |idx| Message::SetDefaultCategory(default_keys[idx].clone()),
            )
            .width(Length::Fill)
            .into();

            let content = settings::view_column(vec![
                settings::section()
                    .title(fl!("appearance"))
                    .add(stacked_item(
                        fl!("layout-mode"),
                        layout_radios,
                        space_xxs
                    ))
                    .add(stacked_item(fl!("menu-size"), size_picker, space_xxs))
                    .add_maybe(
                        custom_size_row
                            .map(|row| stacked_item(fl!("custom-size"), row, space_xxs))
                    )
                    .add(stacked_item(fl!("panel-icon"), picker, space_xxs))
                    .add(
                        settings::item::builder(fl!("monochrome-icon")).toggler(
                            self.config.panel_icon_symbolic,
                            |_| Message::TogglePanelIconSymbolic,
                        ),
                    )
                    .into(),
                settings::section()
                    .title(fl!("sidebar"))
                    .add(settings::item::builder(fl!("show-favourites")).toggler(
                        self.config.show_favourites,
                        |_| Message::ToggleShowFavourites,
                    ))
                    .add(settings::item::builder(fl!("show-recents")).toggler(
                        self.config.show_recents,
                        |_| Message::ToggleShowRecents,
                    ))
                    .add(
                        settings::item::builder(fl!("hide-sidebar-by-default")).toggler(
                            self.config.sidebar_collapsed,
                            |_| Message::ToggleSidebarDefault,
                        ),
                    )
                    .into(),
                settings::section()
                    .title(fl!("bottom-bar"))
                    .add(settings::item::builder(fl!("show-pinned-apps")).toggler(
                        self.config.show_bottom_bar_pinned,
                        |_| Message::ToggleShowBottomBarPinned,
                    ))
                    .add(settings::item::builder(fl!("show-power-actions")).toggler(
                        self.config.show_bottom_bar_power_actions,
                        |_| Message::ToggleShowBottomBarPowerActions,
                    ))
                    .into(),
                settings::section()
                    .title(fl!("default-menu"))
                    .add(stacked_item(
                        fl!("default-category"),
                        default_picker,
                        space_xxs
                    ))
                    .into(),
            ]);

            let settings_header = row![
                cosmic::widget::text::heading(fl!("settings-title")),
                Space::new().width(Length::Fill),
                button::text(fl!("close"))
                    .trailing_icon(icon::from_name("go-next-symbolic"))
                    .on_press(Message::ToggleSettings)
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill);

            Some(
                container(
                    column![
                        settings_header,
                        scrollable(content)
                            .height(Length::Fill)
                            .width(Length::Fill),
                    ]
                    .spacing(space_m)
                    .width(Length::Fill)
                    .height(Length::Fill),
                )
                    .width(Length::Fixed(SETTINGS_PANEL_WIDTH))
                    .height(Length::Fill)
                    .padding([space_s, space_s])
                    .class(theme::Container::Primary)
                    .into(),
            )
        } else {
            None
        };

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
                            cosmic::widget::divider::vertical::default().height(Length::Fill),
                        )
                        .height(Length::Fixed(bottom_bar_height - space_xxs as f32 * 2.0))
                        .padding([0, space_xxs]),
                    );
                }
            }

            if show_bottom_bar_power {
                for action in PowerAction::BOTTOM_BAR {
                    bottom_row = bottom_row.push(bottom_bar_action_button(
                        icon::from_name(action.icon_name())
                            .symbolic(true)
                            .size(20)
                            .icon(),
                        Cow::Owned(action.label()),
                        Message::PowerAction(action),
                        false,
                        menu_too_small,
                        space_xxs,
                        space_xs,
                    ));
                }
            }

            container(bottom_row.padding(space_xxs))
                .height(Length::Fixed(bottom_bar_height))
                .width(Length::Fill)
                .class(theme::Container::Primary)
                .into()
        } else {
            Space::new()
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into()
        };

        let mut dual_pane = row![]
            .spacing(space_m)
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(nav) = nav {
            dual_pane = dual_pane.push(nav);
        }
        dual_pane = dual_pane.push(app_area);

        let dual_pane: Element<'_, Message> = if let Some(settings) = settings_panel {
            stack![
                dual_pane,
                container(settings)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::End)
                    .align_y(Alignment::Start),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            dual_pane.into()
        };

        let category_title: Option<Element<'_, Message>> = if !sidebar_visible {
            self.selected_category.as_ref().map(|cat| {
                let icon = icon::from_name(Arc::from(cat.icon_name.as_str()))
                    .symbolic(true)
                    .size(20)
                    .icon();
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

        let mut main_col = column![]
            .spacing(space_xxs)
            .padding([space_xxs, space_xxs, space_xxs, space_xxs])
            .width(Length::Fill);

        if let Some(top_bar) = top_bar {
            main_col = main_col.push(top_bar);
        }
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

        if is_popup {
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
            container(main_col)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([space_xs, space_xs, 0, space_xs])
                .style(|theme| {
                    let cosmic = theme.cosmic();
                    let bg = cosmic.background(true).base;
                    cosmic::iced::widget::container::Style {
                        background: Some(Color::from(bg).into()),
                        text_color: Some(cosmic.background(true).on.into()),
                        icon_color: Some(cosmic.background(true).on.into()),
                        ..Default::default()
                    }
                })
                .into()
        }
    }
}

fn stacked_item<'a>(
    title: impl Into<Cow<'a, str>> + 'a,
    widget: impl Into<Element<'a, Message>>,
    spacing: u16,
) -> Element<'a, Message> {
    column![
        cosmic::widget::text::body(title),
        widget.into(),
    ]
    .spacing(spacing)
    .width(Length::Fill)
    .into()
}

fn size_preset_label(preset: SizePreset) -> String {
    let label = match preset {
        SizePreset::Small => fl!("size-small"),
        SizePreset::Medium => fl!("size-medium"),
        SizePreset::MediumSquare => fl!("size-medium-square"),
        SizePreset::Large => fl!("size-large"),
        SizePreset::Tall => fl!("size-tall"),
        SizePreset::Square => fl!("size-square"),
        SizePreset::Portrait => fl!("size-portrait"),
        SizePreset::Custom => fl!("size-custom"),
    };
    if preset == SizePreset::Custom {
        label
    } else {
        fl!(
            "size-preset",
            preset = label,
            width = (preset.width() as u32).to_string(),
            height = (preset.height() as u32).to_string()
        )
    }
}

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

fn bottom_bar_icon_fallback_name(app_id: &str) -> Cow<'static, str> {
    match app_id {
        crate::app::COSMIC_FILES_APP_ID => Cow::Borrowed("com.system76.CosmicFiles"),
        crate::app::COSMIC_SETTINGS_APP_ID => Cow::Borrowed("com.system76.CosmicSettings"),
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
    icon::from_name(fallback)
        .symbolic(false)
        .prefer_svg(true)
        .size(size)
        .fallback(Some(icon::IconFallback::Names(vec![
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

    mouse_area(launch_btn)
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
    let btn = button::custom(if icon_only || menu_too_small {
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
    })
    .on_press(message)
    .class(theme::Button::AppletMenu)
    .width(Length::Fill)
    .padding([space_xxs, space_xs]);

    if menu_too_small {
        tooltip(
            btn,
            cosmic::widget::text::body(label),
            cosmic::widget::tooltip::Position::Top,
        )
        .into()
    } else {
        btn.into()
    }
}

const ACTION_ROW_HEIGHT: f32 = 24.0;

fn action_icon(name: &str, is_active: bool) -> cosmic::widget::icon::Icon {
    let icon = icon::from_name(name)
        .symbolic(true)
        .prefer_svg(true)
        .size(CORNER_BADGE_ICON_SIZE)
        .icon();

    icon.class(theme::Svg::Custom(std::rc::Rc::new(move |theme| {
        let color = if is_active {
            theme.cosmic().accent_color()
        } else {
            theme.cosmic().background(true).on
        };
        cosmic::iced::widget::svg::Style {
            color: Some(color.into()),
        }
    })))
}

fn corner_fav_icon(is_favourite: bool) -> cosmic::widget::icon::Icon {
    action_icon("starred-symbolic", is_favourite)
}

fn corner_pin_icon(is_pinned: bool) -> cosmic::widget::icon::Icon {
    let (name, fallbacks): (&str, &[&str]) = if is_pinned {
        ("window-pin-symbolic", &["pin-symbolic", "xapp-pin-symbolic"])
    } else {
        ("view-pin-symbolic", &["pin-symbolic", "xapp-pin-symbolic"])
    };
    let icon = icon::from_name(name)
        .symbolic(true)
        .prefer_svg(true)
        .size(CORNER_BADGE_ICON_SIZE)
        .fallback(Some(icon::IconFallback::Names(
            fallbacks.iter().map(|s| Cow::from(*s)).collect(),
        )))
        .icon();

    icon.class(theme::Svg::Custom(std::rc::Rc::new(move |theme| {
        let color = if is_pinned {
            theme.cosmic().accent_color()
        } else {
            theme.cosmic().background(true).on
        };
        cosmic::iced::widget::svg::Style {
            color: Some(color.into()),
        }
    })))
}

fn corner_icon_button(
    icon: cosmic::widget::icon::Icon,
    message: Message,
    tooltip_text: String,
) -> Element<'static, Message> {
    let button = button::custom(icon)
        .on_press(message)
        .class(theme::Button::Icon)
        .padding(2)
        .width(Length::Fixed(ACTION_ROW_HEIGHT))
        .height(Length::Fixed(ACTION_ROW_HEIGHT));
    tooltip(
        button,
        cosmic::widget::text::body(tooltip_text),
        cosmic::widget::tooltip::Position::Top,
    )
    .into()
}

fn app_pin_action_button(index: usize, is_pinned: bool) -> Element<'static, Message> {
    let (pin_msg, tooltip_text) = if is_pinned {
        (Message::UnpinFromTray(index), fl!("unpin-from-tray"))
    } else {
        (Message::PinToTray(index), fl!("pin-to-tray"))
    };
    corner_icon_button(corner_pin_icon(is_pinned), pin_msg, tooltip_text)
}

fn app_fav_action_button(index: usize, is_favourite: bool) -> Element<'static, Message> {
    let tooltip_text = if is_favourite {
        fl!("remove-from-favourites")
    } else {
        fl!("add-to-favourites")
    };
    corner_icon_button(
        corner_fav_icon(is_favourite),
        Message::ToggleFavourite(index),
        tooltip_text,
    )
}

fn app_action_row(
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
) -> Element<'static, Message> {
    row![
        app_pin_action_button(index, is_pinned),
        Space::new().width(Length::Fill),
        app_fav_action_button(index, is_favourite),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(ACTION_ROW_HEIGHT))
    .align_y(Alignment::Center)
    .into()
}

fn app_action_buttons(
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
) -> Element<'static, Message> {
    row![
        app_pin_action_button(index, is_pinned),
        app_fav_action_button(index, is_favourite),
    ]
    .spacing(0)
    .into()
}

fn app_grid_card_class(selected: bool) -> theme::Button {
    app_card_class(selected, true)
}

fn app_list_card_class(selected: bool) -> theme::Button {
    app_card_class(selected, false)
}

fn app_card_class(selected: bool, transparent_idle: bool) -> theme::Button {
    use cosmic::iced::{Background, Color};

    let idle = move |theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut style = cosmic::widget::button::Style::new();
        if !transparent_idle {
            style.background = Some(Background::Color(component.base.into()));
        }
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
        let mut style = cosmic::widget::button::Style::new();
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
        hovered: Box::new(move |_focused, theme| highlight(theme, 0.3)),
        pressed: Box::new(move |_focused, theme| highlight(theme, 0.25)),
    }
}

#[derive(Clone, Copy)]
struct ListCardLayout {
    space_xxs: u16,
    space_s: u16,
    text_width: f32,
    width: usize,
}

#[derive(Clone, Copy)]
struct AppCardState {
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
    is_selected: bool,
    show_actions: bool,
}

fn app_list_card<'a>(
    app: &'a ApplicationEntry,
    icon: cosmic::widget::icon::Icon,
    layout: ListCardLayout,
    state: AppCardState,
) -> Element<'a, Message> {
    let summary = app
        .description
        .as_deref()
        .map(|d| truncate_name(d, 60))
        .unwrap_or_default();

    let effective_text_width = layout.text_width.max(40.0);

    let name_row: Element<'_, Message> = cosmic::widget::text::body(&app.name)
        .height(Length::Fixed(20.0))
        .width(Length::Fixed(effective_text_width))
        .wrapping(cosmic::iced::widget::text::Wrapping::Word)
        .into();

    let card_height = LIST_ICON_SIZE as f32 + (layout.space_xxs as f32) * 2.0;
    let card_width = layout.width as f32;

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
    .spacing(layout.space_s)
    .width(Length::Fill);

    let card_content: Element<'a, Message> = if state.show_actions {
        stack![
            card_body,
            container(app_action_buttons(
                state.index,
                state.is_favourite,
                state.is_pinned,
            ))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .padding(0),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        card_body.into()
    };

    let btn = button::custom(card_content)
        .on_press(Message::LaunchApp(state.index))
        .padding([layout.space_xxs, layout.space_s])
        .width(Length::Fixed(card_width))
        .height(Length::Fixed(card_height))
        .class(app_list_card_class(state.is_selected));

    mouse_area(btn)
        .on_enter(Message::AppHovered(state.index))
        .on_exit(Message::AppUnhovered)
        .into()
}

pub(crate) fn app_icon(app: &ApplicationEntry, size: f32) -> cosmic::widget::icon::Icon {
    let size_u16 = size as u16;
    if let Some(ref name) = app.icon {
        let name: Arc<str> = Arc::from(name.as_str());
        icon::from_name(name)
            .symbolic(false)
            .prefer_svg(true)
            .size(size_u16)
            .fallback(Some(icon::IconFallback::Names(vec![
                "application-x-executable".into(),
                "application-default".into(),
            ])))
            .icon()
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
    } else {
        icon::from_name("application-x-executable")
            .symbolic(false)
            .prefer_svg(true)
            .size(size_u16)
            .icon()
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
    }
}

fn truncate_name<'a>(name: &'a str, max_chars: usize) -> Cow<'a, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }
    let mut indices = name.char_indices();
    let Some((end, _)) = indices.nth(max_chars - 1) else {
        return Cow::Borrowed(name);
    };
    if indices.next().is_none() {
        return Cow::Borrowed(name);
    }
    Cow::Owned(format!("{}…", &name[..end]))
}

#[cfg(test)]
mod tests {
    use super::truncate_name;

    #[test]
    fn truncates_at_character_boundaries() {
        assert_eq!(truncate_name("short", 8), "short");
        assert_eq!(truncate_name("exact", 5), "exact");
        assert_eq!(truncate_name("aéioux", 5), "aéio…");
    }
}
