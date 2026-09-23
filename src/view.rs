// SPDX-License-Identifier: GPL-3.0-only

//! Menu views and widgets.

use crate::app::{
    Applet, Message, APP_SCROLL_ID, CORNER_BADGE_ICON_SIZE, GRID_ICON_SIZE, LIST_ICON_SIZE,
    SEARCH_ID, SETTINGS_PANEL_WIDTH, SIDEBAR_WIDTH,
};
use crate::config::{LayoutMode, SizePreset};
use crate::power::PowerAction;
use crate::{apps, fl, panel_icons};
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{
    widget::{column, container, lazy, row, stack, Space},
    Alignment, Color, Length, Limits,
};
use cosmic::theme;
use cosmic::widget::{
    button, dropdown, icon, mouse_area, nav_bar, nav_bar_toggle, scrollable, search_input,
    settings, text_input, tooltip,
};
use cosmic::Element;
use apps::ApplicationEntry;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// Panel icon names and display labels.
const ICON_OPTIONS: &[(&str, &str)] = &[
    (
        "com.github.cosmic-kickoff-launcher",
        "Kickoff Launcher",
    ),
    (panel_icons::COSMIC_APP_LIBRARY, "COSMIC App Library"),
    (panel_icons::COSMIC_LOGO, "COSMIC Logo"),
    (panel_icons::SYSTEM76_LOGO, "System76"),
    (panel_icons::MOBILE_DOTS, "Mobile Dots"),
    (panel_icons::APP_DRAWER, "App Drawer"),
    (panel_icons::BENTO, "Bento"),
    (panel_icons::HONEYCOMB, "Honeycomb"),
    (panel_icons::XFCE_APPLICATIONS_MENU, "Xfce Applications"),
    (panel_icons::HAIKU_DESKBAR, "Haiku Deskbar"),
    (panel_icons::KDE2_KICKER, "KDE 2 (Kicker)"),
    (panel_icons::KDE3_CRYSTAL, "KDE 3 (Crystal)"),
    (panel_icons::KDE_CLASSIC, "KDE Classic (legacy)"),
    (panel_icons::KDE_BREEZE, "KDE Plasma (Breeze)"),
    (panel_icons::KDE_OXYGEN, "KDE Menu (Oxygen)"),
    (panel_icons::DEBIAN, "Debian"),
    ("application-menu-symbolic", "App Menu"),
    ("open-menu-symbolic", "Menu"),
    ("start-here-symbolic", "Start"),
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
        .map(|(name, _)| {
            panel_icons::handle(name, false)
                .unwrap_or_else(|| icon::from_name(*name).symbolic(false).handle())
        })
        .collect()
});

pub(crate) fn warm_icon_option_handles() {
    LazyLock::force(&ICON_OPTION_HANDLES);
}

static SIZE_PRESET_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    SizePreset::ALL.iter().map(|preset| size_preset_label(*preset)).collect()
});

const BOTTOM_BAR_CONTENT_HEIGHT: f32 = 21.0;
const BOTTOM_BAR_MIN_HEIGHT: f32 = 44.0;
const GRID_MIN_BUTTON_WIDTH: f32 = 80.0;
const SCROLLBAR_WIDTH: f32 = 8.0;
const ROW_OVERSCAN: usize = 1;

pub(crate) type AppIconHandles = HashMap<(Option<Arc<str>>, u16), cosmic::widget::icon::Handle>;

pub(crate) struct VisibleRowCache {
    generation: u64,
    first_row: usize,
    last_row: usize,
    columns: usize,
    rows: Arc<Vec<Arc<[Arc<ApplicationEntry>]>>>,
}

