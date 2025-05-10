use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::time::Duration;

use basalt::input::{MouseButton, Qwerty};
use basalt::interface::UnitValue::Pixels;
use basalt::interface::{Bin, BinStyle, Position, TextBody, TextSelection};
use basalt::interval::IntvlHookID;
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetContainer, WidgetPlacement};

/// Builder for [`TextArea`]
pub struct TextAreaBuilder<'a, C> {
    widget: WidgetBuilder<'a, C>,
    props: Properties,
    text_body: TextBody,
}

#[derive(Default)]
struct Properties {
    placement: WidgetPlacement,
}

impl Properties {
    fn new(placement: WidgetPlacement) -> Self {
        Self {
            placement,
        }
    }
}

impl<'a, C> TextAreaBuilder<'a, C>
where
    C: WidgetContainer,
{
    pub(crate) fn with_builder(mut builder: WidgetBuilder<'a, C>) -> Self {
        Self {
            props: Properties::new(
                builder
                    .placement
                    .take()
                    .unwrap_or_else(|| TextArea::default_placement(&builder.theme)),
            ),
            widget: builder,
            text_body: Default::default(),
        }
    }

    /// Set inital text body.
    pub fn text_body(mut self, text_body: TextBody) -> Self {
        self.text_body = text_body;
        self
    }

    /// Finish building the [`TextArea`].
    pub fn build(self) -> Arc<TextArea> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let container = window.new_bin();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        let text_area = Arc::new(TextArea {
            theme: self.widget.theme,
            props: self.props,
            container,
            state: ReentrantMutex::new(State {
                c_blink_intvl_hid: RefCell::new(None),
            }),
        });

        let cb_text_area = text_area.clone();

        text_area.container.on_focus(move |_, _| {
            if cb_text_area.theme.border.is_some() {
                cb_text_area.container.style_modify(|style| {
                    style.border_color_t = cb_text_area.theme.colors.accent1;
                    style.border_color_b = cb_text_area.theme.colors.accent1;
                    style.border_color_l = cb_text_area.theme.colors.accent1;
                    style.border_color_r = cb_text_area.theme.colors.accent1;
                });

                cb_text_area.start_cursor_blink();
            }

            Default::default()
        });

        let cb_text_area = text_area.clone();

        text_area.container.on_focus_lost(move |_, _| {
            if cb_text_area.theme.border.is_some() {
                cb_text_area.container.style_modify(|style| {
                    style.border_color_t = cb_text_area.theme.colors.border1;
                    style.border_color_b = cb_text_area.theme.colors.border1;
                    style.border_color_l = cb_text_area.theme.colors.border1;
                    style.border_color_r = cb_text_area.theme.colors.border1;
                    style.text_body.cursor_color.a = 0.0;
                });

                cb_text_area.pause_cursor_blink();
            }

            Default::default()
        });

        let selecting = Arc::new(AtomicBool::new(false));
        let cb_text_area = text_area.clone();
        let cb_selecting = selecting.clone();

        text_area
            .container
            .on_press(MouseButton::Left, move |_, window, _| {
                let cursor_op = cb_text_area.container.get_text_cursor(window.cursor_pos());

                let reset_cursor = cb_text_area.container.style_modify(|style| {
                    let reset_cursor = style.text_body.cursor.is_some();
                    style.text_body.cursor = cursor_op;
                    style.text_body.selection = None;
                    style.text_body.cursor_color = cb_text_area.theme.colors.text1a;
                    reset_cursor
                });

                if reset_cursor && cursor_op.is_some() {
                    cb_text_area.reset_cursor_blink();
                }

                if cursor_op.is_some() {
                    cb_selecting.store(true, atomic::Ordering::Relaxed);
                }

                Default::default()
            });

        let cb_selecting = selecting.clone();

        text_area
            .container
            .on_release(MouseButton::Left, move |_, _, _| {
                cb_selecting.store(false, atomic::Ordering::Relaxed);
                Default::default()
            });

        let cb_text_area = text_area.clone();
        let mut cursor_visible = false;

        *text_area.state.lock().c_blink_intvl_hid.borrow_mut() =
            Some(window.basalt_ref().interval_ref().do_every(
                Duration::from_millis(500),
                None,
                move |elapsed| {
                    if elapsed.is_none() {
                        cursor_visible = true;
                    } else {
                        cursor_visible = !cursor_visible;
                    }

                    cb_text_area.container.style_modify(|style| {
                        if cursor_visible {
                            style.text_body.cursor_color.a = 1.0;
                        } else {
                            style.text_body.cursor_color.a = 0.0;
                        }
                    });

                    Default::default()
                },
            ));

        let cb_text_area = text_area.clone();
        let cb_selecting = selecting.clone();

        text_area.container.on_cursor(move |_, window, _| {
            if !cb_selecting.load(atomic::Ordering::Relaxed) {
                return Default::default();
            }

            cb_text_area.container.style_modify(|style| {
                let sel_from = match style.text_body.cursor {
                    Some(sel_from) => sel_from,
                    None => return,
                };

                match cb_text_area.container.get_text_cursor(window.cursor_pos()) {
                    Some(sel_to) => {
                        if sel_to == sel_from {
                            style.text_body.selection = None;
                        } else if sel_to > sel_from {
                            style.text_body.selection = Some(TextSelection {
                                start: sel_from,
                                end: sel_to,
                            });
                        } else {
                            style.text_body.selection = Some(TextSelection {
                                start: sel_to,
                                end: sel_from,
                            });
                        }
                    },
                    None => {
                        style.text_body.selection = None;
                    },
                }

                debug_assert!(
                    style
                        .text_body
                        .selection
                        .map(|selection| style.text_body.is_selection_valid(selection))
                        .unwrap_or(true)
                );
            });

            Default::default()
        });

        let cb_text_area = text_area.clone();

        text_area
            .container
            .on_press(Qwerty::ArrowLeft, move |_, _, _| {
                cb_text_area.container.style_modify(|style| {
                    let curr_cursor = match style.text_body.cursor {
                        Some(curr_cursor) => curr_cursor,
                        None => return,
                    };

                    style.text_body.cursor = Some(
                        style
                            .text_body
                            .cursor_prev(curr_cursor)
                            .unwrap_or(curr_cursor),
                    );
                });

                cb_text_area.reset_cursor_blink();
                Default::default()
            });

        let cb_text_area = text_area.clone();

        text_area
            .container
            .on_press(Qwerty::ArrowRight, move |_, _, _| {
                cb_text_area.container.style_modify(|style| {
                    let curr_cursor = match style.text_body.cursor {
                        Some(curr_cursor) => curr_cursor,
                        None => return,
                    };

                    style.text_body.cursor = Some(
                        style
                            .text_body
                            .cursor_next(curr_cursor)
                            .unwrap_or(curr_cursor),
                    );
                });

                cb_text_area.reset_cursor_blink();
                Default::default()
            });

        let cb_text_area = text_area.clone();

        text_area.container.on_character(move |_, _, mut c| {
            cb_text_area.container.style_modify(|style| {
                if c.is_backspace() {
                    if style.text_body.cursor.is_none() {
                        return;
                    }

                    style.text_body.cursor = Some(
                        style
                            .text_body
                            .cursor_delete(style.text_body.cursor.unwrap())
                            .unwrap(),
                    );

                    cb_text_area.reset_cursor_blink();
                } else {
                    if c.0 == '\r' {
                        c.0 = '\n';
                    }

                    if style.text_body.cursor.is_none() {
                        style.text_body.cursor = Some(style.text_body.push(*c));
                    } else {
                        style.text_body.cursor = Some(
                            style
                                .text_body
                                .cursor_insert(style.text_body.cursor.unwrap(), *c)
                                .unwrap(),
                        );
                    }

                    cb_text_area.reset_cursor_blink();
                }
            });

            Default::default()
        });

        text_area.style_update(Some(self.text_body));
        text_area
    }
}

