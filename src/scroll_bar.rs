use std::cell::RefCell;
use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::time::Duration;

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinID, BinPosition, BinStyle, BinVert, Color};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::button::{BtnHookColors, button_hooks};
use crate::{Theme, WidgetContainer};

/// Determintes the orientation and axis of the [`ScrollBar`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// The [`ScrollBar`] will control the x-axis and be oriented horizontally.
    X,
    /// The [`ScrollBar`] will control the y-axis and be oriented vertically.
    ///
    /// **Note**: This is the default.
    #[default]
    Y,
}

struct Properties {
    target: Arc<Bin>,
    axis: ScrollAxis,
    smooth: bool,
    step: f32,
    accel: bool,
    accel_pow: f32,
    max_accel_mult: f32,
    animation_duration: Duration,
}

#[derive(Default)]
struct InitialState {
    scroll: Option<f32>,
}

impl Properties {
    fn default_target(target: Arc<Bin>) -> Self {
        Self {
            target,
            axis: ScrollAxis::Y,
            smooth: true,
            step: 50.0,
            accel: true,
            accel_pow: 1.2,
            max_accel_mult: 4.0,
            animation_duration: Duration::from_millis(100),
        }
    }
}

/// Builder for [`ScrollBar`]
pub struct ScrollBarBuilder<'a, C> {
    widget: WidgetBuilder<'a, C>,
    props: Properties,
    initial_state: InitialState,
}

