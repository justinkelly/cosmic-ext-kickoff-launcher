// SPDX-License-Identifier: GPL-3.0-only

use crate::app::Message;
use cosmic::iced::advanced::Renderer as _;
use cosmic::iced::core::layout::{self, Layout};
use cosmic::iced::core::mouse;
use cosmic::iced::core::renderer::{self, Quad};
use cosmic::iced::core::widget::operation::scrollable::{
    AbsoluteOffset, RelativeOffset, Scrollable as ScrollOp,
};
use cosmic::iced::core::widget::tree::{self, Tree};
use cosmic::iced::core::widget::{Operation, Widget};
use cosmic::iced::core::{
    self, Clipboard, Event, Length, Padding, Rectangle, Shell, Size, Vector,
};
use cosmic::iced::widget::scrollable::{self, Catalog as ScrollCatalog};
use cosmic::Element;

const BAR_WIDTH: f32 = 8.0;
const BAR_PAD: f32 = 8.0;
const MIN_THUMB: f32 = 32.0;

pub(crate) fn row_window(
    scroll: f32,
    viewport: f32,
    row_stride: f32,
    row_count: usize,
    overscan: usize,
) -> (usize, usize) {
    if row_count == 0 || !row_stride.is_finite() || row_stride <= f32::EPSILON {
        return (0, 0);
    }
    let scroll = scroll.max(0.0);
    let viewport = if viewport.is_finite() && viewport > 0.0 {
        viewport
    } else {
        row_stride
    };
    let last_row = row_count - 1;
    let first = ((scroll / row_stride).floor() as usize)
        .saturating_sub(overscan)
        .min(last_row);
    let last = (((scroll + viewport) / row_stride).floor() as usize + overscan).min(last_row);
    (first.min(last), last)
}

pub(crate) fn list<'a>(
    content: impl Into<Element<'a, Message>>,
    id: cosmic::widget::Id,
    row_stride: f32,
    row_count: usize,
    overscan: usize,
) -> Element<'a, Message> {
    Element::new(VScroll {
        content: content.into(),
        id,
        row_stride,
        row_count,
        overscan,
    })
}

struct VScroll<'a> {
    content: Element<'a, Message>,
    id: cosmic::widget::Id,
    row_stride: f32,
    row_count: usize,
    overscan: usize,
}

struct State {
    offset: f32,
    viewport: f32,
    content: f32,
    stride: f32,
    row_count: usize,
    overscan: usize,
    grab: Option<f32>,
    notified: Option<(usize, usize, i32)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            offset: 0.0,
            viewport: 0.0,
            content: 0.0,
            stride: 0.0,
            row_count: 0,
            overscan: 0,
            grab: None,
            notified: None,
        }
    }
}

impl State {
    fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    fn set_offset(&mut self, offset: f32) {
        self.offset = offset.clamp(0.0, self.max_offset());
    }

    fn remember(&mut self) {
        if self.viewport < 1.0 || self.row_count == 0 || self.stride <= f32::EPSILON {
            return;
        }
        let (first, last) = row_window(
            self.offset,
            self.viewport,
            self.stride,
            self.row_count,
            self.overscan,
        );
        self.notified = Some((first, last, self.viewport.round() as i32));
    }

    fn publish_if_moved(&mut self, shell: &mut Shell<'_, Message>) {
        if self.viewport < 1.0 || self.row_count == 0 || self.stride <= f32::EPSILON {
            return;
        }
        let (first, last) = row_window(
            self.offset,
            self.viewport,
            self.stride,
            self.row_count,
            self.overscan,
        );
        let key = (first, last, self.viewport.round() as i32);
        if self.notified == Some(key) {
            return;
        }
        self.notified = Some(key);
        shell.publish(Message::AppsScrolled(self.offset, self.viewport));
    }
}

