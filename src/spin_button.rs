use std::cell::RefCell;
use std::sync::Arc;

use basalt::input::{Qwerty, WindowState};
use basalt::interface::{
    Bin, BinPosition, BinStyle, BinVert, Color, TextHoriAlign, TextVertAlign, TextWrap,
};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::button::button_hooks;
use crate::{Theme, WidgetParent};

/// Builder for [`SpinButton`]
pub struct SpinButtonBuilder {
    widget: WidgetBuilder,
    props: Properties,
    on_change: Vec<Box<dyn FnMut(&Arc<SpinButton>, i32) + Send + 'static>>,
}

/// An error than can occur from [`SpinButtonBuilder::build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpinButtonError {
    /// Value provided by [`SpinButtonBuilder::max_value`] is greater than the value provided by
    /// [`SpinButtonBuilder::min_value`].
    MaxLessThanMin,
    /// Value provided by [`SpinButtonBuilder::set_value`] is not in range specified by
    /// [`SpinButtonBuilder::min_value`] and [`SpinButtonBuilder::max_value`].
    SetValNotInRange,
}

struct Properties {
    min: i32,
    max: i32,
    val: i32,
    small_step: i32,
    medium_step: i32,
    large_step: i32,
    width: Option<f32>,
    height: Option<f32>,
    text_height: Option<f32>,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            min: 0,
            max: 0,
            val: 0,
            small_step: 1,
            medium_step: 1,
            large_step: 1,
            width: None,
            height: None,
            text_height: None,
        }
    }
}