impl<'a, C> ScrollBarBuilder<'a, C>
where
    C: WidgetContainer,
{
    pub(crate) fn with_builder<T>(builder: WidgetBuilder<'a, C>, target: T) -> Self
    where
        T: WidgetContainer,
    {
        Self {
            widget: builder,
            props: Properties::default_target(target.container_bin().clone()),
            initial_state: Default::default(),
        }
    }

    /// Set the amount the target container should be scrolled initially.
    ///
    /// **Note**: If not set this defaults the current scroll amount defined by the target container.
    pub fn scroll(mut self, scroll: f32) -> Self {
        self.initial_state.scroll = Some(scroll);
        self
    }

    /// Set the axis.
    ///
    /// See [`ScrollAxis`] docs for more information.
    ///
    /// **Note**: If not set this defaults to [`ScrollAxis::Y`].
    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.props.axis = axis;
        self
    }

    /// Set if smooth scroll is enabled.
    ///
    /// **Note**: If not set this defaults to `true`.
    pub fn smooth(mut self, smooth: bool) -> Self {
        self.props.smooth = smooth;
        self
    }

    /// Set the step size per input event.
    ///
    /// **Note**: If not set this defaults to `50.0`.
    pub fn step(mut self, step: f32) -> Self {
        self.props.step = step;
        self
    }

    /// Set if scroll acceleration is enabled.
    ///
    /// Acceleration behavior is defined by: step size, acceleration power, max acceleration
    /// multiplier and animation duration.
    ///
    /// Acceleration is applied when there is pending scroll events in the animation queue. The
    /// pending scroll amount is divided by step size and raised to the power of acceleration power.
    /// This value is then used as a multiplier on the new step size. The multiplier is capped by the
    /// max acceleration multiplier.
    ///
    /// **Notes**:
    /// - If not set this defaults to `true`.
    /// - Smooth scroll will be enabled if acceleration is enabled.
    pub fn accel(mut self, accel: bool) -> Self {
        self.props.accel = accel;
        self
    }

    /// Set the acceleration power.
    ///
    /// **Notes**:
    /// - If not set this defaults to `1.2`.
    /// - Has no effect if acceleration is not enabled.
    pub fn accel_pow(mut self, accel_pow: f32) -> Self {
        self.props.accel_pow = accel_pow;
        self
    }

    /// Set the max acceleration multiplier.
    ///
    /// **Notes**:
    /// - If not set this defaults to `4.0`.
    /// - Has no effect if acceleration is not enabled.
    pub fn max_accel_mult(mut self, max_accel_mult: f32) -> Self {
        self.props.max_accel_mult = max_accel_mult;
        self
    }

    /// Set the duration of animations.
    ///
    /// **Notes**:
    /// - If not set this defaults to 100 ms.
    /// - Has no effect if smooth scroll or acceleration is not enabled.
    pub fn animation_duration(mut self, animation_duration: Duration) -> Self {
        self.props.animation_duration = animation_duration;
        self
    }

    /// Finish building the [`ScrollBar`].
    pub fn build(self) -> Arc<ScrollBar> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut new_bins = window.new_bins(5).into_iter();
        let container = new_bins.next().unwrap();
        let upright = new_bins.next().unwrap();
        let downleft = new_bins.next().unwrap();
        let confine = new_bins.next().unwrap();
        let bar = new_bins.next().unwrap();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        container.add_child(upright.clone());
        container.add_child(downleft.clone());
        container.add_child(confine.clone());
        confine.add_child(bar.clone());

        let scroll = self.initial_state.scroll.unwrap_or_else(|| {
            self.widget
                .container
                .container_bin()
                .style_inspect(|style| {
                    match self.props.axis {
                        ScrollAxis::X => style.scroll_x.unwrap_or(0.0),
                        ScrollAxis::Y => style.scroll_y.unwrap_or(0.0),
                    }
                })
        });

        let scroll_bar = Arc::new(ScrollBar {
            theme: self.widget.theme,
            props: self.props,
            container,
            upright,
            downleft,
            confine,
            bar,
            state: ReentrantMutex::new(State {
                target: RefCell::new(TargetState {
                    overflow: scroll,
                    scroll,
                }),
                smooth: RefCell::new(SmoothState {
                    run: false,
                    start: 0.0,
                    target: 0.0,
                    time: 0.0,
                }),
                drag: RefCell::new(DragState {
                    cursor_start: 0.0,
                    scroll_start: 0.0,
                    scroll_per_px: 0.0,
                }),
            }),
        });

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar.props.target.on_update(move |_, _| {
            cb_scroll_bar.refresh();
        });

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar.props.target.on_children_added(move |_, _| {
            cb_scroll_bar.refresh();
        });

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar.props.target.on_children_removed(move |_, _| {
            cb_scroll_bar.refresh();
        });

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar
            .props
            .target
            .on_scroll(move |_, _, scroll_y, scroll_x| {
                match cb_scroll_bar.props.axis {
                    ScrollAxis::X => {
                        if scroll_x != 0.0 {
                            cb_scroll_bar.scroll(scroll_x * cb_scroll_bar.props.step);
                        }
                    },
                    ScrollAxis::Y => {
                        if scroll_y != 0.0 {
                            cb_scroll_bar.scroll(scroll_y * cb_scroll_bar.props.step);
                        }
                    },
                }

                Default::default()
            });

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar
            .container
            .on_scroll(move |_, _, scroll_y, scroll_x| {
                match cb_scroll_bar.props.axis {
                    ScrollAxis::X => {
                        if scroll_x != 0.0 {
                            cb_scroll_bar.scroll(scroll_x * cb_scroll_bar.props.step);
                        }
                    },
                    ScrollAxis::Y => {
                        if scroll_y != 0.0 {
                            cb_scroll_bar.scroll(scroll_y * cb_scroll_bar.props.step);
                        }
                    },
                }

                Default::default()
            });

        let bar_held = Arc::new(AtomicBool::new(false));
        let cb_scroll_bar = scroll_bar.clone();
        let cb_bar_held = bar_held.clone();

        scroll_bar
            .bar
            .on_press(MouseButton::Left, move |_, w_state, _| {
                let [cursor_x, cursor_y] = w_state.cursor_pos();

                let cursor_start = match cb_scroll_bar.props.axis {
                    ScrollAxis::X => cursor_x,
                    ScrollAxis::Y => cursor_y,
                };

                let state = cb_scroll_bar.state.lock();
                state.smooth.borrow_mut().run = false;

                let mut drag_state = state.drag.borrow_mut();
                drag_state.cursor_start = cursor_start;
                drag_state.scroll_start = state.target.borrow().scroll;

                cb_bar_held.store(true, atomic::Ordering::SeqCst);
                Default::default()
            });

        let cb_bar_held = bar_held.clone();

        scroll_bar
            .bar
            .on_release(MouseButton::Left, move |_, _, _| {
                cb_bar_held.store(false, atomic::Ordering::SeqCst);
                Default::default()
            });

        let cb_bar_held = bar_held;
        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar
            .container
            .attach_input_hook(window.on_cursor(move |_, w_state, _| {
                if cb_bar_held.load(atomic::Ordering::SeqCst) {
                    let [cursor_x, cursor_y] = w_state.cursor_pos();
                    let state = cb_scroll_bar.state.lock();

                    let jump_to = {
                        let drag_state = state.drag.borrow_mut();

                        let delta = match cb_scroll_bar.props.axis {
                            ScrollAxis::X => cursor_x - drag_state.cursor_start,
                            ScrollAxis::Y => cursor_y - drag_state.cursor_start,
                        };

                        drag_state.scroll_start + (delta * drag_state.scroll_per_px)
                    };

                    cb_scroll_bar.jump_to(jump_to);
                }

                Default::default()
            }));

        let cb_scroll_bar = scroll_bar.clone();

        scroll_bar
            .confine
            .on_press(MouseButton::Left, move |_, w_state, _| {
                let [cursor_x, cursor_y] = w_state.cursor_pos();
                let bar_bpu = cb_scroll_bar.bar.post_update();
                let state = cb_scroll_bar.state.lock();

                let delta = match cb_scroll_bar.props.axis {
                    ScrollAxis::X => {
                        cursor_x - (((bar_bpu.tri[0] - bar_bpu.tli[0]) / 2.0) + bar_bpu.tli[0])
                    },
                    ScrollAxis::Y => {
                        cursor_y - (((bar_bpu.bli[1] - bar_bpu.tli[1]) / 2.0) + bar_bpu.tli[1])
                    },
                };

                let scroll_to =
                    state.target.borrow().scroll + (delta * state.drag.borrow().scroll_per_px);

                cb_scroll_bar.scroll_to(scroll_to);
                Default::default()
            });

        let cb_scroll_bar = scroll_bar.clone();

        button_hooks(
            &scroll_bar.upright,
            BtnHookColors {
                vert_clr: Some(scroll_bar.theme.colors.border1),
                h_vert_clr: Some(scroll_bar.theme.colors.border3),
                p_vert_clr: Some(scroll_bar.theme.colors.border2),
                ..Default::default()
            },
            move |_| {
                cb_scroll_bar.scroll(-cb_scroll_bar.props.step);
            },
        );

        let cb_scroll_bar = scroll_bar.clone();

        button_hooks(
            &scroll_bar.downleft,
            BtnHookColors {
                vert_clr: Some(scroll_bar.theme.colors.border1),
                h_vert_clr: Some(scroll_bar.theme.colors.border3),
                p_vert_clr: Some(scroll_bar.theme.colors.border2),
                ..Default::default()
            },
            move |_| {
                cb_scroll_bar.scroll(cb_scroll_bar.props.step);
            },
        );

        scroll_bar.style_update();
        scroll_bar
    }
}