impl ScrollOp for State {
    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        if let Some(y) = offset.y {
            self.set_offset(y.clamp(0.0, 1.0) * self.max_offset());
            self.remember();
        }
    }

    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        if let Some(y) = offset.y {
            self.set_offset(y);
            self.remember();
        }
    }

    fn scroll_by(
        &mut self,
        offset: AbsoluteOffset,
        _bounds: Rectangle,
        _content_bounds: Rectangle,
    ) {
        self.set_offset(self.offset + offset.y);
        self.remember();
    }
}

struct Bar {
    thumb: Rectangle,
    travel: f32,
    track_y: f32,
}

fn bar(bounds: Rectangle, offset: f32, content: f32) -> Option<Bar> {
    let viewport = bounds.height;
    let max_offset = content - viewport;
    if max_offset <= 1.0 || viewport <= 1.0 {
        return None;
    }
    let track_h = (viewport - BAR_PAD * 2.0).max(0.0);
    if track_h <= 1.0 {
        return None;
    }
    let thumb_h = (viewport / content * track_h).clamp(MIN_THUMB.min(track_h), track_h);
    let travel = (track_h - thumb_h).max(1.0);
    let track_y = bounds.y + BAR_PAD;
    let thumb = Rectangle {
        x: bounds.x + bounds.width - BAR_PAD - BAR_WIDTH,
        y: track_y + (offset / max_offset) * travel,
        width: BAR_WIDTH,
        height: thumb_h,
    };
    Some(Bar {
        thumb,
        travel,
        track_y,
    })
}

fn wheel_delta(delta: mouse::ScrollDelta) -> f32 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => -y * 60.0,
        mouse::ScrollDelta::Pixels { y, .. } => -y,
    }
}

impl<'a> Widget<Message, cosmic::Theme, cosmic::Renderer> for VScroll<'a> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn id(&self) -> Option<cosmic::widget::Id> {
        Some(self.id.clone())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        {
            let state = tree.state.downcast_mut::<State>();
            state.stride = self.row_stride;
            state.row_count = self.row_count;
            state.overscan = self.overscan;
        }

        let node = layout::padded(
            limits,
            Length::Fill,
            Length::Fill,
            Padding::ZERO,
            |limits| {
                let max = limits.max();
                let child_limits = layout::Limits::with_compression(
                    Size::new(max.width, 0.0),
                    Size::new(max.width, f32::INFINITY),
                    Size::new(false, true),
                );
                self.content.as_widget_mut().layout(
                    &mut tree.children[0],
                    renderer,
                    &child_limits,
                )
            },
        );

