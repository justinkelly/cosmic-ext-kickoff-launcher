// SPDX-License-Identifier: GPL-3.0-only

//! View rendering: menu layout, grid/list virtualization, bottom bar, and the
//! settings panel.

use crate::app::{
    Applet, Message, CORNER_BADGE_ICON_SIZE, GRID_ICON_SIZE, LIST_ICON_SIZE, SCROLLABLE_ID,
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

/// Panel-icon options for the settings dropdown: (icon name, display label).
///
/// Names in [`crate::icons::BUNDLED_NAMES`] resolve from SVG assets embedded
/// in the binary (so they render on any icon theme, e.g. the distro logos
/// which the COSMIC icon theme lacks). All other names must exist in the
/// installed icon theme — the previous list contained theme-only icons that
/// rendered blank on the COSMIC theme and were removed.
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

/// Display labels for [`ICON_OPTIONS`], computed once.
static ICON_OPTION_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    ICON_OPTIONS.iter().map(|(_, label)| (*label).to_string()).collect()
});

/// Preset labels never change during the lifetime of the applet. Keeping
/// these borrowed avoids rebuilding and cloning the dropdown model on every
/// pointer or keyboard event in the settings panel.
static SIZE_PRESET_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    SizePreset::ALL.iter().map(|preset| size_preset_label(*preset)).collect()
});

/// Estimated height of bottom-bar button content (icon/text line).
const BOTTOM_BAR_CONTENT_HEIGHT: f32 = 21.0;
/// Minimum touch-target height for the bottom bar.
const BOTTOM_BAR_MIN_HEIGHT: f32 = 44.0;
/// Minimum width of a grid cell button.
const GRID_MIN_BUTTON_WIDTH: f32 = 80.0;

impl Applet {
    pub fn build_menu_view(&self, is_popup: bool) -> Element<'_, Message> {
        let cosmic_theme = theme::active();
        let Spacing {
            space_xxs,
            space_xs,
            space_s,
            space_m,
            ..
        } = cosmic_theme.cosmic().spacing;
        let menu_width = self.config.max_width();
        let menu_height = self.config.max_height();
        let show_bottom_bar_power = self.config.show_bottom_bar_power_actions;
        let show_bottom_bar_pinned =
            self.config.show_bottom_bar_pinned && !self.pinned_apps.is_empty();
        let show_bottom_bar = show_bottom_bar_pinned || show_bottom_bar_power;
        // Bottom bar height: button content + button padding (2×space_xxs)
        // + symmetric row padding (2×space_xxs), minimum touch target.
        let bottom_bar_height = if show_bottom_bar {
            (BOTTOM_BAR_CONTENT_HEIGHT + space_xxs as f32 * 4.0).max(BOTTOM_BAR_MIN_HEIGHT)
        } else {
            0.0
        };
        tracing::debug!(
            "view_window: menu={}×{} bottom_bar_h={} space_m={} space_s={}",
            menu_width,
            menu_height,
            bottom_bar_height,
            space_m,
            space_s
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
        let app_area_height = (menu_height - outer_pad - total_spacing - bottom_bar_height)
            .max(200.0);
        tracing::debug!(
            "view_window: outer_pad={} col_spacings={} total_spacing={} app_area_h={}",
            outer_pad,
            col_spacing_count,
            total_spacing,
            app_area_height
        );

        // In window mode the header bar (header_start/header_end) already
        // provides sidebar-toggle, search, and config buttons.  In popup mode
        // we render them as an in-content top bar.
        let top_bar: Element<'_, Message> = if is_window {
            Space::new()
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into()
        } else {
            let sidebar_toggle: Element<'_, Message> = nav_bar_toggle()
                .on_toggle(Message::ToggleSidebar)
                .active(!self.sidebar_collapsed)
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

            let window_btn: Element<'_, Message> = tooltip(
                button::custom(
                    icon::from_name("window-new-symbolic")
                        .symbolic(true)
                        .size(18)
                        .icon(),
                )
                .on_press(Message::OpenWindow)
                .class(theme::Button::AppletMenu),
                cosmic::widget::text::body(fl!("open-window")),
                cosmic::widget::tooltip::Position::Top,
            )
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

            container(
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
            .into()
        };

        // ── Search bar (collapsible) ──
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

        // ── Category sidebar (collapsible) ──
        let nav: Element<'_, Message> = if self.sidebar_collapsed {
            Space::new()
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into()
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
        let use_grid = self.config.layout_mode == LayoutMode::Grid
            || (self.config.layout_mode == LayoutMode::Hybrid
                && matches!(
                    self.selected_category.as_ref(),
                    Some(cat) if cat.key == "favourites" || cat.key == "recents"
                ));
        // Grid view uses the larger package-card icon size (matching cosmic-store).
        let effective_icon_size: f32 = f32::from(GRID_ICON_SIZE);

