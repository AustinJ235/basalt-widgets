use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

use basalt::input::{MouseButton, Qwerty, WindowState};
use basalt::interface::{Bin, BinPosition, BinStyle};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

/// Builder for [`HoriScaler`]
pub struct HoriScalerBuilder {
    widget: WidgetBuilder,
    props: Properties,
    on_change: Vec<Box<dyn FnMut(&Arc<HoriScaler>, f32) + Send + 'static>>,
}

/// An error than can occur from [`HoriScalerBuilder::build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoriScalerError {
    /// Value provided by [`HoriScalerBuilder::max_value`] is greater than the value provided by
    /// [`HoriScalerBuilder::min_value`].
    MaxLessThanMin,
    /// Value provided by [`HoriScalerBuilder::set_value`] is not in range specified by
    /// [`HoriScalerBuilder::min_value`] and [`HoriScalerBuilder::max_value`].
    SetValNotInRange,
}

/// Determines how the value of [`HoriScaler`] is rounded when it is modified.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum ScalerRound {
    /// The value is not rounded and left as is.
    ///
    /// **Note**: This is the default.
    #[default]
    None,
    /// The value is rounded to increments of the small step provided by
    /// [`HoriScalerBuilder::small_step`].
    Step,
    /// The value is rounded to the nearest whole number.
    Int,
}

struct Properties {
    min: f32,
    max: f32,
    val: f32,
    small_step: f32,
    medium_step: f32,
    large_step: f32,
    round: ScalerRound,
    width: Option<f32>,
    height: Option<f32>,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            val: 0.0,
            small_step: 1.0,
            medium_step: 1.0,
            large_step: 1.0,
            round: Default::default(),
            width: None,
            height: None,
        }
    }
}