        let state = tree.state.downcast_mut::<State>();
        state.content = node
            .children()
            .first()
            .map(|child| child.size().height)
            .unwrap_or(0.0);
        state.viewport = node.size().height;
        state.set_offset(state.offset);
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn Operation,
    ) {
        let bounds = layout.bounds();
        let content = layout.children().next().unwrap();
        let offset = tree.state.downcast_ref::<State>().offset;
        operation.scrollable(
            Some(&self.id),
            bounds,
            content.bounds(),
            Vector::new(0.0, offset),
            tree.state.downcast_mut::<State>(),
        );
        let translation = Vector::new(0.0, tree.state.downcast_ref::<State>().offset);
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                content.with_virtual_offset(translation + layout.virtual_offset()),
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let content = layout.children().next().unwrap();
        let state = tree.state.downcast_mut::<State>();
        state.stride = self.row_stride;
        state.row_count = self.row_count;
        state.overscan = self.overscan;
        state.content = content.bounds().height;
        state.viewport = bounds.height;
        state.set_offset(state.offset);

        let geometry = bar(bounds, state.offset, state.content);
        let over_thumb = geometry
            .as_ref()
            .is_some_and(|bar| cursor.position_over(bar.thumb).is_some());
        let mut handled = false;
        let mut moved = false;

        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.grab = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(grab) = state.grab
                    && let Some(position) = cursor.position()
                    && let Some(bar) = geometry.as_ref()
                {
                    let thumb_y = position.y - grab;
                    let percent = (thumb_y - bar.track_y) / bar.travel;
                    let previous = state.offset;
                    state.set_offset(percent * state.max_offset());
                    moved = state.offset != previous;
                    handled = true;
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_over(bounds)
                    && let Some(bar) = geometry.as_ref()
                {
                    if bar.thumb.contains(position) {
                        state.grab = Some(position.y - bar.thumb.y);
                        handled = true;
                        shell.capture_event();
                    } else if position.x >= bar.thumb.x && position.x <= bar.thumb.x + bar.thumb.width
                    {
                        let delta = if position.y < bar.thumb.y {
                            -bounds.height * 0.8
                        } else {
                            bounds.height * 0.8
                        };
                        let previous = state.offset;
                        state.set_offset(state.offset + delta);
                        moved = state.offset != previous;
                        handled = true;
                        shell.capture_event();
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_over(bounds).is_some() && state.max_offset() > 1.0 {
                    let previous = state.offset;
                    state.set_offset(state.offset + wheel_delta(*delta));
                    moved = state.offset != previous;
                    handled = true;
                    shell.capture_event();
                }
            }
            _ => {}
        }

        if moved {
            shell.request_redraw();
        }
        state.publish_if_moved(shell);

        if handled {
            return;
        }

        let translation = Vector::new(0.0, state.offset);
        let child_cursor = match cursor.position_over(bounds) {
            Some(position) if !over_thumb => mouse::Cursor::Available(position + translation),
            Some(_) => mouse::Cursor::Unavailable,
            None => cursor.levitate() + translation,
        };
        let child_viewport = Rectangle {
            x: bounds.x + translation.x,
            y: bounds.y + translation.y,
            ..bounds
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content.with_virtual_offset(translation + layout.virtual_offset()),
            child_cursor,
            renderer,
            clipboard,
            shell,
            &child_viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let content = layout.children().next().unwrap();
        if state.grab.is_some()
            || bar(bounds, state.offset, content.bounds().height)
                .is_some_and(|bar| cursor.position_over(bar.thumb).is_some())
        {
            return mouse::Interaction::Pointer;
        }
        let translation = Vector::new(0.0, state.offset);
        let child_cursor = match cursor.position_over(bounds) {
            Some(position) => mouse::Cursor::Available(position + translation),
            None => cursor.levitate() + translation,
        };
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content.with_virtual_offset(translation + layout.virtual_offset()),
            child_cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let Some(visible) = bounds.intersection(viewport) else {
            return;
        };
        let content = layout.children().next().unwrap();
        let translation = Vector::new(0.0, state.offset);
        let child_cursor = match cursor.position_over(bounds) {
            Some(position) => mouse::Cursor::Available(position + translation),
            None => mouse::Cursor::Unavailable,
        };
        let child_viewport = Rectangle {
            x: visible.x + translation.x,
            y: visible.y + translation.y,
            ..visible
        };

        renderer.with_layer(visible, |renderer| {
            renderer.with_translation(Vector::new(-translation.x, -translation.y), |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    defaults,
                    content.with_virtual_offset(translation + layout.virtual_offset()),
                    child_cursor,
                    &child_viewport,
                );
            });
        });

        let Some(bar) = bar(bounds, state.offset, content.bounds().height) else {
            return;
        };
        let status = scrollable::Status::Active {
            is_horizontal_scrollbar_disabled: true,
            is_vertical_scrollbar_disabled: false,
        };
        let class = <cosmic::Theme as ScrollCatalog>::default();
        let style = theme.style(&class, status);
        renderer.fill_quad(
            Quad {
                bounds: bar.thumb,
                border: style.vertical_rail.scroller.border,
                ..Quad::default()
            },
            style.vertical_rail.scroller.background,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &cosmic::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<core::overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let visible = bounds.intersection(viewport).unwrap_or(*viewport);
        let offset = Vector::new(0.0, state.offset);
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout
                .children()
                .next()
                .unwrap()
                .with_virtual_offset(translation + layout.virtual_offset()),
            renderer,
            &visible,
            translation - offset,
        )
    }
}
