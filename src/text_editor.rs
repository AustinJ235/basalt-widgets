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

/// Builder for [`TextEditor`]
pub struct TextEditorBuilder<'a, C> {
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

impl<'a, C> TextEditorBuilder<'a, C>
where
    C: WidgetContainer,
{
    pub(crate) fn with_builder(mut builder: WidgetBuilder<'a, C>) -> Self {
        Self {
            props: Properties::new(
                builder
                    .placement
                    .take()
                    .unwrap_or_else(|| TextEditor::default_placement(&builder.theme)),
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

    /// Finish building the [`TextEditor`].
    pub fn build(self) -> Arc<TextEditor> {
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

        let text_editor = Arc::new(TextEditor {
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

        let cb_text_editor = text_editor.clone();

        text_editor.editor.on_focus(move |_, _| {
            if cb_text_editor.theme.border.is_some() {
                cb_text_editor.editor.style_modify(|style| {
                    style.border_color_t = cb_text_editor.theme.colors.accent1;
                    style.border_color_b = cb_text_editor.theme.colors.accent1;
                    style.border_color_l = cb_text_editor.theme.colors.accent1;
                    style.border_color_r = cb_text_editor.theme.colors.accent1;
                });

                cb_text_editor.start_cursor_blink();
            }

            Default::default()
        });

        let cb_text_editor = text_editor.clone();

        text_editor.editor.on_focus_lost(move |_, _| {
            if cb_text_editor.theme.border.is_some() {
                cb_text_editor.editor.style_modify(|style| {
                    style.border_color_t = cb_text_editor.theme.colors.border1;
                    style.border_color_b = cb_text_editor.theme.colors.border1;
                    style.border_color_l = cb_text_editor.theme.colors.border1;
                    style.border_color_r = cb_text_editor.theme.colors.border1;
                    style.text_body.cursor_color.a = 0.0;
                });

                cb_text_editor.pause_cursor_blink();
            }

            Default::default()
        });

        let cb_text_editor = text_editor.clone();
        let mut consecutive_presses: u8 = 0;
        let mut last_press_op: Option<Instant> = None;

        text_editor
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
                let text_body = cb_text_editor.editor.text_body();
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
                    cb_text_editor.reset_cursor_blink();

                    if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
                        let cb_text_editor2 = cb_text_editor.clone();

                        text_body.bin_on_update(move |_, editor_bpu| {
                            cb_text_editor2.check_cursor_in_view(editor_bpu, cursor_bounds);
                        });
                    }
                }

                Default::default()
            });

        let cb_text_editor = text_editor.clone();
        let mut cursor_visible = false;

        *text_editor.state.lock().c_blink_intvl_hid.borrow_mut() =
            Some(window.basalt_ref().interval_ref().do_every(
                Duration::from_millis(500),
                None,
                move |elapsed| {
                    if elapsed.is_none() {
                        cursor_visible = true;
                    } else {
                        cursor_visible = !cursor_visible;
                    }

                    cb_text_editor.editor.style_modify(|style| {
                        if cursor_visible {
                            style.text_body.cursor_color.a = 1.0;
                        } else {
                            style.text_body.cursor_color.a = 0.0;
                        }
                    });

                    Default::default()
                },
            ));

        let cb_text_editor = text_editor.clone();

        text_editor.editor.on_cursor(move |_, window, _| {
            if !window.is_key_pressed(MouseButton::Left) {
                return Default::default();
            }

            let text_body = cb_text_editor.editor.text_body();

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

            text_editor.editor.on_press(key, move |_, _, _| {
                let mut modifiers = Modifiers::from(&cb_modifiers);
                modifiers |= mask;
                cb_modifiers.store(modifiers.0, atomic::Ordering::SeqCst);
                Default::default()
            });

            let cb_modifiers = modifiers.clone();

            text_editor.editor.on_release(key, move |_, _, _| {
                let mut modifiers = Modifiers::from(&cb_modifiers);
                modifiers &= Modifiers(255) ^ mask;
                cb_modifiers.store(modifiers.0, atomic::Ordering::SeqCst);
                Default::default()
            });
        }

        for key in [
            Qwerty::ArrowLeft,
            Qwerty::ArrowRight,
            Qwerty::ArrowUp,
            Qwerty::ArrowDown,
        ] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key, move |_, window_state, _| {
                cb_text_editor.proc_movement_key(window_state, key);
                Default::default()
            });

            let cb_text_editor = text_editor.clone();
            let cb_modifiers = modifiers.clone();

            window
                .basalt_ref()
                .input_ref()
                .hook()
                .bin(&text_editor.editor)
                .on_hold()
                .keys(key)
                .delay(Some(Duration::from_millis(600)))
                .interval(Duration::from_millis(40))
                .call(move |_, _, _| {
                    cb_text_editor.proc_movement_key(&cb_modifiers, key);
                    Default::default()
                })
                .finish()
                .unwrap();
        }

        for key in [Qwerty::Home, Qwerty::End] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key, move |_, window_state, _| {
                cb_text_editor.proc_movement_key(window_state, key);
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::C], [Qwerty::RCtrl, Qwerty::C]] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key_combo, move |_, _, _| {
                cb_text_editor.copy();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::X], [Qwerty::RCtrl, Qwerty::X]] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key_combo, move |_, _, _| {
                cb_text_editor.cut();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::V], [Qwerty::RCtrl, Qwerty::V]] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key_combo, move |_, _, _| {
                cb_text_editor.paste();
                Default::default()
            });
        }

        for key_combo in [[Qwerty::LCtrl, Qwerty::A], [Qwerty::RCtrl, Qwerty::A]] {
            let cb_text_editor = text_editor.clone();

            text_editor.editor.on_press(key_combo, move |_, _, _| {
                cb_text_editor.select_all();
                Default::default()
            });
        }

        let cb_text_editor = text_editor.clone();

        text_editor.editor.on_character(move |_, window, mut c| {
            let modifiers = Modifiers::from(window);

            if (!c.is_backspace() && modifiers.ctrl()) || modifiers.alt() {
                return Default::default();
            }

            let text_body = cb_text_editor.editor.text_body();
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
                                NextWordLineOp::LineStart
                            } else {
                                NextWordLineOp::WordStart
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
                let cb_text_editor2 = cb_text_editor.clone();

                text_body.bin_on_update(move |_, editor_bpu| {
                    cb_text_editor2.check_cursor_in_view(editor_bpu, cursor_bounds);
                });
            }

            cb_text_editor.reset_cursor_blink();
            Default::default()
        });

        text_editor.style_update(Some(self.text_body));
        text_editor
    }
}