/// Builder for [`HoriScaler`].
impl HoriScalerBuilder {
    pub(crate) fn with_builder(builder: WidgetBuilder) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
            on_change: Vec::new(),
        }
    }

    /// Specify the minimum value.
    ///
    /// **Note**: When this isn't used the minimum value will be `0.0`.
    pub fn min_value(mut self, min: f32) -> Self {
        self.props.min = min;
        self
    }

    /// Specify the maximum value.
    ///
    /// **Note**: When this isn't used the maxium value will be `0.0`.
    pub fn max_value(mut self, max: f32) -> Self {
        self.props.max = max;
        self
    }

    /// Set the initial value.
    ///
    /// **Note**: When this isn't used the initial value will be `0.0`.
    pub fn set_value(mut self, val: f32) -> Self {
        self.props.val = val;
        self
    }

    /// Set the value of a small step.
    ///
    /// **Notes**:
    /// - This is when no modifier keys are used.
    /// - When this isn't used the small step will be `1.0`.
    pub fn small_step(mut self, step: f32) -> Self {
        self.props.small_step = step;
        self
    }

    /// Set the value of a medium step.
    ///
    /// **Notes**:
    /// - This when either [`Qwerty::LCtrl`](basalt::input::Qwerty::LCtrl) or
    /// [`Qwerty::RCtrl`](basalt::input::Qwerty::RCtrl) is used.
    /// - Dragging the knob with the mouse will not be effected by this value.
    /// - When this isn't used the medium step will be `1.0`.
    pub fn medium_step(mut self, step: f32) -> Self {
        self.props.medium_step = step;
        self
    }

    /// Set the value of a large step.
    ///
    /// **Notes**:
    /// - This when either [`Qwerty::LShift`](basalt::input::Qwerty::LShift) or
    /// [`Qwerty::RShift`](basalt::input::Qwerty::RShift) is used.
    /// - Dragging the knob with the mouse will not be effected by this value.
    /// - When this isn't used the large step will be `1.0`.
    pub fn large_step(mut self, step: f32) -> Self {
        self.props.large_step = step;
        self
    }

    /// Set how the value is rounded after being modified.
    ///
    /// See documentation of [`ScalerRound`] for more information.
    pub fn round(mut self, round: ScalerRound) -> Self {
        self.props.round = round;
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

    /// Add a callback to be called when the [`HoriScaler`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<HoriScaler>, f32) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`HoriScaler`].
    pub fn build(self) -> Result<Arc<HoriScaler>, HoriScalerError> {
        if self.props.max < self.props.min {
            return Err(HoriScalerError::MaxLessThanMin);
        }

        if self.props.val < self.props.min || self.props.val > self.props.max {
            return Err(HoriScalerError::SetValNotInRange);
        }

        let window = self.widget.parent.window();
        let mut bins = window.new_bins(4).into_iter();
        let container = bins.next().unwrap();
        let track = bins.next().unwrap();
        let confine = bins.next().unwrap();
        let knob = bins.next().unwrap();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => unimplemented!(),
        }

        container.add_child(track.clone());
        container.add_child(confine.clone());
        confine.add_child(knob.clone());

        let initial_val = self.props.val;

        let hori_scaler = Arc::new(HoriScaler {
            theme: self.widget.theme,
            props: self.props,
            container,
            track,
            confine,
            knob,
            state: ReentrantMutex::new(State {
                val: RefCell::new(initial_val),
                on_change: RefCell::new(self.on_change),
            }),
        });

        let cb_hori_scaler = hori_scaler.clone();

        hori_scaler.container.on_scroll(move |_, w_state, amt, _| {
            let step = cb_hori_scaler.step_size(w_state) * -amt;
            cb_hori_scaler.increment(step);
            Default::default()
        });

        let knob_held = Arc::new(AtomicBool::new(false));
        let mut window_hook_ids = Vec::new();

        let cb_knob_held = knob_held.clone();

        hori_scaler
            .knob
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_knob_held.store(true, atomic::Ordering::SeqCst);
                Default::default()
            });

        let cb_knob_held = knob_held.clone();

        hori_scaler
            .knob
            .on_release(MouseButton::Left, move |_, _, _| {
                cb_knob_held.store(false, atomic::Ordering::SeqCst);
                Default::default()
            });

        let cb_hori_scaler = hori_scaler.clone();
        let cb_knob_held = knob_held.clone();

        window_hook_ids.push(window.on_cursor(move |_, w_state, _| {
            if cb_knob_held.load(atomic::Ordering::SeqCst) {
                let [cursor_x, _] = w_state.cursor_pos();
                let track_bpu = cb_hori_scaler.track.post_update();
                let knob_bpu = cb_hori_scaler.knob.post_update();
                let knob_width_1_2 = (knob_bpu.tri[0] - knob_bpu.tli[0]) / 2.0;
                let cursor_x_min = track_bpu.tli[0] + knob_width_1_2;
                let cursor_x_max = track_bpu.tri[0] - knob_width_1_2;
                let pct = ((cursor_x - cursor_x_min) / (cursor_x_max - cursor_x_min)) * 100.0;
                cb_hori_scaler.set_pct(pct.clamp(0.0, 100.0));
            }

            Default::default()
        }));

        let focused = Arc::new(AtomicBool::new(false));

        let widget_bin_ids = [
            hori_scaler.container.id(),
            hori_scaler.track.id(),
            hori_scaler.confine.id(),
            hori_scaler.knob.id(),
        ];

        for bin in [
            &hori_scaler.container,
            &hori_scaler.track,
            &hori_scaler.confine,
            &hori_scaler.knob,
        ] {
            let cb_focused = focused.clone();

            bin.on_focus(move |_, _| {
                cb_focused.store(true, atomic::Ordering::SeqCst);
                Default::default()
            });

            let cb_focused = focused.clone();

            bin.on_focus_lost(move |_, w_state| {
                if let Some(focused_bin_id) = w_state.focused_bin_id() {
                    if !widget_bin_ids.contains(&focused_bin_id) {
                        cb_focused.store(false, atomic::Ordering::SeqCst);
                    }
                }

                Default::default()
            });
        }

        let cb_hori_scaler = hori_scaler.clone();
        let cb_focused = focused.clone();

        window_hook_ids.push(window.on_press(Qwerty::ArrowUp, move |_, w_state, _| {
            if cb_focused.load(atomic::Ordering::SeqCst) {
                let step = cb_hori_scaler.step_size(w_state);
                cb_hori_scaler.increment(step);
            }

            Default::default()
        }));

        let cb_hori_scaler = hori_scaler.clone();
        let cb_focused = focused.clone();

        window_hook_ids.push(window.on_press(Qwerty::ArrowRight, move |_, w_state, _| {
            if cb_focused.load(atomic::Ordering::SeqCst) {
                let step = cb_hori_scaler.step_size(w_state);
                cb_hori_scaler.increment(step);
            }

            Default::default()
        }));

        let cb_hori_scaler = hori_scaler.clone();
        let cb_focused = focused.clone();

        window_hook_ids.push(window.on_press(Qwerty::ArrowDown, move |_, w_state, _| {
            if cb_focused.load(atomic::Ordering::SeqCst) {
                let step = cb_hori_scaler.step_size(w_state);
                cb_hori_scaler.decrement(step);
            }

            Default::default()
        }));

        let cb_hori_scaler = hori_scaler.clone();
        let cb_focused = focused.clone();

        window_hook_ids.push(window.on_press(Qwerty::ArrowLeft, move |_, w_state, _| {
            if cb_focused.load(atomic::Ordering::SeqCst) {
                let step = cb_hori_scaler.step_size(w_state);
                cb_hori_scaler.decrement(step);
            }

            Default::default()
        }));

        for window_hook_id in window_hook_ids {
            hori_scaler.container.attach_input_hook(window_hook_id);
        }

        hori_scaler.style_update();
        Ok(hori_scaler)
    }
}