/// Scroll bar widget
pub struct ScrollBar {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    upright: Arc<Bin>,
    downleft: Arc<Bin>,
    confine: Arc<Bin>,
    bar: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    target: RefCell<TargetState>,
    smooth: RefCell<SmoothState>,
    drag: RefCell<DragState>,
}

struct TargetState {
    overflow: f32,
    scroll: f32,
}

struct SmoothState {
    run: bool,
    start: f32,
    target: f32,
    time: f32,
}

struct DragState {
    cursor_start: f32,
    scroll_start: f32,
    scroll_per_px: f32,
}

impl ScrollBar {
    /// Scroll an amount of pixels.
    ///
    /// **Notes**:
    /// - This may be effected by acceleration.
    /// - If smooth scroll or acceleration are both disabled this uses [`ScrollBar::jump`].
    pub fn scroll(self: &Arc<Self>, amt: f32) {
        let state = self.state.lock();

        if !self.props.accel && !self.props.smooth {
            self.scroll_no_anim(amt);
            return;
        }

        let target_state = state.target.borrow();
        let mut smooth_state = state.smooth.borrow_mut();

        smooth_state.target = if !smooth_state.run {
            target_state.scroll + amt
        } else {
            let direction_changes =
                (smooth_state.target - target_state.scroll).signum() != amt.signum();

            if self.props.accel {
                if direction_changes {
                    target_state.scroll + amt
                } else {
                    smooth_state.target
                        + (((smooth_state.target - target_state.scroll).abs() / self.props.step)
                            .max(1.0)
                            .powf(self.props.accel_pow)
                            .clamp(1.0, self.props.max_accel_mult)
                            * amt)
                }
            } else {
                if direction_changes {
                    target_state.scroll + amt
                } else {
                    smooth_state.target + amt
                }
            }
        };

        if smooth_state.target == target_state.scroll {
            return;
        }

        if !smooth_state.run {
            smooth_state.run = true;
            self.run_smooth_scroll();
        }

        smooth_state.start = target_state.scroll;
        smooth_state.time = 0.0;
    }