        let app_area: Element<'_, Message> = if self.show_settings {
            // The settings panel is the active surface. Do not rebuild and
            // retain dozens of application cards/icons behind it on every
            // dropdown interaction.
            Space::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.available_applications.is_empty() {
            container(cosmic::widget::text::body(fl!("no-applications")))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
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
                avail_width -= SETTINGS_PANEL_WIDTH;
            }
            // Each grid button needs at least 80px; compute columns.
            let min_btn = GRID_MIN_BUTTON_WIDTH + space_s as f32;
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
                            if show_actions {
                                stack![
                                    inner,
                                    container(app_action_row(index, is_fav, is_pinned, true))
                                        .width(Length::Fill)
                                        .height(Length::Fill)
                                        .align_y(Alignment::Start)
                                        // Keep the action glyphs close to the card's
                                        // top and side borders.
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
                            .class(app_list_card_class(is_selected))
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
                row(buttons)
                    .spacing(space_s)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                    .into()
            };

            // Virtualize: only render rows intersecting the viewport (plus one
            // row of overscan on each side), keeping total height identical so
            // the scrollbar and layout are unchanged.
            let viewport_h = if self.grid_viewport_h > 0.0 {
                self.grid_viewport_h
            } else {
                menu_height
            };
            let scroll = self
                .grid_scroll_y
                .clamp(0.0, (content_h - viewport_h).max(0.0));
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

            // Keep the trailing gutter inside the scrollable content. The
            // scrollbar occupies the viewport's right edge, so without this
            // inset the final column is flush against it while the first
            // column still has the main layout's leading padding.
            let app_grid = container(column(rows).spacing(space_s))
                .padding([0, space_xxs, 0, 0])
                .width(Length::Fill);
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
                list_width =
                    list_width.saturating_sub(SIDEBAR_WIDTH as usize + space_m as usize);
            }
            if self.show_settings {
                list_width = list_width.saturating_sub(SETTINGS_PANEL_WIDTH as usize);
            }

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

            // Virtualize: only render rows near the viewport.
            let viewport_h = if self.grid_viewport_h > 0.0 {
                self.grid_viewport_h
            } else {
                menu_height
            };
            let scroll = self
                .grid_scroll_y
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

            // Pre-compute text column width once per view (same for all cards).
            let text_width = item_width.saturating_sub(
                LIST_ICON_SIZE as usize + space_s as usize * 3,
            ) as f32;

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

