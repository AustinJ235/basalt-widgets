use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::time::{Duration, Instant};

use basalt::input::{MouseButton, Qwerty};
use basalt::interface::UnitValue::Pixels;
use basalt::interface::{
    Bin, BinPostUpdate, BinStyle, Position, TextAttrs, TextBody, TextCursor, TextSelection,
    TextSpan,
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

    /// Set the inital text.
    pub fn with_text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        if self.text_body.spans.is_empty() {
            self.text_body.spans.push(TextSpan::from(text.into()));
        } else {
            self.text_body.spans[0] = TextSpan::from(text.into());
        }

        self
    }

    /// Set the [`TextAttrs`] used.
    pub fn with_attrs(mut self, attrs: TextAttrs) -> Self {
        self.text_body.base_attrs = attrs;
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
                clipboard: RefCell::new(String::new()),
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
        let mut consecutive_presses: u8 = 0;
        let mut last_press_op: Option<Instant> = None;

        text_area
            .editor
            .on_press(MouseButton::Left, move |_, window, _| {
                match last_press_op {
                    Some(last_press) => {
                        if last_press.elapsed() <= Duration::from_millis(300) {
                            consecutive_presses += 1;

                            if consecutive_presses > 3 {
                                consecutive_presses = 1;
                            }
                        } else {
                            consecutive_presses = 1;
                        }
                    },
                    None => {
                        consecutive_presses = 1;
                    },
                }

                last_press_op = Some(Instant::now());

                let extends_selection =
                    window.is_key_pressed(Qwerty::LShift) || window.is_key_pressed(Qwerty::RShift);

                let text_body = cb_text_area.editor.text_body();
                let cursor = text_body.get_cursor(window.cursor_pos());

                if !matches!(cursor, TextCursor::Position(..)) {
                    return Default::default();
                }

                match consecutive_presses {
                    1 => {
                        if extends_selection {
                            match text_body.selection() {
                                Some(existing_selection) => {
                                    text_body.set_selection(existing_selection.extend(cursor));
                                },
                                None => {
                                    text_body.set_cursor(cursor);
                                },
                            }
                        } else {
                            text_body.clear_selection();
                            text_body.set_cursor(cursor);
                        }
                    },
                    2 | 3 => {
                        match match consecutive_presses {
                            2 => text_body.cursor_select_word(cursor),
                            3 => text_body.cursor_select_line(cursor, true),
                            _ => unreachable!(),
                        } {
                            Some(selection) => {
                                if extends_selection {
                                    match text_body.selection() {
                                        Some(existing_selection) => {
                                            text_body.set_selection(
                                                selection.extend(existing_selection),
                                            );
                                        },
                                        None => {
                                            text_body.set_selection(selection);
                                        },
                                    }
                                } else {
                                    text_body.set_selection(selection);
                                }
                            },
                            None => {
                                text_body.clear_selection();
                                text_body.set_cursor(cursor);
                            },
                        }
                    },
                    0 | 4.. => unreachable!(),
                }

                if matches!(text_body.cursor(), TextCursor::Position(..)) {
                    cb_text_area.reset_cursor_blink();
                    cb_selecting.store(true, atomic::Ordering::Relaxed);

                    if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
                        let cb_text_area2 = cb_text_area.clone();

                        text_body.bin_on_update(move |_, editor_bpu| {
                            cb_text_area2.check_cursor_in_view2(editor_bpu, cursor_bounds);
                        });
                    }
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

            let text_body = cb_text_area.editor.text_body();

            let select_from = match text_body.cursor() {
                TextCursor::None | TextCursor::Empty => return Default::default(),
                TextCursor::Position(cursor) => cursor,
            };

            let select_to = match text_body.get_cursor(window.cursor_pos()) {
                TextCursor::None | TextCursor::Empty => {
                    text_body.clear_selection();
                    return Default::default();
                },
                TextCursor::Position(cursor) => cursor,
            };

            if select_from == select_to {
                text_body.clear_selection();
            } else if select_from < select_to {
                text_body.set_selection(TextSelection {
                    start: select_from,
                    end: select_to,
                });
            } else {
                text_body.set_selection(TextSelection {
                    start: select_to,
                    end: select_from,
                });
            }

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

        text_area.editor.on_press(Qwerty::Home, move |_, _, _| {
            cb_text_area.move_cursor_sol();
            Default::default()
        });

        let cb_text_area = text_area.clone();

        text_area.editor.on_press(Qwerty::End, move |_, _, _| {
            cb_text_area.move_cursor_eol();
            Default::default()
        });

        for key_combo in [[Qwerty::LCtrl, Qwerty::C], [Qwerty::RCtrl, Qwerty::C]] {
            let cb_text_area = text_area.clone();

            text_area.editor.on_press(key_combo, move |_, _, _| {
                cb_text_area.copy();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::X], [Qwerty::RCtrl, Qwerty::X]] {
            let cb_text_area = text_area.clone();

            text_area.editor.on_press(key_combo, move |_, _, _| {
                cb_text_area.cut();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::V], [Qwerty::RCtrl, Qwerty::V]] {
            let cb_text_area = text_area.clone();

            text_area.editor.on_press(key_combo, move |_, _, _| {
                cb_text_area.paste();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::A], [Qwerty::RCtrl, Qwerty::A]] {
            let cb_text_area = text_area.clone();

            text_area.editor.on_press(key_combo, move |_, _, _| {
                cb_text_area.select_all();
                Default::default()
            });
        }

        let cb_text_area = text_area.clone();

        text_area.editor.on_character(move |_, window, mut c| {
            if window.is_key_pressed(Qwerty::LCtrl)
                || window.is_key_pressed(Qwerty::LAlt)
                || window.is_key_pressed(Qwerty::RCtrl)
                || window.is_key_pressed(Qwerty::RAlt)
            {
                return Default::default();
            }

            let text_body = cb_text_area.editor.text_body();
            let mut selection_deleted = false;

            if let Some(selection) = text_body.selection() {
                text_body.clear_selection();
                text_body.set_cursor(text_body.selection_delete(selection));
                selection_deleted = true;
            }

            if c.is_backspace() {
                if !selection_deleted {
                    text_body.set_cursor(text_body.cursor_delete(text_body.cursor()));
                }
            } else {
                if c.0 == '\r' {
                    c.0 = '\n';
                }

                text_body.set_cursor(text_body.cursor_insert(text_body.cursor(), *c));
            }

            if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
                let cb_text_area2 = cb_text_area.clone();

                text_body.bin_on_update(move |_, editor_bpu| {
                    cb_text_area2.check_cursor_in_view2(editor_bpu, cursor_bounds);
                });
            }

            cb_text_area.reset_cursor_blink();
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
    editor: Arc<Bin>,
    v_scroll_b: Arc<ScrollBar>,
    h_scroll_b: Arc<ScrollBar>,
    state: ReentrantMutex<State>,
}

struct State {
    c_blink_intvl_hid: RefCell<Option<IntvlHookID>>,
    clipboard: RefCell<String>,
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
        let text_body = self.editor.text_body();

        match text_body.selection() {
            Some(selection) => {
                text_body.clear_selection();
                text_body.set_cursor(selection.start.into());
            },
            None => {
                let cursor_prev = text_body.cursor_prev(text_body.cursor());

                if matches!(cursor_prev, TextCursor::Position(..)) {
                    text_body.set_cursor(cursor_prev);
                }
            },
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_right(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        match text_body.selection() {
            Some(selection) => {
                text_body.clear_selection();
                text_body.set_cursor(selection.start.into());
            },
            None => {
                let cursor_next = text_body.cursor_next(text_body.cursor());

                if matches!(cursor_next, TextCursor::Position(..)) {
                    text_body.set_cursor(cursor_next);
                }
            },
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_up(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            text_body.clear_selection();
            text_body.set_cursor(selection.start.into());
        }

        let cursor_up = text_body.cursor_up(text_body.cursor(), true);

        if matches!(cursor_up, TextCursor::Position(..)) {
            text_body.set_cursor(cursor_up);
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_down(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            text_body.clear_selection();
            text_body.set_cursor(selection.end.into());
        }

        let cursor_down = text_body.cursor_down(text_body.cursor(), true);

        if matches!(cursor_down, TextCursor::Position(..)) {
            text_body.set_cursor(cursor_down);
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_sol(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            text_body.clear_selection();
            text_body.set_cursor(selection.start.into());
        }

        let cursor_sol = text_body.cursor_line_start(text_body.cursor(), true);

        if matches!(cursor_sol, TextCursor::Position(..)) {
            text_body.set_cursor(cursor_sol);
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn move_cursor_eol(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            text_body.clear_selection();
            text_body.set_cursor(selection.end.into());
        }

        let cursor_eol = text_body.cursor_line_end(text_body.cursor(), true);

        if matches!(cursor_eol, TextCursor::Position(..)) {
            text_body.set_cursor(cursor_eol);
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn copy(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            *self.state.lock().clipboard.borrow_mut() = text_body.selection_string(selection);
        }
    }

    fn cut(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            let (cursor, selection_value) = text_body.selection_take_string(selection);
            text_body.clear_selection();
            text_body.set_cursor(cursor);
            *self.state.lock().clipboard.borrow_mut() = selection_value;

            if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
                let text_area = self.clone();

                text_body.bin_on_update(move |_, editor_bpu| {
                    text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
                });
            }
        }

        self.reset_cursor_blink();
    }

    fn paste(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.selection() {
            text_body.clear_selection();
            text_body.set_cursor(text_body.selection_delete(selection));
        }

        text_body.set_cursor(text_body.cursor_insert_str(
            text_body.cursor(),
            self.state.lock().clipboard.borrow().clone(),
        ));

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view2(editor_bpu, cursor_bounds);
            });
        }

        self.reset_cursor_blink();
    }

    fn select_all(self: &Arc<Self>) {
        let text_body = self.editor.text_body();

        if let Some(selection) = text_body.select_all() {
            text_body.set_selection(selection);
        }
    }

    fn check_cursor_in_view2(&self, editor_bpu: &BinPostUpdate, mut cursor_bounds: [f32; 4]) {
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
