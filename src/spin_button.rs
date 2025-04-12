use std::cell::RefCell;
use std::sync::Arc;

use basalt::input::{Qwerty, WindowState};
use basalt::interface::UnitValue::{Pixels, Undefined};
use basalt::interface::{
    Bin, BinStyle, BinVert, Color, Position, TextAttrs, TextBody, TextHoriAlign, TextVertAlign,
    TextWrap, Visibility, ZIndex,
};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::button::{BtnHookColors, button_hooks};
use crate::{Theme, WidgetContainer};

/// Builder for [`SpinButton`]
pub struct SpinButtonBuilder<'a, C> {
    widget: WidgetBuilder<'a, C>,
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

impl<'a, C> SpinButtonBuilder<'a, C>
where
    C: WidgetContainer,
{
    pub(crate) fn with_builder(builder: WidgetBuilder<'a, C>) -> Self {
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

        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut new_bins = window.new_bins(4).into_iter();
        let container = new_bins.next().unwrap();
        let entry = new_bins.next().unwrap();
        let sub_button = new_bins.next().unwrap();
        let add_button = new_bins.next().unwrap();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

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
            BtnHookColors {
                back_clr: Some(spin_button.theme.colors.back3),
                vert_clr: Some(spin_button.theme.colors.border2),
                h_back_clr: Some(spin_button.theme.colors.accent1),
                h_vert_clr: Some(spin_button.theme.colors.back2),
                p_back_clr: Some(spin_button.theme.colors.accent2),
                p_vert_clr: Some(spin_button.theme.colors.back2),
                ..Default::default()
            },
            move |w_state| {
                let step = cb_spin_button.step_size(w_state);
                cb_spin_button.decrement(step);
            },
        );

        let cb_spin_button = spin_button.clone();

        button_hooks(
            &spin_button.add_button,
            BtnHookColors {
                back_clr: Some(spin_button.theme.colors.back3),
                vert_clr: Some(spin_button.theme.colors.border2),
                h_back_clr: Some(spin_button.theme.colors.accent1),
                h_vert_clr: Some(spin_button.theme.colors.back2),
                p_back_clr: Some(spin_button.theme.colors.accent2),
                p_vert_clr: Some(spin_button.theme.colors.back2),
                ..Default::default()
            },
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
                    .style_inspect(|style| style.text.spans[0].text.parse::<i32>())
                    .unwrap_or(cb_spin_button.props.val);

                cb_spin_button
                    .container
                    .basalt_ref()
                    .input_ref()
                    .clear_bin_focus(cb_spin_button.container.window().unwrap().id());

                cb_spin_button.set(val);
            } else if c.is_backspace() {
                let mut entry_style = cb_spin_button.entry.style_copy();
                entry_style.text.spans[0].text.pop();

                cb_spin_button
                    .entry
                    .style_update(entry_style)
                    .expect_valid();
            } else if c.0.is_numeric() {
                let mut entry_style = cb_spin_button.entry.style_copy();
                entry_style.text.spans[0].text.push(c.0);

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
                    border_size_t: Pixels(border_size),
                    border_size_b: Pixels(border_size),
                    border_size_l: Pixels(border_size),
                    border_size_r: Pixels(border_size),
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
                    border_size_t: Undefined,
                    border_size_b: Undefined,
                    border_size_l: Undefined,
                    border_size_r: Undefined,
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

        self.entry.style_modify(|style| {
            style.text.spans[0].text = format!("{}", val);
        });

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
            position: Position::Floating,
            margin_t: Pixels(self.theme.spacing),
            margin_b: Pixels(self.theme.spacing),
            margin_l: Pixels(self.theme.spacing),
            margin_r: Pixels(self.theme.spacing),
            height: Pixels(widget_height),
            ..Default::default()
        };

        let mut entry_style = BinStyle {
            z_index: ZIndex::Offset(1),
            pos_from_t: Pixels(0.0),
            pos_from_l: Pixels(0.0),
            pos_from_b: Pixels(0.0),
            pos_from_r: Pixels((widget_height * 2.0) + (border_size * 2.0)),
            back_color: self.theme.colors.back2,
            border_color_t: self.theme.colors.accent1,
            border_color_b: self.theme.colors.accent1,
            border_color_l: self.theme.colors.accent1,
            border_color_r: self.theme.colors.accent1,
            padding_l: Pixels(self.theme.spacing),
            text: TextBody {
                spans: vec![Default::default()],
                hori_align: TextHoriAlign::Left,
                vert_align: TextVertAlign::Center,
                text_wrap: TextWrap::None,
                base_attrs: TextAttrs {
                    height: Pixels(text_height),
                    color: self.theme.colors.text1a,
                    font_family: self.theme.font_family.clone(),
                    font_weight: self.theme.font_weight,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let mut sub_button_style = BinStyle {
            pos_from_t: Pixels(0.0),
            pos_from_r: Pixels(widget_height + border_size),
            pos_from_b: Pixels(0.0),
            width: Pixels(widget_height),
            back_color: self.theme.colors.back3,
            custom_verts: sub_symbol_verts(
                text_height,
                self.theme.spacing,
                self.theme.colors.border2,
            ),
            ..Default::default()
        };

        let mut add_button_style = BinStyle {
            pos_from_t: Pixels(0.0),
            pos_from_r: Pixels(0.0),
            pos_from_b: Pixels(0.0),
            width: Pixels(widget_height),
            back_color: self.theme.colors.back3,
            custom_verts: add_symbol_verts(
                text_height,
                self.theme.spacing,
                self.theme.colors.border2,
            ),
            ..Default::default()
        };

        if let Some(border_size) = self.theme.border {
            container_style.border_size_t = Pixels(border_size);
            container_style.border_size_b = Pixels(border_size);
            container_style.border_size_l = Pixels(border_size);
            container_style.border_size_r = Pixels(border_size);
            container_style.border_color_t = self.theme.colors.border1;
            container_style.border_color_b = self.theme.colors.border1;
            container_style.border_color_l = self.theme.colors.border1;
            container_style.border_color_r = self.theme.colors.border1;

            sub_button_style.border_size_l = Pixels(border_size);
            sub_button_style.border_color_l = self.theme.colors.border2;

            add_button_style.border_size_l = Pixels(border_size);
            add_button_style.border_color_l = self.theme.colors.border2;
        }

        if let Some(border_radius) = self.theme.roundness {
            container_style.border_radius_tl = border_radius;
            container_style.border_radius_tr = border_radius;
            container_style.border_radius_bl = border_radius;
            container_style.border_radius_br = border_radius;

            entry_style.border_radius_tl = border_radius;
            entry_style.border_radius_bl = border_radius;

            add_button_style.border_radius_tr = border_radius;
            add_button_style.border_radius_br = border_radius;
        }

        match self.props.width {
            Some(width) => {
                let min_widget_width = (widget_height * 3.0) + (border_size * 2.0);
                container_style.width = Pixels(min_widget_width.max(width));
                entry_style.text.spans[0].text = format!("{}", self.props.val);
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

                entry_style.text.spans[0].text = (0..places).map(|_| '9').collect();
                container_style.width = Pixels(base_widget_width);
                container_style.visibility = Visibility::Hide;

                let cb_spin_button = self.clone();

                self.entry.on_update_once(move |_, _| {
                    cb_spin_button
                        .container
                        .style_update(BinStyle {
                            width: Pixels(
                                base_widget_width
                                    + cb_spin_button.entry.calc_hori_overflow()
                                    + cb_spin_button.theme.spacing,
                            ),
                            visibility: Visibility::Inheirt,
                            ..cb_spin_button.container.style_copy()
                        })
                        .expect_valid();

                    cb_spin_button.entry.style_modify(|style| {
                        style.text.spans[0].text = format!("{}", cb_spin_button.props.val);
                    });
                });
            },
        }

        Bin::style_update_batch([
            (&self.container, container_style),
            (&self.entry, entry_style),
            (&self.sub_button, sub_button_style),
            (&self.add_button, add_button_style),
        ]);
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