pub(crate) fn resolve_icon_handles(apps: &[Arc<ApplicationEntry>]) -> AppIconHandles {
    let mut handles = HashMap::new();
    for &size in [crate::app::GRID_ICON_SIZE, crate::app::LIST_ICON_SIZE, crate::app::BAR_ICON_SIZE]
        .iter()
    {
        handles.insert((None, size), app_icon_handle(None, size));
    }
    for app in apps {
        let Some(name) = app.icon.clone() else {
            continue;
        };
        for &size in
            [crate::app::GRID_ICON_SIZE, crate::app::LIST_ICON_SIZE, crate::app::BAR_ICON_SIZE]
                .iter()
        {
            let key = (Some(Arc::clone(&name)), size);
            if handles.contains_key(&key) {
                continue;
            }
            handles.insert(key, app_icon_handle(Some(name.as_ref()), size));
        }
    }
    handles
}

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

        let use_grid = uses_grid_layout(self);
        let app_content_width = app_content_width(
            menu_width,
            sidebar_visible,
            cosmic_theme.cosmic().spacing,
        );

        let app_area: Element<'_, Message> = if self.available_applications.is_empty() {
            container(cosmic::widget::text::body(fl!("no-applications")))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .height(Length::Fill)
                .width(Length::Fill)
                .into()
        } else {
            virtualized_apps(
                self,
                menu_height,
                app_content_width,
                use_grid,
                space_xxs,
                space_s,
                space_m,
            )
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

            let current_icon = if self.config.panel_icon.is_empty() {
                "com.github.cosmic-kickoff-launcher"
            } else {
                self.config.panel_icon.as_str()
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
                    let app = self.apps_by_id.get(pinned_id);
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
                    let icon = app
                        .map(|app| icon_widget(&self.icon_handles, app, crate::app::BAR_ICON_SIZE))
                        .unwrap_or_else(|| {
                            bottom_bar_fallback_icon(pinned_id, crate::app::BAR_ICON_SIZE)
                        });
                    let hovered = self.hovered_pinned_id.as_deref() == Some(pinned_id.as_str());
                    bottom_row = bottom_row.push(bottom_bar_pinned_item(
                        icon,
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
    column_spacing: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RowKey {
    generation: u64,
    row: usize,
    columns: usize,
    selected: Option<usize>,
    hovered: Option<usize>,
    flags: u64,
    grid: bool,
    space_xxs: u16,
    space_s: u16,
}

struct PageLayout {
    columns: usize,
    card_height: f32,
    row_gap: f32,
    gap: u16,
    row_stride: f32,
    first_row: usize,
    last_row: usize,
    icon_size: u16,
    grid: bool,
}

fn page_layout(
    applet: &Applet,
    menu_height: f32,
    content_width: f32,
    grid: bool,
    space_xxs: u16,
    space_s: u16,
) -> Option<PageLayout> {
    let count = applet.available_applications.len();
    if count == 0 || content_width <= 1.0 {
        return None;
    }
    let (columns, gap, card_height, icon_size) = if grid {
        let min_btn = GRID_MIN_BUTTON_WIDTH + space_s as f32;
        let columns = applet
            .config
            .grid_columns
            .min(((content_width / min_btn) as usize).clamp(1, 8))
            .max(1);
        let card_height = f32::from(GRID_ICON_SIZE) + 44.0 + space_xxs as f32 * 2.0;
        (columns, space_s, card_height, GRID_ICON_SIZE)
    } else {
        let metrics = list_grid_metrics(space_xxs, space_s, content_width as usize);
        let card_height = f32::from(LIST_ICON_SIZE) + space_xxs as f32 * 2.0;
        (
            metrics.cols.max(1),
            metrics.column_spacing,
            card_height,
            LIST_ICON_SIZE,
        )
    };
    let row_gap = f32::from(gap);
    let row_stride = card_height + row_gap;
    let row_count = count.div_ceil(columns);
    let viewport = if applet.app_viewport_height > 1.0 {
        applet.app_viewport_height
    } else {
        menu_height
    };
    let (first_row, last_row) =
        visible_row_range(applet.app_scroll_y, viewport, row_stride, row_count);
    Some(PageLayout {
        columns,
        card_height,
        row_gap,
        gap,
        row_stride,
        first_row,
        last_row,
        icon_size,
        grid,
    })
}

pub(crate) fn index_in_visible_rows(applet: &Applet, index: usize) -> bool {
    if index >= applet.available_applications.len() {
        return false;
    }
    let spacing = theme::active().cosmic().spacing;
    let menu_width = if applet.is_window_mode {
        applet.window_width
    } else {
        applet.config.max_width()
    };
    let menu_height = if applet.is_window_mode {
        applet.window_height
    } else {
        applet.config.max_height()
    };
    let sidebar_visible = !applet.sidebar_collapsed && menu_width >= 600.0;
    let content_width = app_content_width(menu_width, sidebar_visible, spacing);
    let Some(layout) = page_layout(
        applet,
        menu_height,
        content_width,
        uses_grid_layout(applet),
        spacing.space_xxs,
        spacing.space_s,
    ) else {
        return false;
    };
    let row = index / layout.columns;
    (layout.first_row..=layout.last_row).contains(&row)
}

pub(crate) fn scroll_offset_for_index(applet: &Applet, index: usize) -> f32 {
    let spacing = theme::active().cosmic().spacing;
    let menu_width = if applet.is_window_mode {
        applet.window_width
    } else {
        applet.config.max_width()
    };
    let menu_height = if applet.is_window_mode {
        applet.window_height
    } else {
        applet.config.max_height()
    };
    let sidebar_visible = !applet.sidebar_collapsed && menu_width >= 600.0;
    let content_width = app_content_width(menu_width, sidebar_visible, spacing);
    let Some(layout) = page_layout(
        applet,
        menu_height,
        content_width,
        uses_grid_layout(applet),
        spacing.space_xxs,
        spacing.space_s,
    ) else {
        return 0.0;
    };
    let row_count = applet
        .available_applications
        .len()
        .div_ceil(layout.columns.max(1));
    if row_count <= 1 {
        return 0.0;
    }
    let content_h = content_height(row_count, layout.card_height, layout.row_gap);
    let viewport = if applet.app_viewport_height > 1.0 {
        applet.app_viewport_height
    } else {
        menu_height
    };
    let max_scroll = (content_h - viewport).max(0.0);
    let row = index / layout.columns.max(1);
    let relative = row as f32 / (row_count - 1) as f32;
    relative * max_scroll
}

fn visible_row_range(
    scroll: f32,
    viewport: f32,
    row_stride: f32,
    row_count: usize,
) -> (usize, usize) {
    crate::vscroll::row_window(
        scroll,
        viewport,
        row_stride,
        row_count,
        ROW_OVERSCAN,
    )
}

fn content_height(row_count: usize, card_height: f32, row_gap: f32) -> f32 {
    if row_count == 0 {
        0.0
    } else {
        row_count as f32 * card_height + row_count.saturating_sub(1) as f32 * row_gap
    }
}

fn virtualized_apps<'a>(
    applet: &'a Applet,
    menu_height: f32,
    content_width: f32,
    grid: bool,
    space_xxs: u16,
    space_s: u16,
    space_m: u16,
) -> Element<'a, Message> {
    let Some(layout) = page_layout(
        applet,
        menu_height,
        content_width,
        grid,
        space_xxs,
        space_s,
    ) else {
        return container(cosmic::widget::text::body(fl!("no-applications")))
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    };

    let apps = &applet.available_applications;
    let row_count = apps.len().div_ceil(layout.columns);
    let content_h = content_height(row_count, layout.card_height, layout.row_gap);
    let mut rows: Vec<Element<'static, Message>> =
        Vec::with_capacity(layout.last_row - layout.first_row + 3);
    if layout.first_row > 0 {
        let top_h = (layout.first_row as f32 * layout.row_stride - layout.row_gap).max(0.0);
        rows.push(Space::new().height(Length::Fixed(top_h)).into());
    }

    let icons = Arc::clone(&applet.icon_handles);
    let row_apps = cached_visible_rows(applet, &layout);
    for (offset, row_apps) in row_apps.iter().cloned().enumerate() {
        let row_index = layout.first_row + offset;
        let start = row_index * layout.columns;
        let end = start + row_apps.len();
        let selected = applet
            .selected_index
            .filter(|index| (start..end).contains(index));
        let hovered = applet
            .hovered_app_index
            .filter(|index| (start..end).contains(index));
        let flags = hovered.map_or(0, |index| {
            let app = &apps[index];
            u64::from(applet.fav_ids.contains(app.id.as_str()))
                | (u64::from(applet.pinned_ids.contains(app.id.as_str())) << 1)
        });
        let key = RowKey {
            generation: applet.available_generation,
            row: row_index,
            columns: layout.columns,
            selected,
            hovered,
            flags,
            grid: layout.grid,
            space_xxs,
            space_s,
        };
        let icons = Arc::clone(&icons);
        let card_height = layout.card_height;
        let icon_size = layout.icon_size;
        let gap = layout.gap;
        let is_grid = layout.grid;
        rows.push(
            lazy(key, move |_| {
                build_app_row(
                    &row_apps,
                    start,
                    layout.columns,
                    &icons,
                    icon_size,
                    card_height,
                    space_xxs,
                    space_s,
                    gap,
                    is_grid,
                    selected,
                    hovered,
                    flags,
                )
            })
            .into(),
        );
    }
    if layout.last_row + 1 < row_count {
        let bottom_h = content_h
            - (layout.last_row as f32 * layout.row_stride + layout.card_height)
            - layout.row_gap;
        if bottom_h > 0.0 {
            rows.push(Space::new().height(Length::Fixed(bottom_h)).into());
        }
    }

    let list = column(rows).spacing(layout.gap).width(Length::Fill);
    let list = container(list)
        .padding([0, space_m.saturating_add(SCROLLBAR_WIDTH as u16), 0, 0])
        .width(Length::Fill);
    crate::vscroll::list(
        list,
        (*APP_SCROLL_ID).clone(),
        layout.row_stride,
        row_count,
        ROW_OVERSCAN,
    )
}

fn cached_visible_rows(
    applet: &Applet,
    layout: &PageLayout,
) -> Arc<Vec<Arc<[Arc<ApplicationEntry>]>>> {
    if let Some(cached) = applet.visible_row_cache.as_ref()
        && cached.generation == applet.available_generation
        && cached.first_row == layout.first_row
        && cached.last_row == layout.last_row
        && cached.columns == layout.columns
    {
        return Arc::clone(&cached.rows);
    }
    row_slices(&applet.available_applications, layout)
}

fn row_slices(
    apps: &[Arc<ApplicationEntry>],
    layout: &PageLayout,
) -> Arc<Vec<Arc<[Arc<ApplicationEntry>]>>> {
    Arc::new(
        (layout.first_row..=layout.last_row)
            .map(|row_index| {
                let start = row_index * layout.columns;
                let end = (start + layout.columns).min(apps.len());
                Arc::<[Arc<ApplicationEntry>]>::from(&apps[start..end])
            })
            .collect::<Vec<_>>(),
    )
}

impl Applet {
    pub(crate) fn sync_visible_rows(&mut self) {
        let spacing = theme::active().cosmic().spacing;
        let (menu_width, menu_height) = if self.is_window_mode {
            (self.window_width, self.window_height)
        } else {
            (self.config.max_width(), self.config.max_height())
        };
        let sidebar_visible = !self.sidebar_collapsed && menu_width >= 600.0;
        let content_width = app_content_width(menu_width, sidebar_visible, spacing);
        let Some(layout) = page_layout(
            self,
            menu_height,
            content_width,
            uses_grid_layout(self),
            spacing.space_xxs,
            spacing.space_s,
        ) else {
            self.visible_row_cache = None;
            return;
        };
        if self.visible_row_cache.as_ref().is_some_and(|cached| {
            cached.generation == self.available_generation
                && cached.first_row == layout.first_row
                && cached.last_row == layout.last_row
                && cached.columns == layout.columns
        }) {
            return;
        }
        self.visible_row_cache = Some(VisibleRowCache {
            generation: self.available_generation,
            first_row: layout.first_row,
            last_row: layout.last_row,
            columns: layout.columns,
            rows: row_slices(&self.available_applications, &layout),
        });
    }
}

fn build_app_row(
    apps: &[Arc<ApplicationEntry>],
    start: usize,
    columns: usize,
    icons: &AppIconHandles,
    icon_size: u16,
    card_height: f32,
    space_xxs: u16,
    space_s: u16,
    gap: u16,
    grid: bool,
    selected: Option<usize>,
    hovered: Option<usize>,
    flags: u64,
) -> Element<'static, Message> {
    let mut buttons: Vec<Element<'static, Message>> = apps
        .iter()
        .enumerate()
        .map(|(offset, app)| {
            let index = start + offset;
            let show_actions = hovered == Some(index);
            let state = AppCardState {
                index,
                is_favourite: show_actions && flags & 1 != 0,
                is_pinned: show_actions && flags & 2 != 0,
                is_selected: selected == Some(index),
                show_actions,
            };
            let icon = icon_widget(icons, app, icon_size);
            if grid {
                app_grid_card(app, icon, state, f32::from(icon_size), card_height, space_xxs)
            } else {
                app_list_card(app, icon, ListCardLayout { space_xxs, space_s }, state)
            }
        })
        .collect();
    for _ in 0..columns.saturating_sub(buttons.len()) {
        buttons.push(Space::new().width(Length::Fill).into());
    }
    row(buttons)
        .spacing(gap)
        .width(Length::Fill)
        .align_y(Alignment::Start)
        .into()
}

impl GridMetrics {
    fn new(width: usize, min_width: usize, column_spacing: u16) -> Self {
        let width_m1 = width.saturating_sub(min_width);
        let cols_m1 = width_m1 / (min_width + column_spacing as usize);
        let cols = cols_m1 + 1;
        Self {
            cols,
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
        crate::app::COSMIC_STORE_APP_ID => Cow::Borrowed("com.system76.CosmicStore"),
        crate::app::COSMIC_SETTINGS_APP_ID => Cow::Borrowed("com.system76.CosmicSettings"),
        _ => Cow::Owned(
            app_id
                .strip_suffix(".desktop")
                .unwrap_or(app_id)
                .to_string(),
        ),
    }
}

fn bottom_bar_fallback_icon(app_id: &str, size: u16) -> cosmic::widget::icon::Icon {
    let fallback = bottom_bar_icon_fallback_name(app_id);
    icon::from_name(fallback)
        .symbolic(false)
        .size(size)
        .fallback(Some(icon::IconFallback::Names(vec![
            "application-x-executable".into(),
            "application-default".into(),
        ])))
        .icon()
        .width(Length::Fixed(f32::from(size)))
        .height(Length::Fixed(f32::from(size)))
}

fn bottom_bar_pinned_item(
    icon: cosmic::widget::icon::Icon,
    pinned_id: &str,
    label: Cow<'static, str>,
    hovered: bool,
    menu_too_small: bool,
    space_xxs: u16,
    space_xs: u16,
) -> Element<'static, Message> {
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
}

#[derive(Clone, Copy)]
struct AppCardState {
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
    is_selected: bool,
    show_actions: bool,
}

fn app_grid_card(
    app: &ApplicationEntry,
    icon: cosmic::widget::icon::Icon,
    state: AppCardState,
    icon_size: f32,
    card_height: f32,
    space_xxs: u16,
) -> Element<'static, Message> {
    let name = app.name.clone();
    let inner = column![
        icon.width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size)),
        single_line(name, true, true),
    ]
    .align_x(Alignment::Center)
    .spacing(space_xxs);
    let inner = container(inner)
        .center_x(Length::Fill)
        .align_y(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([space_xxs, 0, 0, 0]);
    let content: Element<'static, Message> = if state.show_actions {
        stack![
            inner,
            container(app_action_row(
                state.index,
                state.is_favourite,
                state.is_pinned,
            ))
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
    };
    let button = button::custom(content)
        .on_press(Message::LaunchApp(state.index))
        .class(app_grid_card_class(state.is_selected))
        .width(Length::Fill)
        .height(Length::Fixed(card_height));

    mouse_area(button)
        .on_enter(Message::AppHovered(state.index))
        .on_exit(Message::AppUnhovered)
        .into()
}