/// Horizonal scaler widget
pub struct HoriScaler {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    track: Arc<Bin>,
    confine: Arc<Bin>,
    knob: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    val: RefCell<f32>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<HoriScaler>, f32) + Send + 'static>>>,
}

impl HoriScaler {
    fn step_size(&self, w_state: &WindowState) -> f32 {
        if w_state.is_key_pressed(Qwerty::LCtrl) || w_state.is_key_pressed(Qwerty::RCtrl) {
            self.props.medium_step
        } else if w_state.is_key_pressed(Qwerty::LShift) || w_state.is_key_pressed(Qwerty::RShift) {
            self.props.large_step
        } else {
            self.props.small_step
        }
    }

    fn set_pct(self: &Arc<Self>, pct: f32) {
        self.set(((self.props.max - self.props.min) * (pct / 100.0)) + self.props.min);
    }

    /// Set the value to the provided valued.
    ///
    /// **Notes**:
    /// - This will be effected by rounding provided by [`HoriScalerBuilder::round`].
    /// - This value will be clamped to values provided by [`HoriScalerBuilder::min_value`]
    /// and [`HoriScalerBuilder::max_value`].
    pub fn set(self: &Arc<Self>, mut val: f32) {
        val = match self.props.round {
            ScalerRound::None => val,
            ScalerRound::Int => val.round(),
            ScalerRound::Step => (val / self.props.small_step).round() * self.props.small_step,
        }
        .clamp(self.props.min, self.props.max);

        let pct = ((val - self.props.min) / (self.props.max - self.props.min)) * 100.0;

        self.knob
            .style_update(BinStyle {
                pos_from_l_pct: Some(pct),
                ..self.knob.style_copy()
            })
            .expect_valid();

        let state = self.state.lock();
        *state.val.borrow_mut() = val;

        if let Ok(mut on_change_cbs) = state.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(self, val);
            }
        }
    }

    /// Get the current value.
    pub fn val(&self) -> f32 {
        *self.state.lock().val.borrow()
    }

    /// Increment the value by the provided amount.
    ///
    /// **Notes**:
    /// - The resulting value will be effected by rounding provided by [`HoriScalerBuilder::round`].
    /// - The resulting value will be clamped to values provided by [`HoriScalerBuilder::min_value`]
    /// and [`HoriScalerBuilder::max_value`].
    pub fn increment(self: &Arc<Self>, amt: f32) {
        let state = self.state.lock();
        let val = *state.val.borrow() + amt;
        self.set(val);
    }

    /// Decrement the value by the provided amount.
    ///
    /// **Notes**:
    /// - The resulting value will be effected by rounding provided by [`HoriScalerBuilder::round`].
    /// - The resulting value will be clamped to values provided by [`HoriScalerBuilder::min_value`]
    /// and [`HoriScalerBuilder::max_value`].
    pub fn decrement(self: &Arc<Self>, amt: f32) {
        let state = self.state.lock();
        let val = *state.val.borrow() - amt;
        self.set(val);
    }

    /// Add a callback to be called when the [`HoriScaler`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<HoriScaler>, f32) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    fn style_update(self: &Arc<Self>) {
        let widget_height = match self.props.height {
            Some(height) => height,
            None => self.theme.spacing * 2.0,
        };

        let widget_width = match self.props.width {
            Some(width) => width.max(widget_height),
            None => widget_height * 8.0,
        };

        let widget_height_1_2 = widget_height / 2.0;
        let widget_height_1_4 = widget_height / 4.0;

        let pct = ((*self.state.lock().val.borrow() - self.props.min)
            / (self.props.max - self.props.min))
            * 100.0;

        let container_style = BinStyle {
            position: Some(BinPosition::Floating),
            height: Some(widget_height),
            width: Some(widget_width),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            ..Default::default()
        };

        let mut track_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(widget_height_1_4),
            pos_from_b: Some(widget_height_1_4),
            pos_from_l: Some(0.0),
            pos_from_r: Some(0.0),
            back_color: Some(self.theme.colors.back4),
            border_radius_tl: Some(widget_height_1_4),
            border_radius_tr: Some(widget_height_1_4),
            border_radius_bl: Some(widget_height_1_4),
            border_radius_br: Some(widget_height_1_4),
            ..Default::default()
        };

        let confine_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_l: Some(0.0),
            pos_from_r: Some(widget_height),
            overflow_x: Some(true),
            ..Default::default()
        };

        let mut knob_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_l_pct: Some(pct),
            width: Some(widget_height),
            back_color: Some(self.theme.colors.accent1),
            border_radius_tl: Some(widget_height_1_2),
            border_radius_tr: Some(widget_height_1_2),
            border_radius_bl: Some(widget_height_1_2),
            border_radius_br: Some(widget_height_1_2),
            ..Default::default()
        };

        if let Some(border_size) = self.theme.border {
            track_style.border_size_t = Some(border_size);
            track_style.border_size_b = Some(border_size);
            track_style.border_size_l = Some(border_size);
            track_style.border_size_r = Some(border_size);
            track_style.border_color_t = Some(self.theme.colors.border1);
            track_style.border_color_b = Some(self.theme.colors.border1);
            track_style.border_color_l = Some(self.theme.colors.border1);
            track_style.border_color_r = Some(self.theme.colors.border1);
            knob_style.border_size_t = Some(border_size);
            knob_style.border_size_b = Some(border_size);
            knob_style.border_size_l = Some(border_size);
            knob_style.border_size_r = Some(border_size);
            knob_style.border_color_t = Some(self.theme.colors.border3);
            knob_style.border_color_b = Some(self.theme.colors.border3);
            knob_style.border_color_l = Some(self.theme.colors.border3);
            knob_style.border_color_r = Some(self.theme.colors.border3);
        }

        self.container.style_update(container_style).expect_valid();
        self.track.style_update(track_style).expect_valid();
        self.confine.style_update(confine_style).expect_valid();
        self.knob.style_update(knob_style).expect_valid();
    }
}