    /// Scroll to a certain amount of pixels.
    ///
    /// **Note**: If smooth scroll or acceleration are both disabled this uses [`ScrollBar::jump_to`].
    pub fn scroll_to(self: &Arc<Self>, to: f32) {
        let state = self.state.lock();

        if !self.props.accel && !self.props.smooth {
            self.jump_to(to);
            return;
        }

        let target_state = state.target.borrow();
        let mut smooth_state = state.smooth.borrow_mut();

        if target_state.scroll == to {
            smooth_state.run = false;
            return;
        }

        if !smooth_state.run {
            smooth_state.run = true;
            self.run_smooth_scroll();
        }

        smooth_state.start = target_state.scroll;
        smooth_state.target = to;
        smooth_state.time = 0.0;
    }

    /// Scroll to the minimum.
    ///
    /// If [`ScrollAxis`] is `Y` this it the top. If `X` then the left.
    ///
    /// **Note**: If smooth scroll or acceleration are both disabled this uses [`ScrollBar::jump_to_min`].
    pub fn scroll_to_min(self: &Arc<Self>) {
        self.scroll_to(0.0);
    }

    /// Scroll to the maximum.
    ///
    /// If [`ScrollAxis`] is `Y` this it the bottom. If `X` then the right.
    ///
    /// **Note**: If smooth scroll or acceleration are both disabled this uses [`ScrollBar::jump_to_max`].
    pub fn scroll_to_max(self: &Arc<Self>) {
        let state = self.state.lock();
        let max = state.target.borrow().overflow;
        self.scroll_to(max);
    }

    fn scroll_no_anim(&self, amt: f32) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();