fn app_list_card(
    app: &ApplicationEntry,
    icon: cosmic::widget::icon::Icon,
    layout: ListCardLayout,
    state: AppCardState,
) -> Element<'static, Message> {
    let summary = app
        .description
        .as_deref()
        .map(|description| truncate_name(description, 80).into_owned())
        .unwrap_or_default();

    let card_height = f32::from(LIST_ICON_SIZE) + f32::from(layout.space_xxs) * 2.0;

    let card_body = row![
        icon,
        column![
            single_line(app.name.clone(), false, false),
            single_line(summary, true, false),
        ]
            .spacing(2)
            .width(Length::Fill)
            .padding([0.0, ACTION_ROW_HEIGHT * 2.0, 0.0, 0.0]),
    ]
    .align_y(Alignment::Center)
    .spacing(layout.space_s)
    .width(Length::Fill);

    let card_content: Element<'static, Message> = if state.show_actions {
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
        .width(Length::Fill)
        .height(Length::Fixed(card_height))
        .class(app_list_card_class(state.is_selected));

    mouse_area(btn)
        .on_enter(Message::AppHovered(state.index))
        .on_exit(Message::AppUnhovered)
        .into()
}

fn single_line(content: String, caption: bool, centered: bool) -> Element<'static, Message> {
    use cosmic::iced::alignment::Horizontal;
    use cosmic::iced::core::text::EllipsizeHeightLimit;
    use cosmic::iced::widget::text::{Ellipsize, Wrapping};
    let ellipsize = Ellipsize::End(EllipsizeHeightLimit::Lines(1));
    let align = if centered {
        Horizontal::Center
    } else {
        Horizontal::Left
    };
    if caption {
        cosmic::widget::text::caption(content)
            .wrapping(Wrapping::None)
            .ellipsize(ellipsize)
            .align_x(align)
            .width(Length::Fill)
            .into()
    } else {
        cosmic::widget::text::body(content)
            .wrapping(Wrapping::None)
            .ellipsize(ellipsize)
            .align_x(align)
            .height(Length::Fixed(20.0))
            .width(Length::Fill)
            .into()
    }
}

