use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::time::Duration;

use basalt::input::{MouseButton, Qwerty};
use basalt::interface::UnitValue::Pixels;
use basalt::interface::{
    Bin, BinPostUpdate, BinStyle, Position, TextBody, TextCursor, TextSelection,
};
use basalt::interval::IntvlHookID;
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{ScrollAxis, ScrollBar, Theme, WidgetContainer, WidgetPlacement, ulps_eq};

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

        let mut bins = window.new_bins(2).into_iter();
        let container = bins.next().unwrap();
        let editor = bins.next().unwrap();

        container.add_child(editor.clone());

        let sb_size = match ScrollBar::default_placement(&self.widget.theme, ScrollAxis::Y).width {
            Pixels(px) => px,
            _ => unreachable!(),
        };

        let border_size = self.widget.theme.border.unwrap_or(0.0);

        let v_scroll_b = container
            .create_widget()
            .with_theme(self.widget.theme.clone())
            .with_placement(WidgetPlacement {
                pos_from_b: Pixels(sb_size + border_size),
                ..ScrollBar::default_placement(&self.widget.theme, ScrollAxis::Y)
            })
            .scroll_bar(&editor)
            .build();

        let h_scroll_b = container
            .create_widget()
            .with_theme(self.widget.theme.clone())
            .with_placement(WidgetPlacement {
                pos_from_r: Pixels(sb_size + border_size),
                ..ScrollBar::default_placement(&self.widget.theme, ScrollAxis::X)
            })
            .scroll_bar(&editor)
            .axis(ScrollAxis::X)
            .build();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        let text_area = Arc::new(TextArea {
            theme: self.widget.theme,
            props: self.props,
            container,
            editor,
            v_scroll_b,
            h_scroll_b,
            state: ReentrantMutex::new(State {
                c_blink_intvl_hid: RefCell::new(None),
            }),
        });

        let cb_text_area = text_area.clone();

        text_area.editor.on_focus(move |_, _| {
            if cb_text_area.theme.border.is_some() {
                cb_text_area.editor.style_modify(|style| {
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

        text_area.editor.on_focus_lost(move |_, _| {
            if cb_text_area.theme.border.is_some() {
                cb_text_area.editor.style_modify(|style| {
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
            .editor
            .on_press(MouseButton::Left, move |_, window, _| {
                let cursor = cb_text_area.editor.get_text_cursor(window.cursor_pos());

                cb_text_area.editor.style_modify(|style| {
                    style.text_body.cursor = cursor;
                    style.text_body.selection = None;
                    style.text_body.cursor_color = cb_text_area.theme.colors.text1a;
                });

                if cursor != TextCursor::None {
                    cb_text_area.reset_cursor_blink();
                    cb_selecting.store(true, atomic::Ordering::Relaxed);
                }

                Default::default()
            });

        let cb_selecting = selecting.clone();

        text_area
            .editor
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

                    cb_text_area.editor.style_modify(|style| {
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

        text_area.editor.on_cursor(move |_, window, _| {
            if !cb_selecting.load(atomic::Ordering::Relaxed) {
                return Default::default();
            }

            // Note: `get_text_cursor` must be called outside of `style_modify`.
            let sel_to_cursor = cb_text_area.editor.get_text_cursor(window.cursor_pos());

            cb_text_area.editor.style_modify(|style| {
                let sel_from = match style.text_body.cursor {
                    TextCursor::None | TextCursor::Empty => return,
                    TextCursor::Position(sel_from) => sel_from,
                };

                match sel_to_cursor {
                    TextCursor::None | TextCursor::Empty => {
                        style.text_body.selection = None;
                    },
                    TextCursor::Position(sel_to) => {
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
                }
            });

            Default::default()
        });

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowLeft, move |_, _, _| {
                cb_text_area.move_cursor_left();
                Default::default()
            });

        let cb_text_area = text_area.clone();

        window
            .basalt_ref()
            .input_ref()
            .hook()
            .bin(&text_area.editor)
            .on_hold()
            .keys(Qwerty::ArrowLeft)
            .delay(Some(Duration::from_millis(600)))
            .interval(Duration::from_millis(40))
            .call(move |_, _, _| {
                cb_text_area.move_cursor_left();
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowRight, move |_, _, _| {
                cb_text_area.move_cursor_right();
                Default::default()
            });

        let cb_text_area = text_area.clone();

        window
            .basalt_ref()
            .input_ref()
            .hook()
            .bin(&text_area.editor)
            .on_hold()
            .keys(Qwerty::ArrowRight)
            .delay(Some(Duration::from_millis(600)))
            .interval(Duration::from_millis(40))
            .call(move |_, _, _| {
                cb_text_area.move_cursor_right();
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area.editor.on_press(Qwerty::ArrowUp, move |_, _, _| {
            cb_text_area.move_cursor_up();
            Default::default()
        });

        let cb_text_area = text_area.clone();

        window
            .basalt_ref()
            .input_ref()
            .hook()
            .bin(&text_area.editor)
            .on_hold()
            .keys(Qwerty::ArrowUp)
            .delay(Some(Duration::from_millis(600)))
            .interval(Duration::from_millis(40))
            .call(move |_, _, _| {
                cb_text_area.move_cursor_up();
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowDown, move |_, _, _| {
                cb_text_area.move_cursor_down();
                Default::default()
            });

        let cb_text_area = text_area.clone();

        window
            .basalt_ref()
            .input_ref()
            .hook()
            .bin(&text_area.editor)
            .on_hold()
            .keys(Qwerty::ArrowDown)
            .delay(Some(Duration::from_millis(600)))
            .interval(Duration::from_millis(40))
            .call(move |_, _, _| {
                cb_text_area.move_cursor_down();
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area.editor.on_character(move |_, _, mut c| {
            let cb2_text_area = cb_text_area.clone();

            cb_text_area.editor.style_modify_then(
                |style| {
                    if let Some(selection) = style.text_body.selection.take() {
                        style.text_body.cursor = style.text_body.selection_delete(selection);

                        if c.is_backspace() {
                            cb_text_area.reset_cursor_blink();
                            return style.text_body.cursor;
                        }
                    }

                    if c.is_backspace() {
                        if matches!(style.text_body.cursor, TextCursor::None | TextCursor::Empty) {
                            return style.text_body.cursor;
                        }

                        style.text_body.cursor =
                            style.text_body.cursor_delete(style.text_body.cursor);

                        cb_text_area.reset_cursor_blink();
                    } else {
                        if c.0 == '\r' {
                            c.0 = '\n';
                        }

                        style.text_body.cursor =
                            style.text_body.cursor_insert(style.text_body.cursor, *c);

                        if style.text_body.cursor != TextCursor::None {
                            cb_text_area.reset_cursor_blink();
                        }
                    }

                    style.text_body.cursor
                },
                move |_editor, bpu, cursor| {
                    cb2_text_area.check_cursor_in_view(bpu, cursor);
                },
            );

            Default::default()
        });

        /*text_area.editor.on_update(|container, _| {
            let cursor = container.style_inspect(|style| style.text_body.cursor);
            let bounds = container.get_text_cursor_bounds(cursor);
            println!("Cursor:        {:?}", cursor);
            println!("Cursor Bounds: {:?}", bounds);
        });*/

        text_area.style_update(Some(self.text_body));
        text_area
    }
}

/// TextArea widget.
pub struct TextArea {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    editor: Arc<Bin>,
    v_scroll_b: Arc<ScrollBar>,
    h_scroll_b: Arc<ScrollBar>,
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
        self.editor
            .basalt_ref()
            .interval_ref()
            .start(self.state.lock().c_blink_intvl_hid.borrow().unwrap());
    }

    fn pause_cursor_blink(&self) {
        self.editor
            .basalt_ref()
            .interval_ref()
            .pause(self.state.lock().c_blink_intvl_hid.borrow().unwrap());
    }

    fn move_cursor_left(self: &Arc<Self>) {
        let cb_text_area = self.clone();

        self.editor.style_modify_then(
            |style| {
                if let Some(selection) = style.text_body.selection.take() {
                    style.text_body.cursor = selection.start.into();
                    return style.text_body.cursor;
                }

                style.text_body.cursor = match style.text_body.cursor_prev(style.text_body.cursor) {
                    TextCursor::Empty | TextCursor::None => style.text_body.cursor,
                    TextCursor::Position(cursor) => cursor.into(),
                };

                style.text_body.cursor
            },
            move |_editor, bpu, cursor| {
                cb_text_area.check_cursor_in_view(bpu, cursor);
            },
        );

        self.reset_cursor_blink();
    }

    fn move_cursor_right(self: &Arc<Self>) {
        let cb_text_area = self.clone();

        self.editor.style_modify_then(
            |style| {
                if let Some(selection) = style.text_body.selection.take() {
                    style.text_body.cursor = selection.end.into();
                    return style.text_body.cursor;
                }

                style.text_body.cursor = match style.text_body.cursor_next(style.text_body.cursor) {
                    TextCursor::Empty | TextCursor::None => style.text_body.cursor,
                    TextCursor::Position(cursor) => cursor.into(),
                };

                style.text_body.cursor
            },
            move |_editor, bpu, cursor| {
                cb_text_area.check_cursor_in_view(bpu, cursor);
            },
        );

        self.reset_cursor_blink();
    }

    fn move_cursor_up(self: &Arc<Self>) {
        let editor_style = self.editor.style();
        let mut update_required = false;

        let mut cursor = match editor_style.text_body.selection {
            Some(selection) => {
                update_required = true;
                selection.start.into()
            },
            None => editor_style.text_body.cursor,
        };

        match self.editor.text_cursor_up(cursor) {
            TextCursor::None | TextCursor::Empty => (),
            TextCursor::Position(new_cursor) => {
                cursor = new_cursor.into();
                update_required = true;
            },
        }

        if update_required {
            let cb_text_area = self.clone();

            self.editor.style_modify_then(
                |style| {
                    style.text_body.selection = None;
                    style.text_body.cursor = cursor;
                },
                move |_editor, bpu, _| {
                    cb_text_area.check_cursor_in_view(bpu, cursor);
                },
            );
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_down(self: &Arc<Self>) {
        let editor_style = self.editor.style();
        let mut update_required = false;

        let mut cursor = match editor_style.text_body.selection {
            Some(selection) => {
                update_required = true;
                selection.start.into()
            },
            None => editor_style.text_body.cursor,
        };

        match self.editor.text_cursor_down(cursor) {
            TextCursor::None | TextCursor::Empty => (),
            TextCursor::Position(new_cursor) => {
                cursor = new_cursor.into();
                update_required = true;
            },
        }

        if update_required {
            let cb_text_area = self.clone();

            self.editor.style_modify_then(
                |style| {
                    style.text_body.selection = None;
                    style.text_body.cursor = cursor;
                },
                move |_editor, bpu, _| {
                    cb_text_area.check_cursor_in_view(bpu, cursor);
                },
            );
        }

        self.reset_cursor_blink();
    }

    fn check_cursor_in_view(&self, editor_bpu: &BinPostUpdate, cursor: TextCursor) {
        let mut cursor_bounds = match self.editor.get_text_cursor_bounds(cursor) {
            Some(some) => some,
            None => return,
        };

        let editor_size = [
            editor_bpu.optimal_inner_bounds[1] - editor_bpu.optimal_inner_bounds[0],
            editor_bpu.optimal_inner_bounds[3] - editor_bpu.optimal_inner_bounds[2],
        ];

        let text_offset = [
            editor_bpu.optimal_inner_bounds[0] + editor_bpu.content_offset[0],
            editor_bpu.optimal_inner_bounds[2] + editor_bpu.content_offset[1],
        ];

        cursor_bounds[0] -= text_offset[0];
        cursor_bounds[1] -= text_offset[0];
        cursor_bounds[2] -= text_offset[1];
        cursor_bounds[3] -= text_offset[1];

        let target_scroll = [
            self.h_scroll_b.target_scroll(),
            self.v_scroll_b.target_scroll(),
        ];

        let editor_overflow = [
            self.h_scroll_b.target_overflow(),
            self.v_scroll_b.target_overflow(),
        ];

        let scroll_to_x_op = if cursor_bounds[0] - target_scroll[0] - self.theme.spacing < 0.0 {
            Some(cursor_bounds[0] - self.theme.spacing)
        } else if cursor_bounds[1] - target_scroll[0] + self.theme.spacing > editor_size[0] {
            Some(cursor_bounds[1] + self.theme.spacing - editor_size[0])
        } else {
            None
        };

        let scroll_to_y_op = if cursor_bounds[2] - target_scroll[1] - self.theme.spacing < 0.0 {
            Some(cursor_bounds[2] - self.theme.spacing)
        } else if cursor_bounds[3] - target_scroll[1] + self.theme.spacing > editor_size[1] {
            Some(cursor_bounds[3] + self.theme.spacing - editor_size[1])
        } else {
            None
        };

        if let Some(mut scroll_to_x) = scroll_to_x_op {
            scroll_to_x = scroll_to_x.clamp(0.0, editor_overflow[0]);

            if !ulps_eq(scroll_to_x, target_scroll[0], 8) {
                self.h_scroll_b.scroll_to(scroll_to_x);
            }
        }

        if let Some(mut scroll_to_y) = scroll_to_y_op {
            scroll_to_y = scroll_to_y.clamp(0.0, editor_overflow[1]);

            if !ulps_eq(scroll_to_y, target_scroll[1], 8) {
                self.v_scroll_b.scroll_to(scroll_to_y);
            }
        }
    }

    fn style_update(&self, text_body_op: Option<TextBody>) {
        let mut container_style = self.props.placement.clone().into_style();
        container_style.back_color = self.theme.colors.back2;
        let mut editor_style = BinStyle::default();

        if let Some(text_body) = text_body_op {
            editor_style.text_body = text_body;
        }

        editor_style.position = Position::Relative;
        editor_style.pos_from_t = Pixels(0.0);
        editor_style.pos_from_b = ScrollBar::default_placement(&self.theme, ScrollAxis::X).height;
        editor_style.pos_from_l = Pixels(0.0);
        editor_style.pos_from_r = ScrollBar::default_placement(&self.theme, ScrollAxis::Y).width;
        editor_style.text_body.base_attrs.height = Pixels(self.theme.text_height);
        editor_style.text_body.base_attrs.color = self.theme.colors.text1a;
        editor_style.text_body.base_attrs.font_family = self.theme.font_family.clone();
        editor_style.text_body.base_attrs.font_weight = self.theme.font_weight;
        editor_style.back_color = self.theme.colors.back2;
        editor_style.padding_t = Pixels(self.theme.spacing);
        editor_style.padding_b = Pixels(self.theme.spacing);
        editor_style.padding_l = Pixels(self.theme.spacing);
        editor_style.padding_r = Pixels(self.theme.spacing);

        if let Some(border_size) = self.theme.border {
            container_style.border_size_t = Pixels(border_size);
            container_style.border_size_b = Pixels(border_size);
            container_style.border_size_l = Pixels(border_size);
            container_style.border_size_r = Pixels(border_size);
            container_style.border_color_t = self.theme.colors.border1;
            container_style.border_color_b = self.theme.colors.border1;
            container_style.border_color_l = self.theme.colors.border1;
            container_style.border_color_r = self.theme.colors.border1;
        }

        if let Some(border_radius) = self.theme.roundness {
            container_style.border_radius_tl = Pixels(border_radius);
            container_style.border_radius_tr = Pixels(border_radius);
            container_style.border_radius_bl = Pixels(border_radius);
            container_style.border_radius_br = Pixels(border_radius);
            editor_style.border_radius_tl = Pixels(border_radius);
        }

        Bin::style_update_batch([
            (&self.container, container_style),
            (&self.editor, editor_style),
        ]);
    }
}