/// TextArea widget.
pub struct TextArea {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    c_blink_intvl_hid: RefCell<Option<IntvlHookID>>,
}

impl TextArea {
    /// Obtain the default [`WidgetPlacement`](`WidgetPlacement`) given a [`Theme`](`Theme`).
    pub fn default_placement(theme: &Theme) -> WidgetPlacement {
        let height = theme.spacing + (theme.base_size * 5.0);
        let width = height * 3.0;

        WidgetPlacement {
            position: Position::Floating,
            margin_t: Pixels(theme.spacing),
            margin_b: Pixels(theme.spacing),
            margin_l: Pixels(theme.spacing),
            margin_r: Pixels(theme.spacing),
            width: Pixels(width),
            height: Pixels(height),
            ..Default::default()
        }
    }

    fn reset_cursor_blink(&self) {
        let _state = self.state.lock();
        self.pause_cursor_blink();
        self.start_cursor_blink();
    }

    fn start_cursor_blink(&self) {
        self.container
            .basalt_ref()
            .interval_ref()
            .start(self.state.lock().c_blink_intvl_hid.borrow().unwrap());
    }

    fn pause_cursor_blink(&self) {
        self.container
            .basalt_ref()
            .interval_ref()
            .pause(self.state.lock().c_blink_intvl_hid.borrow().unwrap());
    }

    fn style_update(&self, text_body_op: Option<TextBody>) {
        self.container.style_modify(|style| {
            let mut text_body = text_body_op.unwrap_or_else(|| style.text_body.clone());

            // TODO: This doesn't feel right
            text_body.base_attrs.height = Pixels(self.theme.text_height);
            text_body.base_attrs.color = self.theme.colors.text1a;
            text_body.base_attrs.font_family = self.theme.font_family.clone();
            text_body.base_attrs.font_weight = self.theme.font_weight;

            *style = BinStyle {
                back_color: self.theme.colors.back2,
                text_body,
                padding_t: Pixels(self.theme.spacing),
                padding_b: Pixels(self.theme.spacing),
                padding_l: Pixels(self.theme.spacing),
                padding_r: Pixels(self.theme.spacing),
                ..self.props.placement.clone().into_style()
            };

            if let Some(border_size) = self.theme.border {
                style.border_size_t = Pixels(border_size);
                style.border_size_b = Pixels(border_size);
                style.border_size_l = Pixels(border_size);
                style.border_size_r = Pixels(border_size);
                style.border_color_t = self.theme.colors.border1;
                style.border_color_b = self.theme.colors.border1;
                style.border_color_l = self.theme.colors.border1;
                style.border_color_r = self.theme.colors.border1;
            }

            if let Some(border_radius) = self.theme.roundness {
                style.border_radius_tl = Pixels(border_radius);
                style.border_radius_tr = Pixels(border_radius);
                style.border_radius_bl = Pixels(border_radius);
                style.border_radius_br = Pixels(border_radius);
            }
        });
    }
}