fn app_icon_handle(name: Option<&str>, size: u16) -> cosmic::widget::icon::Handle {
    let mut named = match name {
        Some(name) => icon::from_name(name).fallback(Some(icon::IconFallback::Names(vec![
            "application-x-executable".into(),
            "application-default".into(),
        ]))),
        None => icon::from_name("application-x-executable"),
    };
    named.symbolic = false;
    named.size = Some(size);
    named.handle()
}

fn icon_widget(
    handles: &AppIconHandles,
    app: &ApplicationEntry,
    size: u16,
) -> cosmic::widget::icon::Icon {
    let key = (app.icon.clone(), size);
    let handle = handles
        .get(&key)
        .cloned()
        .unwrap_or_else(|| app_icon_handle(app.icon.as_deref(), size));
    icon::icon(handle)
        .size(size)
        .width(Length::Fixed(f32::from(size)))
        .height(Length::Fixed(f32::from(size)))
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
    use super::{truncate_name, visible_row_range};

    #[test]
    fn truncates_at_character_boundaries() {
        assert_eq!(truncate_name("short", 8), "short");
        assert_eq!(truncate_name("exact", 5), "exact");
        assert_eq!(truncate_name("aéioux", 5), "aéio…");
    }

    #[test]
    fn visible_rows_include_one_row_of_overscan() {
        assert_eq!(visible_row_range(0.0, 250.0, 100.0, 20), (0, 3));
        assert_eq!(visible_row_range(400.0, 250.0, 100.0, 20), (3, 7));
        assert_eq!(visible_row_range(10_000.0, 250.0, 100.0, 4), (3, 3));
    }
}