impl SpinButtonBuilder {
    pub(crate) fn with_builder(builder: WidgetBuilder) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
            on_change: Vec::new(),
        }
    }

    /// Specify the minimum value.
    ///
    /// **Note**: When this isn't used the minimum value will be `0`.
    pub fn min_value(mut self, min: i32) -> Self {
        self.props.min = min;
        self
    }

    /// Specify the maximum value.
    ///
    /// **Note**: When this isn't used the maxium value will be `0`.
    pub fn max_value(mut self, max: i32) -> Self {
        self.props.max = max;
        self
    }

    /// Set the initial value.
    ///
    /// **Note**: When this isn't used the initial value will be `0`.
    pub fn set_value(mut self, val: i32) -> Self {
        self.props.val = val;
        self
    }

    /// Set the value of a small step.
    ///
    /// **Notes**:
    /// - This is when no modifier keys are used.
    /// - When this isn't used the small step will be `1`.
    pub fn small_step(mut self, step: i32) -> Self {
        self.props.small_step = step;
        self
    }

    /// Set the value of a medium step.
    ///
    /// **Notes**:
    /// - This when either [`Qwerty::LCtrl`](basalt::input::Qwerty::LCtrl) or
    /// [`Qwerty::RCtrl`](basalt::input::Qwerty::RCtrl) is used.
    /// - Dragging the knob with the mouse will not be effected by this value.
    /// - When this isn't used the medium step will be `1`.
    pub fn medium_step(mut self, step: i32) -> Self {
        self.props.medium_step = step;
        self
    }

    /// Set the value of a large step.
    ///
    /// **Notes**:
    /// - This when either [`Qwerty::LShift`](basalt::input::Qwerty::LShift) or
    /// [`Qwerty::RShift`](basalt::input::Qwerty::RShift) is used.
    /// - Dragging the knob with the mouse will not be effected by this value.
    /// - When this isn't used the large step will be `1`.
    pub fn large_step(mut self, step: i32) -> Self {
        self.props.large_step = step;
        self
    }

    /// **Temporary**
    pub fn width(mut self, width: f32) -> Self {
        self.props.width = Some(width);
        self
    }

    /// **Temporary**
    pub fn height(mut self, height: f32) -> Self {
        self.props.height = Some(height);
        self
    }

    /// **Temporary**
    pub fn text_height(mut self, text_height: f32) -> Self {
        self.props.text_height = Some(text_height);
        self
    }

    /// Add a callback to be called when the [`SpinButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<SpinButton>, i32) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`SpinButton`].
    pub fn build(self) -> Result<Arc<SpinButton>, SpinButtonError> {
        if self.props.max < self.props.min {
            return Err(SpinButtonError::MaxLessThanMin);
        }

        if self.props.val < self.props.min || self.props.val > self.props.max {
            return Err(SpinButtonError::SetValNotInRange);
        }

        let window = self.widget.parent.window();
        let mut bins = window.new_bins(4).into_iter();
        let container = bins.next().unwrap();
        let entry = bins.next().unwrap();
        let sub_button = bins.next().unwrap();
        let add_button = bins.next().unwrap();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => unimplemented!(),
        }

        container.add_child(entry.clone());
        container.add_child(sub_button.clone());
        container.add_child(add_button.clone());
        let initial_val = self.props.val;

        let spin_button = Arc::new(SpinButton {
            theme: self.widget.theme,
            props: self.props,
            container,
            entry,
            sub_button,
            add_button,
            state: ReentrantMutex::new(State {
                val: RefCell::new(initial_val),
                on_change: RefCell::new(self.on_change),
            }),
        });

        let cb_spin_button = spin_button.clone();

        button_hooks(
            &spin_button.sub_button,
            [
                spin_button.theme.colors.back3,
                spin_button.theme.colors.border2,
            ],
            [
                spin_button.theme.colors.accent1,
                spin_button.theme.colors.back2,
            ],
            [
                spin_button.theme.colors.accent2,
                spin_button.theme.colors.back2,
            ],
            move |w_state| {
                let step = cb_spin_button.step_size(w_state);
                cb_spin_button.decrement(step);
            },
        );

        let cb_spin_button = spin_button.clone();

        button_hooks(
            &spin_button.add_button,
            [
                spin_button.theme.colors.back3,
                spin_button.theme.colors.border2,
            ],
            [
                spin_button.theme.colors.accent1,
                spin_button.theme.colors.back2,
            ],
            [
                spin_button.theme.colors.accent2,
                spin_button.theme.colors.back2,
            ],
            move |w_state| {
                let step = cb_spin_button.step_size(w_state);
                cb_spin_button.increment(step);
            },
        );

        let cb_spin_button = spin_button.clone();

        spin_button.entry.on_character(move |_, _, c| {
            if c.is_new_line() {
                let val: i32 = cb_spin_button
                    .entry
                    .style_inspect(|style| style.text.parse::<i32>())
                    .unwrap_or(cb_spin_button.props.val);

                cb_spin_button
                    .container
                    .basalt_ref()
                    .input_ref()
                    .clear_bin_focus(cb_spin_button.container.window().unwrap().id());

                cb_spin_button.set(val);
            } else if c.is_backspace() {
                let mut entry_style = cb_spin_button.entry.style_copy();
                entry_style.text.pop();

                cb_spin_button
                    .entry
                    .style_update(entry_style)
                    .expect_valid();
            } else if c.0.is_numeric() {
                let mut entry_style = cb_spin_button.entry.style_copy();
                entry_style.text.push(c.0);

                cb_spin_button
                    .entry
                    .style_update(entry_style)
                    .expect_valid();
            }

            Default::default()
        });

        let cb_spin_button = spin_button.clone();

        spin_button.entry.on_focus(move |_, _| {
            let border_size = cb_spin_button.theme.border.unwrap_or(1.0);

            cb_spin_button
                .entry
                .style_update(BinStyle {
                    border_size_t: Some(border_size),
                    border_size_b: Some(border_size),
                    border_size_l: Some(border_size),
                    border_size_r: Some(border_size),
                    ..cb_spin_button.entry.style_copy()
                })
                .expect_valid();

            Default::default()
        });

        let cb_spin_button = spin_button.clone();

        spin_button.entry.on_focus_lost(move |_, _| {
            cb_spin_button
                .entry
                .style_update(BinStyle {
                    border_size_t: None,
                    border_size_b: None,
                    border_size_l: None,
                    border_size_r: None,
                    ..cb_spin_button.entry.style_copy()
                })
                .expect_valid();

            Default::default()
        });

        spin_button.style_update();
        Ok(spin_button)
    }
}

/// Spin button widget
pub struct SpinButton {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    entry: Arc<Bin>,
    sub_button: Arc<Bin>,
    add_button: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    val: RefCell<i32>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<SpinButton>, i32) + Send + 'static>>>,
}

impl SpinButton {
    fn step_size(&self, w_state: &WindowState) -> i32 {
        if w_state.is_key_pressed(Qwerty::LCtrl) || w_state.is_key_pressed(Qwerty::RCtrl) {
            self.props.medium_step
        } else if w_state.is_key_pressed(Qwerty::LShift) || w_state.is_key_pressed(Qwerty::RShift) {
            self.props.large_step
        } else {
            self.props.small_step
        }
    }