            container(
                scrollable(column(rows).spacing(column_spacing).width(Length::Fill))
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

        let app_area: Element<'_, Message> = mouse_area(
            container(app_area)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_exit(Message::ClearAppHover)
        .into();

        // ── Settings panel (right side) ──
        let settings_panel: Option<Element<'_, Message>> = if self.show_settings {
            // Layout mode (radio group — one of Grid/List/Hybrid)
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

            // Menu size preset (dropdown)
            let size_labels = if self.config.custom_width > 0.0
                && self.config.custom_height > 0.0
            {
                let mut labels = SIZE_PRESET_LABELS.clone();
                labels[SizePreset::ALL.len() - 1] = fl!(
                    "size-preset",
                    preset = fl!("size-custom"),
                    width = (self.config.custom_width as u32).to_string(),
                    height = (self.config.custom_height as u32).to_string()
                );
                std::borrow::Cow::Owned(labels)
            } else {
                std::borrow::Cow::Borrowed(SIZE_PRESET_LABELS.as_slice())
            };
            // The dropdown shows "Custom" while the custom size is being
            // edited, even though the config preset is unchanged until Apply.
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

            // Custom size inputs — shown while "Custom" is being edited
            // (selected but not applied, or already applied).
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
                            .width(Length::Fill),
                    ]
                    .spacing(space_xxs)
                    .width(Length::Fill)
                    .into()
                });

            // Panel icon dropdown
            let current_icon = match self.config.panel_icon.as_str() {
                "" => "cosmic-logo",
                // Migrate the old bundled KDE icon visually without changing
                // the user's persisted configuration on every view rebuild.
                "kde" => "kde-official",
                name => name,
            };
            let selected_idx = ICON_OPTIONS
                .iter()
                .position(|(name, _)| *name == current_icon);
            // Keep this text-only. Rendering every SVG preview in the popup
            // makes opening the panel-icon selector noticeably expensive.
            let picker: Element<'_, Message> = dropdown(
                ICON_OPTION_LABELS.as_slice(),
                selected_idx,
                move |idx: usize| {
                let icon_name = ICON_OPTIONS[idx].0.to_string();
                Message::SetPanelIcon(icon_name)
                },
            )
            .width(Length::Fill)
            .padding([space_xxs, space_s])
            .into();

            // Boolean toggles (titles are provided by the settings items)
            let monochrome_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.panel_icon_symbolic)
                    .on_toggle(|_| Message::TogglePanelIconSymbolic)
                    .into();
            let show_favourites_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.show_favourites)
                    .on_toggle(|_| Message::ToggleShowFavourites)
                    .into();
            let show_recents_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.show_recents)
                    .on_toggle(|_| Message::ToggleShowRecents)
                    .into();
            let hide_sidebar_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.sidebar_collapsed)
                    .on_toggle(|_| Message::ToggleSidebarDefault)
                    .into();
            let show_pinned_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.show_bottom_bar_pinned)
                    .on_toggle(|_| Message::ToggleShowBottomBarPinned)
                    .into();
            let show_power_toggle: Element<'_, Message> =
                cosmic::widget::toggler(self.config.show_bottom_bar_power_actions)
                    .on_toggle(|_| Message::ToggleShowBottomBarPowerActions)
                    .into();

            // Default category dropdown
            // Default category dropdown — mirrors the sidebar nav: All
            // Applications, Favourites/Recents (when shown in the sidebar)
            // and every app category.
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

            let content = column![
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
                    .add(settings::item(fl!("monochrome-icon"), monochrome_toggle)),
                settings::section()
                    .title(fl!("sidebar"))
                    .add(settings::item(fl!("show-favourites"), show_favourites_toggle))
                    .add(settings::item(fl!("show-recents"), show_recents_toggle))
                    .add(settings::item(
                        fl!("hide-sidebar-by-default"),
                        hide_sidebar_toggle,
                    )),
                settings::section()
                    .title(fl!("bottom-bar"))
                    .add(settings::item(fl!("show-pinned-apps"), show_pinned_toggle))
                    .add(settings::item(fl!("show-power-actions"), show_power_toggle)),
                settings::section()
                    .title(fl!("default-menu"))
                    .add(stacked_item(
                        fl!("default-category"),
                        default_picker,
                        space_xxs
                    )),
            ]
            .spacing(space_s);

            Some(
                container(scrollable(content).height(Length::Fill))
                    .width(Length::Fixed(SETTINGS_PANEL_WIDTH))
                    .height(Length::Fill)
                    .class(theme::Container::Primary)
                    .into(),
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
                            cosmic::widget::divider::vertical::default().height(Length::Fill),
                        )
                        .height(Length::Fixed(bottom_bar_height - space_xxs as f32 * 2.0))
                        .padding([0, space_xxs]),
                    );
                }
            }

            if show_bottom_bar_power {
                for &action in PowerAction::BOTTOM_BAR.iter() {
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

            container(bottom_row.padding([space_xxs, space_xxs, space_xxs, space_xxs]))
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
                let icon = icon::from_name(std::sync::Arc::from(cat.icon_name.as_str()))
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

/// A settings item with the label above the control, so dropdowns and
/// segmented controls get the full panel width instead of being squashed
/// into the narrow space beside the label.
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

/// Localized label for a size preset, including dimensions.
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
    // Keep the glyph filled in both states; state is communicated by the
    // COSMIC accent color rather than switching to an outline star.
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

/// Compact action button placed beside the app icon.
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

/// Pin and favourite actions remain discoverable above grid icons and beside
/// list icons. Active actions use the theme accent; inactive actions are
/// solid monochrome foreground icons.
fn app_action_row(
    index: usize,
    is_favourite: bool,
    is_pinned: bool,
    show_actions: bool,
) -> Element<'static, Message> {
    if !show_actions {
        return Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(ACTION_ROW_HEIGHT))
            .into();
    }

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

/// Idle card look (matches `Container::Card`). Hover/active uses the
/// left-menu nav highlight background without changing text/icon colour.
fn app_list_card_class(selected: bool) -> theme::Button {
    use cosmic::iced::{Background, Color};

    let idle = |theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut style = cosmic::widget::button::Style::new();
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
        // Match NavBar hover alpha (0.3) from libcosmic segmented_button.
        hovered: Box::new(move |_focused, theme| highlight(theme, 0.3)),
        pressed: Box::new(move |_focused, theme| highlight(theme, 0.25)),
    }
}

#[allow(clippy::too_many_arguments)]
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

    let card_content: Element<'a, Message> = if show_actions {
        stack![
            card_body,
            container(app_action_buttons(index, is_favourite, is_pinned))
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

pub fn app_icon(app: &ApplicationEntry, size: f32) -> cosmic::widget::icon::Icon {
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