            if amt.is_sign_negative() {
                if target_state.scroll != 0.0 {
                    if target_state.scroll + amt < 0.0 {
                        target_state.scroll = 0.0;
                    } else {
                        target_state.scroll += amt;
                    }

                    update = true;
                }
            } else {
                if target_state.scroll != target_state.overflow {
                    if target_state.scroll + amt > target_state.overflow {
                        target_state.scroll = target_state.overflow;
                    } else {
                        target_state.scroll += amt;
                    }

                    update = true;
                }
            }
        }

        if update {
            self.update();
        }
    }

    /// Jump an amount of pixels.
    ///
    /// **Note**: This is the same as [`ScrollBar::scroll`] but does not animate or accelerate.
    pub fn jump(&self, amt: f32) {
        let state = self.state.lock();
        state.smooth.borrow_mut().run = false;
        self.scroll_no_anim(amt);
    }

    /// Jump to a certain amount of pixels.
    ///
    /// **Note**: This is the same as [`ScrollBar::scroll_to`] but does not animate.
    pub fn jump_to(&self, to: f32) {
        self.jump_to_inner(to, true);
    }

    fn jump_to_inner(&self, to: f32, cancel_smooth: bool) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();

            if cancel_smooth {
                state.smooth.borrow_mut().run = false;
            }

            if to > target_state.overflow {
                if target_state.scroll != target_state.overflow {
                    target_state.scroll = target_state.overflow;
                    update = true;
                }
            } else if to < 0.0 {
                if target_state.scroll != 0.0 {
                    target_state.scroll = 0.0;
                    update = true;
                }
            } else {
                if target_state.scroll != to {
                    target_state.scroll = to;
                    update = true;
                }
            }
        }

        if update {
            self.update();
        }
    }

    /// Jump to the minimum.
    ///
    /// If [`ScrollAxis`] is `Y` this it the top. If `X` then the left.
    ///
    /// **Note**: This is the same as [`ScrollBar::scroll_to_min`] but does not animate.
    pub fn jump_to_min(&self) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();
            state.smooth.borrow_mut().run = false;

            if target_state.scroll != 0.0 {
                target_state.scroll = 0.0;
                update = true;
            }
        }

        if update {
            self.update();
        }
    }

    /// Jump to the minimum.
    ///
    /// If [`ScrollAxis`] is `Y` this it the bottom. If `X` then the right.
    ///
    /// **Note**: This is the same as [`ScrollBar::scroll_to_max`] but does not animate.
    pub fn jump_to_max(&self) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();
            state.smooth.borrow_mut().run = false;

            if target_state.scroll != target_state.overflow {
                target_state.scroll = target_state.overflow;
                update = true;
            }
        }

        if update {
            self.update();
        }
    }

    /// Recheck the state and update if needed.
    ///
    /// **Note**: This may need to be called in certain cases.
    pub fn refresh(&self) {
        let _state = self.state.lock();

        if self.check_target_state() {
            self.update();
        }
    }

    /// The amount of overflow the target has.
    pub fn target_overflow(&self) -> f32 {
        match self.props.axis {
            ScrollAxis::X => self.props.target.calc_hori_overflow(),
            ScrollAxis::Y => self.props.target.calc_vert_overflow(),
        }
    }

    /// The current amount the target is scrolled.
    pub fn current_scroll(&self) -> f32 {
        self.state.lock().target.borrow().scroll
    }

    /// The amount the target will be scrolled after animations.
    ///
    /// This will be the same value as [`ScrollBar::current_scroll`] when:
    /// - Smooth scroll and acceleration are disabled.
    /// - There is no animiation currently happening.
    pub fn target_scroll(&self) -> f32 {
        let state = self.state.lock();
        let smooth_state = state.smooth.borrow();

        if !smooth_state.run {
            return smooth_state.target;
        }

        state.target.borrow().scroll
    }

    // TODO: Public?
    pub(crate) fn size(theme: &Theme) -> f32 {
        (theme.base_size / 1.5) + theme.border.unwrap_or(0.0)
    }

    pub(crate) fn has_bin_id(&self, bin_id: BinID) -> bool {
        bin_id == self.container.id()
            || bin_id == self.upright.id()
            || bin_id == self.downleft.id()
            || bin_id == self.confine.id()
            || bin_id == self.bar.id()
    }

    fn check_target_state(&self) -> bool {
        let state = self.state.lock();
        let target_overflow = self.target_overflow();
        let mut target_state = state.target.borrow_mut();
        let mut update = false;

        if target_state.overflow != target_overflow {
            target_state.overflow = target_overflow;
            update = true;
        }

        if target_overflow < target_state.scroll {
            target_state.scroll = target_overflow;
            update = true;
        }

        update
    }

    fn run_smooth_scroll(self: &Arc<Self>) {
        if let Some(window) = self.container.window() {
            let scroll_bar = self.clone();
            let animation_duration = self.props.animation_duration.as_micros() as f32 / 1000.0;

            window.renderer_on_frame(move |elapsed_op| {
                let state = scroll_bar.state.lock();
                let mut smooth_state = state.smooth.borrow_mut();

                if !smooth_state.run {
                    return false;
                }

                if let Some(elapsed) = elapsed_op {
                    smooth_state.time += elapsed.as_micros() as f32 / 1000.0;
                }

                let delta = smooth_state.target - smooth_state.start;
                let linear_t = (smooth_state.time / animation_duration).clamp(0.0, 1.0);
                let smooth_t = (((linear_t + 1.5) * PI).sin() + 1.0) / 2.0;
                scroll_bar.jump_to_inner(smooth_state.start + (delta * smooth_t), false);
                smooth_state.run = smooth_state.time < animation_duration;
                smooth_state.run
            });
        }
    }

    fn update(&self) {
        let state = self.state.lock();
        let target_state = state.target.borrow();
        let confine_bpu = self.confine.post_update();

        let confine_size = match self.props.axis {
            ScrollAxis::X => confine_bpu.tri[0] - confine_bpu.tli[0],
            ScrollAxis::Y => confine_bpu.bli[1] - confine_bpu.tli[1],
        };

        let space_size =
            (target_state.overflow / self.props.step).min(confine_size - self.theme.base_size);

        let scroll_per_px = target_state.overflow / space_size;
        state.drag.borrow_mut().scroll_per_px = scroll_per_px;
        let bar_size_pct = ((confine_size - space_size) / confine_size) * 100.0;

        let bar_offset_pct = ((target_state.scroll / scroll_per_px) / confine_size) * 100.0;
        let mut bar_style = self.bar.style_copy();
        let mut target_style = self.props.target.style_copy();
        let mut target_style_update = false;

        match self.props.axis {
            ScrollAxis::X => {
                if target_style.scroll_x.is_none()
                    || target_style.scroll_x.unwrap() != target_state.scroll
                {
                    target_style.scroll_x = Some(target_state.scroll);
                    target_style_update = true;

                    bar_style.pos_from_l_pct = Some(bar_offset_pct);
                    bar_style.width_pct = Some(bar_size_pct);
                }
            },
            ScrollAxis::Y => {
                if target_style.scroll_y.is_none()
                    || target_style.scroll_y.unwrap() != target_state.scroll
                {
                    target_style.scroll_y = Some(target_state.scroll);
                    target_style_update = true;

                    bar_style.pos_from_t_pct = Some(bar_offset_pct);
                    bar_style.height_pct = Some(bar_size_pct);
                }
            },
        }

        if target_style_update {
            Bin::style_update_batch([(&self.props.target, target_style), (&self.bar, bar_style)]);
        } else {
            self.bar.style_update(bar_style).expect_valid();
        }
    }

    fn style_update(&self) {
        let size = (self.theme.base_size / 1.5).ceil();
        let spacing = (self.theme.spacing / 10.0).ceil();
        let border_size = self.theme.border.unwrap_or(0.0);

        let mut container_style = BinStyle {
            position: Some(BinPosition::Parent),
            back_color: Some(self.theme.colors.back2),
            ..Default::default()
        };

        let mut upright_style = BinStyle {
            position: Some(BinPosition::Parent),
            ..Default::default()
        };

        let mut downleft_style = BinStyle {
            position: Some(BinPosition::Parent),
            ..Default::default()
        };

        let mut confine_style = BinStyle {
            position: Some(BinPosition::Parent),
            ..Default::default()
        };

        let mut bar_style = BinStyle {
            position: Some(BinPosition::Parent),
            back_color: Some(self.theme.colors.accent1),
            ..Default::default()
        };

        match self.props.axis {
            ScrollAxis::X => {
                container_style.pos_from_b = Some(0.0);
                container_style.pos_from_l = Some(0.0);
                container_style.pos_from_r = Some(0.0);
                container_style.height = Some(size);

                upright_style.pos_from_t = Some(0.0);
                upright_style.pos_from_b = Some(0.0);
                upright_style.pos_from_r = Some(0.0);
                upright_style.width = Some(size);
                upright_style.custom_verts =
                    right_symbol_verts(size, spacing, self.theme.colors.border1);

                downleft_style.pos_from_t = Some(0.0);
                downleft_style.pos_from_b = Some(0.0);
                downleft_style.pos_from_l = Some(0.0);
                downleft_style.width = Some(size);
                downleft_style.custom_verts =
                    left_symbol_verts(size, spacing, self.theme.colors.border1);

                confine_style.pos_from_t = Some(spacing);
                confine_style.pos_from_b = Some(spacing);
                confine_style.pos_from_l = Some(size + border_size);
                confine_style.pos_from_r = Some(size + border_size);
                confine_style.overflow_x = Some(true);

                bar_style.pos_from_t = Some(0.0);
                bar_style.pos_from_b = Some(0.0);
                bar_style.pos_from_l_pct = Some(0.0);
                bar_style.width_pct = Some(100.0);
            },
            ScrollAxis::Y => {
                container_style.pos_from_t = Some(0.0);
                container_style.pos_from_b = Some(0.0);
                container_style.pos_from_r = Some(0.0);
                container_style.width = Some(size);

                upright_style.pos_from_t = Some(0.0);
                upright_style.pos_from_l = Some(0.0);
                upright_style.pos_from_r = Some(0.0);
                upright_style.height = Some(size);
                upright_style.custom_verts =
                    up_symbol_verts(size, spacing, self.theme.colors.border1);

                downleft_style.pos_from_b = Some(0.0);
                downleft_style.pos_from_l = Some(0.0);
                downleft_style.pos_from_r = Some(0.0);
                downleft_style.height = Some(size);
                downleft_style.custom_verts =
                    down_symbol_verts(size, spacing, self.theme.colors.border1);

                confine_style.pos_from_t = Some(size + border_size);
                confine_style.pos_from_b = Some(size + border_size);
                confine_style.pos_from_l = Some(spacing);
                confine_style.pos_from_r = Some(spacing);
                confine_style.overflow_y = Some(true);

                bar_style.pos_from_t_pct = Some(0.0);
                bar_style.pos_from_l = Some(0.0);
                bar_style.pos_from_r = Some(0.0);
                bar_style.height_pct = Some(100.0);
            },
        }

        if let Some(border_size) = self.theme.border {
            bar_style.border_size_t = Some(border_size);
            bar_style.border_size_b = Some(border_size);
            bar_style.border_size_l = Some(border_size);
            bar_style.border_size_r = Some(border_size);
            bar_style.border_color_t = Some(self.theme.colors.border3);
            bar_style.border_color_b = Some(self.theme.colors.border3);
            bar_style.border_color_l = Some(self.theme.colors.border3);
            bar_style.border_color_r = Some(self.theme.colors.border3);

            match self.props.axis {
                ScrollAxis::X => {
                    container_style.border_size_t = Some(border_size);
                    container_style.border_color_t = Some(self.theme.colors.border1);
                },
                ScrollAxis::Y => {
                    container_style.border_size_l = Some(border_size);
                    container_style.border_color_l = Some(self.theme.colors.border1);
                },
            }
        }

        if self.theme.roundness.is_some() {
            let bar_size_1_2 = (size - (spacing * 2.0)) / 2.0;
            bar_style.border_radius_tl = Some(bar_size_1_2);
            bar_style.border_radius_tr = Some(bar_size_1_2);
            bar_style.border_radius_bl = Some(bar_size_1_2);
            bar_style.border_radius_br = Some(bar_size_1_2);
        }

        Bin::style_update_batch([
            (&self.container, container_style),
            (&self.upright, upright_style),
            (&self.downleft, downleft_style),
            (&self.confine, confine_style),
            (&self.bar, bar_style),
        ]);
    }
}