    /// Set the value to the provided valued.
    ///
    /// **Note**: This value will be clamped to values provided by [`SpinButtonBuilder::min_value`]
    /// and [`SpinButtonBuilder::max_value`].
    pub fn set(self: &Arc<Self>, val: i32) {
        let state = self.state.lock();
        let val = val.clamp(self.props.min, self.props.max);
        *state.val.borrow_mut() = val;

        self.entry
            .style_update(BinStyle {
                text: format!("{}", val),
                ..self.entry.style_copy()
            })
            .expect_valid();

        if let Ok(mut on_change_cbs) = state.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(self, val);
            }
        }
    }

    /// Get the current value.
    pub fn val(&self) -> i32 {
        *self.state.lock().val.borrow()
    }

    /// Increment the value by the provided amount.
    ///
    /// **Note**: The resulting value will be clamped to values provided by [`SpinButtonBuilder::min_value`]
    /// and [`SpinButtonBuilder::max_value`].
    pub fn increment(self: &Arc<Self>, amt: i32) {
        let state = self.state.lock();

        let val = state
            .val
            .borrow()
            .checked_add(amt)
            .unwrap_or(self.props.max);

        self.set(val);
    }

    /// Decrement the value by the provided amount.
    ///
    /// **Note**: The resulting value will be clamped to values provided by [`SpinButtonBuilder::min_value`]
    /// and [`SpinButtonBuilder::max_value`].
    pub fn decrement(self: &Arc<Self>, amt: i32) {
        let state = self.state.lock();

        let val = state
            .val
            .borrow()
            .checked_sub(amt)
            .unwrap_or(self.props.min);

        self.set(val);
    }

    /// Add a callback to be called when the [`SpinButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<SpinButton>, i32) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    fn style_update(self: &Arc<Self>) {
        let text_height = self.props.text_height.unwrap_or(self.theme.text_height);
        let border_size = self.theme.border.unwrap_or(0.0);

        let widget_height = match self.props.height {
            Some(height) => height,
            None => (self.theme.spacing * 2.0) + self.theme.spacing,
        };

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            height: Some(widget_height),
            ..Default::default()
        };

        let mut entry_style = BinStyle {
            position: Some(BinPosition::Parent),
            add_z_index: Some(1),
            pos_from_t: Some(0.0),
            pos_from_l: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_r: Some((widget_height * 2.0) + (border_size * 2.0)),
            back_color: Some(self.theme.colors.back2),
            border_color_t: Some(self.theme.colors.accent1),
            border_color_b: Some(self.theme.colors.accent1),
            border_color_l: Some(self.theme.colors.accent1),
            border_color_r: Some(self.theme.colors.accent1),
            text_height: Some(text_height),
            text_color: Some(self.theme.colors.text1a),
            text_hori_align: Some(TextHoriAlign::Left),
            text_vert_align: Some(TextVertAlign::Center),
            text_wrap: Some(TextWrap::None),
            font_family: Some(self.theme.font_family.clone()),
            font_weight: Some(self.theme.font_weight),
            pad_l: Some(self.theme.spacing),
            ..Default::default()
        };

        let mut sub_button_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_r: Some(widget_height + border_size),
            pos_from_b: Some(0.0),
            width: Some(widget_height),
            back_color: Some(self.theme.colors.back3),
            custom_verts: sub_symbol_verts(
                text_height,
                self.theme.spacing,
                self.theme.colors.border2,
            ),
            ..Default::default()
        };

        let mut add_button_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_r: Some(0.0),
            pos_from_b: Some(0.0),
            width: Some(widget_height),
            back_color: Some(self.theme.colors.back3),
            custom_verts: add_symbol_verts(
                text_height,
                self.theme.spacing,
                self.theme.colors.border2,
            ),
            ..Default::default()
        };

        if let Some(border_size) = self.theme.border {
            container_style.border_size_t = Some(border_size);
            container_style.border_size_b = Some(border_size);
            container_style.border_size_l = Some(border_size);
            container_style.border_size_r = Some(border_size);
            container_style.border_color_t = Some(self.theme.colors.border1);
            container_style.border_color_b = Some(self.theme.colors.border1);
            container_style.border_color_l = Some(self.theme.colors.border1);
            container_style.border_color_r = Some(self.theme.colors.border1);

            sub_button_style.border_size_l = Some(border_size);
            sub_button_style.border_color_l = Some(self.theme.colors.border2);

            add_button_style.border_size_l = Some(border_size);
            add_button_style.border_color_l = Some(self.theme.colors.border2);
        }

        if let Some(border_radius) = self.theme.roundness {
            container_style.border_radius_tl = Some(border_radius);
            container_style.border_radius_tr = Some(border_radius);
            container_style.border_radius_bl = Some(border_radius);
            container_style.border_radius_br = Some(border_radius);

            entry_style.border_radius_tl = Some(border_radius);
            entry_style.border_radius_bl = Some(border_radius);

            add_button_style.border_radius_tr = Some(border_radius);
            add_button_style.border_radius_br = Some(border_radius);
        }

        match self.props.width {
            Some(width) => {
                let min_widget_width = (widget_height * 3.0) + (border_size * 2.0);
                container_style.width = Some(min_widget_width.max(width));
                entry_style.text = format!("{}", self.props.val);
            },
            None => {
                let min_val_places = self.props.min.abs().checked_ilog10().unwrap_or(0) + 1;

                let max_val_places = self.props.max.abs().checked_ilog10().unwrap_or(0) + 1;
                let mut places = min_val_places.max(max_val_places);

                if self.props.min.is_negative() {
                    places += 1;
                }

                let base_widget_width =
                    (widget_height * 2.0) + (border_size * 2.0) + self.theme.spacing;

                entry_style.text = (0..places).map(|_| '9').collect();
                container_style.width = Some(base_widget_width);
                container_style.hidden = Some(true);

                let cb_spin_button = self.clone();

                self.entry.on_update_once(move |_, _| {
                    cb_spin_button
                        .container
                        .style_update(BinStyle {
                            width: Some(
                                base_widget_width
                                    + cb_spin_button.entry.calc_hori_overflow()
                                    + cb_spin_button.theme.spacing,
                            ),
                            hidden: None,
                            ..cb_spin_button.container.style_copy()
                        })
                        .expect_valid();

                    cb_spin_button
                        .entry
                        .style_update(BinStyle {
                            text: format!("{}", cb_spin_button.props.val),
                            ..cb_spin_button.entry.style_copy()
                        })
                        .expect_valid();
                });
            },
        }

        self.container.style_update(container_style).expect_valid();
        self.entry.style_update(entry_style).expect_valid();

        self.sub_button
            .style_update(sub_button_style)
            .expect_valid();

        self.add_button
            .style_update(add_button_style)
            .expect_valid();
    }
}