/// TextEditor widget.
pub struct TextEditor {
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

impl TextEditor {
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

    fn proc_movement_key<M>(self: &Arc<Self>, modifiers: M, key: Qwerty)
    where
        M: Into<Modifiers>,
    {
        let modifiers = modifiers.into();
        let text_body = self.editor.text_body();

        if modifiers.shift() {
            if modifiers.ctrl() && matches!(key, Qwerty::ArrowUp | Qwerty::ArrowDown) {
                return;
            }

            match text_body.selection() {
                Some(sel_exist) => {
                    let cur_exist = match text_body.cursor() {
                        TextCursor::None | TextCursor::Empty => {
                            text_body.clear_selection();
                            return;
                        },
                        TextCursor::Position(cursor) => cursor,
                    };

                    let (sel_start, sel_end) = if sel_exist.start == cur_exist {
                        (sel_exist.end, sel_exist.start)
                    } else if sel_exist.end == cur_exist {
                        (sel_exist.start, sel_exist.end)
                    } else {
                        text_body.clear_selection();
                        return;
                    };

                    let cur_move = if modifiers.alt() { sel_start } else { sel_end };

                    let cur_next = if modifiers.ctrl() {
                        let next_op = match key {
                            Qwerty::ArrowLeft => Some(NextWordLineOp::WordStart),
                            Qwerty::ArrowRight => Some(NextWordLineOp::WordEnd),
                            Qwerty::Home | Qwerty::End => None,
                            Qwerty::ArrowUp | Qwerty::ArrowDown => unreachable!(),
                            _ => return,
                        };

                        match next_op {
                            Some(next_op) => cursor_next_word_line(&text_body, cur_move, next_op),
                            None => {
                                let sel_all = match text_body.select_all() {
                                    Some(selection) => selection,
                                    None => {
                                        text_body.clear_selection();
                                        return;
                                    },
                                };

                                match key {
                                    Qwerty::Home => sel_all.start,
                                    Qwerty::End => sel_all.end,
                                    _ => unreachable!(),
                                }
                            },
                        }
                    } else {
                        let next_op = match key {
                            Qwerty::Home => Some(NextWordLineOp::LineStart),
                            Qwerty::End => Some(NextWordLineOp::LineEnd),
                            Qwerty::ArrowLeft
                            | Qwerty::ArrowRight
                            | Qwerty::ArrowUp
                            | Qwerty::ArrowDown => None,
                            _ => return,
                        };

                        match next_op {
                            Some(next_op) => cursor_next_word_line(&text_body, cur_move, next_op),
                            None => {
                                match match key {
                                    Qwerty::ArrowLeft => text_body.cursor_prev(cur_move.into()),
                                    Qwerty::ArrowRight => text_body.cursor_next(cur_move.into()),
                                    Qwerty::ArrowUp => text_body.cursor_up(cur_move.into(), true),
                                    Qwerty::ArrowDown => {
                                        text_body.cursor_down(cur_move.into(), true)
                                    },
                                    _ => unreachable!(),
                                } {
                                    TextCursor::None | TextCursor::Empty => return,
                                    TextCursor::Position(cursor) => cursor,
                                }
                            },
                        }
                    };

                    if text_body.are_cursors_equivalent(cur_move.into(), cur_next.into()) {
                        return;
                    }

                    if modifiers.alt() {
                        if text_body.are_cursors_equivalent(sel_end.into(), cur_next.into()) {
                            text_body.clear_selection();
                        } else {
                            text_body.set_selection(TextSelection::unordered(sel_end, cur_next));
                        }
                    } else {
                        if text_body.are_cursors_equivalent(sel_start.into(), cur_next.into()) {
                            text_body.clear_selection();
                            text_body.set_cursor(sel_start.into());
                        } else {
                            text_body.set_selection(TextSelection::unordered(sel_start, cur_next));
                            text_body.set_cursor(cur_next.into());
                        }
                    }
                },
                None => {
                    let sel_start = match text_body.cursor() {
                        TextCursor::None => return,
                        TextCursor::Empty => {
                            match text_body.select_all() {
                                Some(sel_all) => sel_all.start,
                                None => return,
                            }
                        },
                        TextCursor::Position(cursor) => cursor,
                    };

                    let sel_end = if modifiers.ctrl() {
                        match key {
                            Qwerty::ArrowLeft => {
                                cursor_next_word_line(
                                    &text_body,
                                    sel_start,
                                    NextWordLineOp::WordStart,
                                )
                            },
                            Qwerty::ArrowRight => {
                                cursor_next_word_line(
                                    &text_body,
                                    sel_start,
                                    NextWordLineOp::WordEnd,
                                )
                            },
                            Qwerty::Home => {
                                match text_body.select_all() {
                                    Some(sel_all) => sel_all.start,
                                    None => return,
                                }
                            },
                            Qwerty::End => {
                                match text_body.select_all() {
                                    Some(sel_all) => sel_all.end,
                                    None => return,
                                }
                            },
                            Qwerty::ArrowUp | Qwerty::ArrowDown => unreachable!(),
                            _ => return,
                        }
                    } else {
                        match match key {
                            Qwerty::ArrowLeft => text_body.cursor_prev(sel_start.into()),
                            Qwerty::ArrowRight => text_body.cursor_next(sel_start.into()),
                            Qwerty::ArrowUp => text_body.cursor_up(sel_start.into(), true),
                            Qwerty::ArrowDown => text_body.cursor_down(sel_start.into(), true),
                            Qwerty::Home => text_body.cursor_line_start(sel_start.into(), true),
                            Qwerty::End => text_body.cursor_line_end(sel_start.into(), true),
                            _ => return,
                        } {
                            TextCursor::None | TextCursor::Empty => return,
                            TextCursor::Position(cursor) => cursor,
                        }
                    };

                    if text_body.are_cursors_equivalent(sel_start.into(), sel_end.into()) {
                        return;
                    }

                    text_body.set_cursor(sel_end.into());
                    text_body.set_selection(TextSelection::unordered(sel_start, sel_end));
                },
            }
        } else if modifiers.ctrl() {
            match text_body.selection() {
                Some(selection) => {
                    let cursor = match key {
                        Qwerty::ArrowLeft => selection.start,
                        Qwerty::ArrowRight => selection.end,
                        Qwerty::ArrowUp => {
                            match text_body.cursor_up(selection.start.into(), true) {
                                TextCursor::None | TextCursor::Empty => selection.start,
                                TextCursor::Position(cursor) => cursor,
                            }
                        },
                        Qwerty::ArrowDown => {
                            match text_body.cursor_down(selection.end.into(), true) {
                                TextCursor::None | TextCursor::Empty => selection.end,
                                TextCursor::Position(cursor) => cursor,
                            }
                        },
                        Qwerty::Home => {
                            match text_body.select_all() {
                                Some(sel_all) => sel_all.start,
                                None => return,
                            }
                        },
                        Qwerty::End => {
                            match text_body.select_all() {
                                Some(sel_all) => sel_all.end,
                                None => return,
                            }
                        },
                        _ => unreachable!(),
                    };

                    text_body.set_cursor(cursor.into());
                    text_body.clear_selection();
                },
                None => {
                    match key {
                        Qwerty::ArrowLeft | Qwerty::ArrowRight => {
                            match text_body.cursor() {
                                TextCursor::None => return,
                                TextCursor::Empty => {
                                    match text_body.select_all() {
                                        Some(sel_all) => {
                                            text_body.set_cursor(sel_all.start.into());
                                        },
                                        None => return,
                                    }
                                },
                                TextCursor::Position(cursor) => {
                                    let cursor_next = cursor_next_word_line(
                                        &text_body,
                                        cursor,
                                        if key == Qwerty::ArrowLeft {
                                            NextWordLineOp::WordStart
                                        } else {
                                            NextWordLineOp::WordEnd
                                        },
                                    );

                                    if text_body
                                        .are_cursors_equivalent(cursor.into(), cursor_next.into())
                                    {
                                        return;
                                    }

                                    text_body.set_cursor(cursor_next.into());
                                },
                            }
                        },
                        Qwerty::ArrowUp | Qwerty::ArrowDown => {
                            let line_height = (self.theme.text_height * 1.2).round();
                            self.v_scroll_b.scroll(
                                if key == Qwerty::ArrowUp {
                                    -line_height
                                } else {
                                    line_height
                                },
                            );
                        },
                        Qwerty::Home => {
                            match text_body.select_all() {
                                Some(sel_all) => {
                                    text_body.set_cursor(sel_all.start.into());
                                },
                                None => return,
                            }
                        },
                        Qwerty::End => {
                            match text_body.select_all() {
                                Some(sel_all) => {
                                    text_body.set_cursor(sel_all.end.into());
                                },
                                None => return,
                            }
                        },
                        _ => return,
                    }
                },
            }
        } else {
            match text_body.selection() {
                Some(selection) => {
                    let cursor = match key {
                        Qwerty::ArrowLeft | Qwerty::ArrowUp | Qwerty::Home => selection.start,
                        Qwerty::ArrowRight | Qwerty::ArrowDown | Qwerty::End => selection.end,
                        _ => return,
                    };

                    text_body.set_cursor(cursor.into());
                    text_body.clear_selection();
                },
                None => {
                    let cursor = match match key {
                        Qwerty::ArrowLeft => text_body.cursor_prev(text_body.cursor()),
                        Qwerty::ArrowRight => text_body.cursor_next(text_body.cursor()),
                        Qwerty::ArrowUp => text_body.cursor_up(text_body.cursor(), true),
                        Qwerty::ArrowDown => text_body.cursor_down(text_body.cursor(), true),
                        Qwerty::Home => text_body.cursor_line_start(text_body.cursor(), true),
                        Qwerty::End => text_body.cursor_line_end(text_body.cursor(), true),
                        _ => return,
                    } {
                        TextCursor::None | TextCursor::Empty => return,
                        position => position,
                    };

                    text_body.set_cursor(cursor);
                },
            }
        }

        if let Some(cursor_bounds) = text_body.cursor_bounds(text_body.cursor()) {
            let text_editor = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_editor.check_cursor_in_view(editor_bpu, cursor_bounds);
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
                let text_editor = self.clone();

                text_body.bin_on_update(move |_, editor_bpu| {
                    text_editor.check_cursor_in_view(editor_bpu, cursor_bounds);
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
            let text_editor = self.clone();

            text_body.bin_on_update(move |_, editor_bpu| {
                text_editor.check_cursor_in_view(editor_bpu, cursor_bounds);
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

enum NextWordLineOp {
    WordStart,
    WordEnd,
    LineStart,
    LineEnd,
}

fn cursor_next_word_line(
    text_body: &TextBodyGuard,
    cursor: PosTextCursor,
    op: NextWordLineOp,
) -> PosTextCursor {
    let edge = match match op {
        NextWordLineOp::WordStart => text_body.cursor_word_start(cursor.into()),
        NextWordLineOp::WordEnd => text_body.cursor_word_end(cursor.into()),
        NextWordLineOp::LineStart => text_body.cursor_line_start(cursor.into(), true),
        NextWordLineOp::LineEnd => text_body.cursor_line_end(cursor.into(), true),
    } {
        TextCursor::None | TextCursor::Empty => return cursor,
        TextCursor::Position(cursor) => cursor,
    };

    if !text_body.are_cursors_equivalent(cursor.into(), edge.into()) {
        return edge;
    }

    let next = match match op {
        NextWordLineOp::WordStart | NextWordLineOp::LineStart => {
            text_body.cursor_prev(cursor.into())
        },
        NextWordLineOp::WordEnd | NextWordLineOp::LineEnd => text_body.cursor_next(cursor.into()),
    } {
        TextCursor::None | TextCursor::Empty => return edge,
        TextCursor::Position(cursor) => cursor,
    };

    match match op {
        NextWordLineOp::WordStart => text_body.cursor_word_start(next.into()),
        NextWordLineOp::WordEnd => text_body.cursor_word_end(next.into()),
        NextWordLineOp::LineStart => text_body.cursor_line_start(next.into(), true),
        NextWordLineOp::LineEnd => text_body.cursor_line_end(next.into(), true),
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