fn up_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POINTS: [[f32; 2]; 3] = [[0.5, 0.25], [0.0, 0.75], [1.0, 0.75]];
    symbol_verts(target_size, spacing, color, &UNIT_POINTS)
}

pub(crate) fn down_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POINTS: [[f32; 2]; 3] = [[0.0, 0.25], [1.0, 0.25], [0.5, 0.75]];
    symbol_verts(target_size, spacing, color, &UNIT_POINTS)
}

fn left_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POINTS: [[f32; 2]; 3] = [[0.75, 0.25], [0.25, 0.5], [0.75, 0.75]];
    symbol_verts(target_size, spacing, color, &UNIT_POINTS)
}

fn right_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POINTS: [[f32; 2]; 3] = [[0.25, 0.25], [0.25, 0.75], [0.75, 0.5]];
    symbol_verts(target_size, spacing, color, &UNIT_POINTS)
}

fn symbol_verts(
    target_size: f32,
    spacing: f32,
    color: Color,
    unit_points: &[[f32; 2]; 3],
) -> Vec<BinVert> {
    let size = target_size - (spacing * 2.0);
    let mut verts = Vec::with_capacity(3);

    for [x, y] in unit_points.iter() {
        verts.push(BinVert {
            position: ((*x * size) + spacing, (*y * size) + spacing, 0),
            color,
        });
    }

    verts
}