fn sub_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    let h_bar_l = spacing + 1.0;
    let h_bar_r = spacing + target_size - 1.0;
    let h_bar_t = spacing + ((target_size / 2.0) - 1.0);
    let h_bar_b = h_bar_t + 2.0;
    let mut verts = Vec::with_capacity(6);

    for [x, y] in [
        [h_bar_r, h_bar_t],
        [h_bar_l, h_bar_t],
        [h_bar_l, h_bar_b],
        [h_bar_r, h_bar_t],
        [h_bar_l, h_bar_b],
        [h_bar_r, h_bar_b],
    ] {
        verts.push(BinVert {
            position: (x, y, 0),
            color,
        });
    }

    verts
}

fn add_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    let v_bar_l = spacing + ((target_size / 2.0) - 1.0);
    let v_bar_r = v_bar_l + 2.0;
    let v_bar_t = spacing;
    let v_bar_b = spacing + target_size;
    let h_bar_l = spacing;
    let h_bar_r = spacing + target_size;
    let h_bar_t = spacing + ((target_size / 2.0) - 1.0);
    let h_bar_b = h_bar_t + 2.0;
    let mut verts = Vec::with_capacity(12);

    for [x, y] in [
        [v_bar_r, v_bar_t],
        [v_bar_l, v_bar_t],
        [v_bar_l, v_bar_b],
        [v_bar_r, v_bar_t],
        [v_bar_l, v_bar_b],
        [v_bar_r, v_bar_b],
        [h_bar_r, h_bar_t],
        [h_bar_l, h_bar_t],
        [h_bar_l, h_bar_b],
        [h_bar_r, h_bar_t],
        [h_bar_l, h_bar_b],
        [h_bar_r, h_bar_b],
    ] {
        verts.push(BinVert {
            position: (x, y, 0),
            color,
        });
    }

    verts
}
