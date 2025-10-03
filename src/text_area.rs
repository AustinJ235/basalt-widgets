use std::cell::RefCell;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};
use std::sync::Arc;
use std::sync::atomic::{self, AtomicU8};
use std::time::{Duration, Instant};

use basalt::input::{MouseButton, Qwerty, WindowState};
use basalt::interface::UnitValue::Pixels;
use basalt::interface::{
    Bin, BinPostUpdate, BinStyle, PosTextCursor, Position, TextAttrs, TextBody, TextBodyGuard,
    TextCursor, TextSelection, TextSpan,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
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

        let cb_text_area = text_area.clone();
        let mut consecutive_presses: u8 = 0;
        let mut last_press_op: Option<Instant> = None;

        text_area
            .editor
            .on_press(MouseButton::Left, move |_, window, _| {
                let modifiers = Modifiers::from(window);

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
                let text_body = cb_text_area.editor.text_body();
                let cursor = text_body.get_cursor(window.cursor_pos());

                if !matches!(cursor, TextCursor::Position(..)) {
                    return Default::default();
                }

                match consecutive_presses {
                    1 => {
                        if modifiers.shift() {
                            match text_body.selection() {
                                Some(existing_selection) => {
                                    let sel_s = match text_body.cursor() {
                                        TextCursor::None | TextCursor::Empty => {
                                            existing_selection.start
                                        },
                                        TextCursor::Position(existing_cursor) => {
                                            if existing_cursor == existing_selection.start {
                                                existing_selection.end
                                            } else {
                                                existing_selection.start
                                            }
                                        },
                                    };

                                    text_body.set_selection(TextSelection::unordered(
                                        sel_s,
                                        cursor.into_position().unwrap(),
                                    ))
                                },
                                None => {
                                    match text_body.cursor() {
                                        TextCursor::None | TextCursor::Empty => (),
                                        TextCursor::Position(sel_s) => {
                                            text_body.set_selection(TextSelection::unordered(
                                                sel_s,
                                                cursor.into_position().unwrap(),
                                            ));
                                        },
                                    }
                                },
                            }

                            text_body.set_cursor(cursor);
                        } else {
                            text_body.set_cursor(cursor);
                            text_body.clear_selection();
                        }
                    },
                    2 | 3 => {
                        match match consecutive_presses {
                            2 => text_body.cursor_select_word(cursor),
                            3 => text_body.cursor_select_line(cursor, true),
                            _ => unreachable!(),
                        } {
                            Some(selection) => {
                                if modifiers.shift() {
                                    match text_body.selection() {
                                        Some(existing_selection) => {
                                            text_body.set_selection(TextSelection {
                                                start: existing_selection
                                                    .start
                                                    .min(selection.start),
                                                end: existing_selection.end.max(selection.end),
                                            });

                                            if selection.start > existing_selection.start {
                                                text_body.set_cursor(selection.end.into());
                                            } else {
                                                text_body.set_cursor(selection.start.into());
                                            }
                                        },
                                        None => {
                                            text_body.set_cursor(selection.end.into());
                                            text_body.set_selection(selection);
                                        },
                                    }
                                } else {
                                    text_body.set_cursor(selection.end.into());
                                    text_body.set_selection(selection);
                                }
                            },
                            None => {
                                text_body.set_cursor(cursor);
                                text_body.clear_selection();
                            },
                        }
                    },
                    0 | 4.. => unreachable!(),
                }

                if matches!(text_body.cursor(), TextCursor::Position(..)) {
                    cb_text_area.reset_cursor_blink();

                    if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
                        let cb_text_area2 = cb_text_area.clone();

                        text_body.bin_on_update(move |_, editor_bpu| {
                            cb_text_area2.check_cursor_in_view(editor_bpu, cursor_bounds);
                        });
                    }
                }

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

        text_area.editor.on_cursor(move |_, window, _| {
            if !window.is_key_pressed(MouseButton::Left) {
                return Default::default();
            }

            let text_body = cb_text_area.editor.text_body();

            let cursor = match text_body.cursor() {
                TextCursor::None | TextCursor::Empty => return Default::default(),
                TextCursor::Position(cursor) => cursor,
            };

            let sel_s = match text_body.selection() {
                Some(selection) => {
                    if selection.start == cursor {
                        selection.end
                    } else {
                        selection.start
                    }
                },
                None => cursor,
            };

            let sel_e = match text_body.get_cursor(window.cursor_pos()) {
                TextCursor::None | TextCursor::Empty => {
                    text_body.set_cursor(TextCursor::None);
                    text_body.clear_selection();
                    return Default::default();
                },
                TextCursor::Position(cursor) => cursor,
            };

            text_body.set_cursor(sel_e.into());

            if sel_s == sel_e {
                text_body.clear_selection();
            } else {
                text_body.set_selection(TextSelection::unordered(sel_s, sel_e));
            }

            Default::default()
        });

        let modifiers = Arc::new(AtomicU8::new(0));

        for (key, mask) in [
            (Qwerty::LShift, Modifiers::LEFT_SHIFT),
            (Qwerty::RShift, Modifiers::RIGHT_SHIFT),
            (Qwerty::LCtrl, Modifiers::LEFT_CTRL),
            (Qwerty::RCtrl, Modifiers::RIGHT_CTRL),
            (Qwerty::LAlt, Modifiers::LEFT_ALT),
            (Qwerty::RAlt, Modifiers::RIGHT_ALT),
        ] {
            let cb_modifiers = modifiers.clone();

            text_area.editor.on_press(key, move |_, _, _| {
                let mut modifiers = Modifiers::from(&cb_modifiers);
                modifiers |= mask;
                cb_modifiers.store(modifiers.0, atomic::Ordering::SeqCst);
                Default::default()
            });

            let cb_modifiers = modifiers.clone();

            text_area.editor.on_release(key, move |_, _, _| {
                let mut modifiers = Modifiers::from(&cb_modifiers);
                modifiers &= Modifiers(255) ^ mask;
                cb_modifiers.store(modifiers.0, atomic::Ordering::SeqCst);
                Default::default()
            });
        }

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowLeft, move |_, window_state, _| {
                cb_text_area.move_cursor_direction(window_state, Direction::Left);
                Default::default()
            });

        let cb_text_area = text_area.clone();
        let cb_modifiers = modifiers.clone();

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
                cb_text_area.move_cursor_direction(&cb_modifiers, Direction::Left);
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowRight, move |_, window_state, _| {
                cb_text_area.move_cursor_direction(window_state, Direction::Right);
                Default::default()
            });

        let cb_text_area = text_area.clone();
        let cb_modifiers = modifiers.clone();

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
                cb_text_area.move_cursor_direction(&cb_modifiers, Direction::Right);
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowUp, move |_, window_state, _| {
                cb_text_area.move_cursor_direction(window_state, Direction::Up);
                Default::default()
            });

        let cb_text_area = text_area.clone();
        let cb_modifiers = modifiers.clone();

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
                cb_text_area.move_cursor_direction(&cb_modifiers, Direction::Up);
                Default::default()
            })
            .finish()
            .unwrap();

        let cb_text_area = text_area.clone();

        text_area
            .editor
            .on_press(Qwerty::ArrowDown, move |_, window_state, _| {
                cb_text_area.move_cursor_direction(window_state, Direction::Down);
                Default::default()
            });

        let cb_text_area = text_area.clone();
        let cb_modifiers = modifiers;

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
                cb_text_area.move_cursor_direction(&cb_modifiers, Direction::Down);
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
            let modifiers = Modifiers::from(window);

            if (!c.is_backspace() && modifiers.ctrl()) || modifiers.alt() {
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
                    if modifiers.ctrl() {
                        let delete_end = match text_body.cursor() {
                            TextCursor::None | TextCursor::Empty => return Default::default(),
                            TextCursor::Position(cursor) => cursor,
                        };

                        let delete_start = cursor_next_word_line(
                            &text_body,
                            delete_end,
                            if modifiers.shift() {
                                Direction::Up
                            } else {
                                Direction::Left
                            },
                        );

                        if delete_end == delete_start {
                            return Default::default();
                        }

                        text_body.set_cursor(text_body.selection_delete(TextSelection {
                            start: delete_start,
                            end: delete_end,
                        }));
                    } else {
                        text_body.set_cursor(text_body.cursor_delete(text_body.cursor()));
                    }
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
                    cb_text_area2.check_cursor_in_view(editor_bpu, cursor_bounds);
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

    fn move_cursor_direction<M>(self: &Arc<Self>, modifiers: M, direction: Direction)
    where
        M: Into<Modifiers>,
    {
        let modifiers = modifiers.into();
        let text_body = self.editor.text_body();

        let cursor_direction = |cursor: TextCursor| {
            match direction {
                Direction::Left => text_body.cursor_prev(cursor),
                Direction::Right => text_body.cursor_next(cursor),
                Direction::Up => text_body.cursor_up(cursor, true),
                Direction::Down => text_body.cursor_down(cursor, true),
            }
        };

        if modifiers.shift() {
            let cursor = match text_body.cursor() {
                TextCursor::None | TextCursor::Empty => return,
                TextCursor::Position(cursor) => cursor,
            };

            let selection = match text_body.selection() {
                Some(selection) => selection,
                None => {
                    if !modifiers.alt() {
                        let sel_s = match match direction {
                            Direction::Left => text_body.cursor_word_start(cursor.into()),
                            Direction::Right => text_body.cursor_word_end(cursor.into()),
                            Direction::Up => text_body.cursor_line_start(cursor.into(), true),
                            Direction::Down => text_body.cursor_line_end(cursor.into(), true),
                        } {
                            TextCursor::None | TextCursor::Empty => return,
                            TextCursor::Position(cursor) => cursor,
                        };

                        text_body.set_selection(TextSelection::unordered(sel_s, cursor));
                        text_body.set_cursor(sel_s.into());
                    }

                    return;
                },
            };

            let (sel_s, mut sel_e) = if modifiers.alt() == (selection.start == cursor) {
                (selection.start, selection.end)
            } else {
                (selection.end, selection.start)
            };

            sel_e = if modifiers.ctrl() {
                cursor_next_word_line(&text_body, sel_e, direction)
            } else {
                match cursor_direction(sel_e.into()) {
                    TextCursor::None | TextCursor::Empty => return,
                    TextCursor::Position(cursor) => cursor,
                }
            };

            text_body.set_selection(TextSelection::unordered(sel_s, sel_e));

            if modifiers.alt() {
                text_body.set_cursor(sel_s.into());
            } else {
                text_body.set_cursor(sel_e.into())
            }

            self.reset_cursor_blink();
            return;
        } else if modifiers.ctrl() {
            if matches!(direction, Direction::Left | Direction::Right) {
                let mut cursor = match text_body.selection() {
                    Some(selection) => {
                        if direction == Direction::Left {
                            selection.start
                        } else {
                            selection.end
                        }
                    },
                    None => {
                        match text_body.cursor() {
                            TextCursor::None | TextCursor::Empty => return,
                            TextCursor::Position(cursor) => cursor,
                        }
                    },
                };

                cursor = cursor_next_word_line(&text_body, cursor, direction);
                text_body.set_cursor(cursor.into());
                text_body.clear_selection();
                self.reset_cursor_blink();
            } else {
                let amt = (self.theme.text_height * 1.2).round();

                self.v_scroll_b.scroll(
                    if direction == Direction::Up {
                        -amt
                    } else {
                        amt
                    },
                );
            }

            return;
        }

        match text_body.selection() {
            Some(selection) => {
                text_body.clear_selection();

                text_body.set_cursor(match direction {
                    Direction::Left | Direction::Up => selection.start.into(),
                    Direction::Right | Direction::Down => selection.end.into(),
                });
            },
            None => {
                if let TextCursor::Position(cursor) = cursor_direction(text_body.cursor()) {
                    text_body.set_cursor(cursor.into());
                }
            },
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_area = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_area.check_cursor_in_view(editor_bpu, cursor_bounds);
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
                text_area.check_cursor_in_view(editor_bpu, cursor_bounds);
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
                text_area.check_cursor_in_view(editor_bpu, cursor_bounds);
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
                    text_area.check_cursor_in_view(editor_bpu, cursor_bounds);
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
                text_area.check_cursor_in_view(editor_bpu, cursor_bounds);
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

    fn check_cursor_in_view(&self, editor_bpu: &BinPostUpdate, mut cursor_bounds: [f32; 4]) {
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

fn cursor_next_word_line(
    text_body: &TextBodyGuard,
    cursor: PosTextCursor,
    direction: Direction,
) -> PosTextCursor {
    let edge = match match direction {
        Direction::Left => text_body.cursor_word_start(cursor.into()),
        Direction::Right => text_body.cursor_word_end(cursor.into()),
        Direction::Up => text_body.cursor_line_start(cursor.into(), true),
        Direction::Down => text_body.cursor_line_end(cursor.into(), true),
    } {
        TextCursor::None | TextCursor::Empty => return cursor,
        TextCursor::Position(cursor) => cursor,
    };

    if !text_body.are_cursors_equivalent(cursor.into(), edge.into()) {
        return edge;
    }

    let next = match match direction {
        Direction::Left | Direction::Up => text_body.cursor_prev(cursor.into()),
        Direction::Right | Direction::Down => text_body.cursor_next(cursor.into()),
    } {
        TextCursor::None | TextCursor::Empty => return edge,
        TextCursor::Position(cursor) => cursor,
    };

    match match direction {
        Direction::Left => text_body.cursor_word_start(next.into()),
        Direction::Right => text_body.cursor_word_end(next.into()),
        Direction::Up => text_body.cursor_line_start(next.into(), true),
        Direction::Down => text_body.cursor_line_end(next.into(), true),
    } {
        TextCursor::None | TextCursor::Empty => next,
        TextCursor::Position(cursor) => cursor,
    }
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const LEFT_ALT: Self = Self(0b00001000);
    pub const LEFT_CTRL: Self = Self(0b00100000);
    pub const LEFT_SHIFT: Self = Self(0b10000000);
    pub const RIGHT_ALT: Self = Self(0b00000100);
    pub const RIGHT_CTRL: Self = Self(0b00010000);
    pub const RIGHT_SHIFT: Self = Self(0b01000000);

    pub fn shift(self) -> bool {
        self & Self::LEFT_SHIFT == Self::LEFT_SHIFT || self & Self::RIGHT_SHIFT == Self::RIGHT_SHIFT
    }

    pub fn ctrl(self) -> bool {
        self & Self::LEFT_CTRL == Self::LEFT_CTRL || self & Self::RIGHT_CTRL == Self::RIGHT_CTRL
    }

    pub fn alt(self) -> bool {
        self & Self::LEFT_ALT == Self::LEFT_ALT || self & Self::RIGHT_ALT == Self::RIGHT_ALT
    }
}

impl From<&Arc<AtomicU8>> for Modifiers {
    fn from(atomic: &Arc<AtomicU8>) -> Self {
        Self(atomic.load(atomic::Ordering::SeqCst))
    }
}

impl From<&WindowState> for Modifiers {
    fn from(window_state: &WindowState) -> Self {
        let mut modifiers = Self(0);

        if window_state.is_key_pressed(Qwerty::LShift) {
            modifiers |= Self::LEFT_SHIFT;
        }

        if window_state.is_key_pressed(Qwerty::RShift) {
            modifiers |= Self::RIGHT_SHIFT;
        }

        if window_state.is_key_pressed(Qwerty::LCtrl) {
            modifiers |= Self::LEFT_CTRL;
        }

        if window_state.is_key_pressed(Qwerty::RCtrl) {
            modifiers |= Self::RIGHT_CTRL;
        }

        if window_state.is_key_pressed(Qwerty::LAlt) {
            modifiers |= Self::LEFT_ALT;
        }

        if window_state.is_key_pressed(Qwerty::RAlt) {
            modifiers |= Self::RIGHT_ALT;
        }

        modifiers
    }
}

impl BitAnd for Modifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Modifiers {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = Self(self.0 & rhs.0);
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = Self(self.0 | rhs.0);
    }
}

impl BitXor for Modifiers {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for Modifiers {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = Self(self.0 ^ rhs.0);
    }
}
