use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use basalt::interface::{Bin, BinPosition, BinStyle, BinVert, Color};
use basalt::interval::{IntvlHookCtrl, IntvlHookID};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetContainer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    X,
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
    update_intvl: Duration,
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
            update_intvl: Duration::from_secs(1) / 120,
            animation_duration: Duration::from_millis(100),
        }
    }
}

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

    pub fn scroll(mut self, scroll: f32) -> Self {
        self.initial_state.scroll = Some(scroll);
        self
    }

    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.props.axis = axis;
        self
    }

    pub fn smooth(mut self, smooth: bool) -> Self {
        self.props.smooth = smooth;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.props.step = step;
        self
    }

    pub fn accel(mut self, accel: bool) -> Self {
        self.props.accel = accel;
        self
    }

    pub fn accel_pow(mut self, accel_pow: f32) -> Self {
        self.props.accel_pow = accel_pow;
        self
    }

    pub fn max_accel_mult(mut self, max_accel_mult: f32) -> Self {
        self.props.max_accel_mult = max_accel_mult;
        self
    }

    pub fn update_hz(mut self, update_hz: u32) -> Self {
        self.props.update_intvl = Duration::from_secs(1) / update_hz;
        self
    }

    pub fn animation_duration(mut self, animation_duration: Duration) -> Self {
        self.props.animation_duration = animation_duration;
        self
    }

    pub fn build(self) -> Arc<ScrollBar> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut new_bins = window.new_bins(5).into_iter();
        let container = new_bins.next().unwrap();
        let incr = new_bins.next().unwrap();
        let decr = new_bins.next().unwrap();
        let confine = new_bins.next().unwrap();
        let bar = new_bins.next().unwrap();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        container.add_child(incr.clone());
        container.add_child(decr.clone());
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
            incr,
            decr,
            confine,
            bar,
            state: ReentrantMutex::new(State {
                target: RefCell::new(TargetState {
                    overflow: scroll,
                    scroll,
                }),
                smooth: RefCell::new(SmoothState {
                    target: scroll,
                    steps: VecDeque::new(),
                    intvl_hook_id: None,
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

        if scroll_bar.props.smooth || scroll_bar.props.accel {
            let scroll_bar_wk = Arc::downgrade(&scroll_bar);

            let intvl_hook_id = window.basalt_ref().interval_ref().do_every(
                scroll_bar.props.update_intvl,
                None,
                move |_| {
                    let scroll_bar = match scroll_bar_wk.upgrade() {
                        Some(some) => some,
                        None => return IntvlHookCtrl::Remove,
                    };

                    let state = scroll_bar.state.lock();
                    let mut smooth_state = state.smooth.borrow_mut();

                    if smooth_state.steps.is_empty() {
                        return IntvlHookCtrl::Pause;
                    }

                    scroll_bar.scroll_na(smooth_state.steps.pop_front().unwrap());
                    IntvlHookCtrl::Continue
                },
            );

            scroll_bar.state.lock().smooth.borrow_mut().intvl_hook_id = Some(intvl_hook_id);
        }

        // TODO: Incr/Decr button hooks
        // TODO: Bar drag hooks
        // TODO: Track click hooks

        scroll_bar.style_update();
        scroll_bar
    }
}

pub struct ScrollBar {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    incr: Arc<Bin>,
    decr: Arc<Bin>,
    confine: Arc<Bin>,
    bar: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    target: RefCell<TargetState>,
    smooth: RefCell<SmoothState>,
}

struct TargetState {
    overflow: f32,
    scroll: f32,
}

struct SmoothState {
    target: f32,
    steps: VecDeque<f32>,
    intvl_hook_id: Option<IntvlHookID>,
}

impl ScrollBar {
    pub fn scroll(&self, amt: f32) {
        let state = self.state.lock();

        if !self.props.accel && !self.props.smooth {
            self.scroll_na(amt);
            return;
        }

        let target_state = state.target.borrow();
        let mut smooth_state = state.smooth.borrow_mut();

        smooth_state.target = if smooth_state.steps.is_empty() {
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

        smooth_state.steps =
            VecDeque::from_iter(self.animation_steps(target_state.scroll, smooth_state.target));

        self.container
            .basalt_ref()
            .interval_ref()
            .start(smooth_state.intvl_hook_id.unwrap());
    }

    fn scroll_na(&self, amt: f32) {
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

    pub fn jump_to(&self, to: f32) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();

            if to > target_state.overflow {
                if target_state.scroll != target_state.overflow {
                    target_state.scroll = target_state.overflow;
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

    pub fn jump_to_top(&self) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();

            if target_state.scroll != 0.0 {
                target_state.scroll = 0.0;
                update = true;
            }
        }

        if update {
            self.update();
        }
    }

    pub fn jump_to_bottom(&self) {
        let state = self.state.lock();
        let mut update = self.check_target_state();

        {
            let mut target_state = state.target.borrow_mut();

            if target_state.scroll != target_state.overflow {
                target_state.scroll = target_state.overflow;
                update = true;
            }
        }

        if update {
            self.update();
        }
    }

    pub fn refresh(&self) {
        let _state = self.state.lock();

        if self.check_target_state() {
            self.update();
        }
    }

    pub fn target_overflow(&self) -> f32 {
        match self.props.axis {
            ScrollAxis::X => self.props.target.calc_hori_overflow(),
            ScrollAxis::Y => self.props.target.calc_vert_overflow(),
        }
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

    fn animation_steps(&self, current: f32, target: f32) -> Vec<f32> {
        const ANIM_POW: f32 = 0.2;
        let delta = target - current;

        let step_count = (self.props.animation_duration.as_micros() as f32
            / self.props.update_intvl.as_micros() as f32)
            .round() as usize;

        let step_base = delta
            / (1..=step_count)
                .into_iter()
                .map(|i| (i as f32 / step_count as f32).powf(ANIM_POW))
                .sum::<f32>();

        (1..=step_count)
            .into_iter()
            .map(|i| (i as f32 / step_count as f32).powf(ANIM_POW) * step_base)
            .collect()
    }

    fn update(&self) {
        let state = self.state.lock();
        let target_state = state.target.borrow();
        let mut bar_style = self.bar.style_copy();

        let [bar_size_pct, bar_offset_pct] = if target_state.overflow <= 0.0 {
            [100.0, 0.0]
        } else {
            let bar_space = (target_state.overflow / self.props.step).clamp(10.0, 100.0);

            [
                100.0 - bar_space,
                (target_state.scroll / target_state.overflow) * bar_space,
            ]
        };

        // TODO: Remove This
        /*println!(
            "Overflow: {} Px, Scroll: {} Px, Bar Size: {:.1} %, Bar Offset: {:.1} %",
            target_state.overflow, target_state.scroll, bar_size_pct, bar_offset_pct
        );*/

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

        let mut incr_style = BinStyle {
            position: Some(BinPosition::Parent),
            ..Default::default()
        };

        let mut decr_style = BinStyle {
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

                incr_style.pos_from_t = Some(0.0);
                incr_style.pos_from_b = Some(0.0);
                incr_style.pos_from_r = Some(0.0);
                incr_style.width = Some(size);
                incr_style.custom_verts =
                    right_symbol_verts(size, spacing, self.theme.colors.text1a);

                decr_style.pos_from_t = Some(0.0);
                decr_style.pos_from_b = Some(0.0);
                decr_style.pos_from_l = Some(0.0);
                decr_style.width = Some(size);
                decr_style.custom_verts =
                    left_symbol_verts(size, spacing, self.theme.colors.text1a);

                confine_style.pos_from_t = Some(spacing);
                confine_style.pos_from_b = Some(spacing);
                confine_style.pos_from_l = Some(size + border_size);
                confine_style.pos_from_r = Some(size + border_size);

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

                incr_style.pos_from_t = Some(0.0);
                incr_style.pos_from_l = Some(0.0);
                incr_style.pos_from_r = Some(0.0);
                incr_style.height = Some(size);
                incr_style.custom_verts = up_symbol_verts(size, spacing, self.theme.colors.text1a);

                decr_style.pos_from_b = Some(0.0);
                decr_style.pos_from_l = Some(0.0);
                decr_style.pos_from_r = Some(0.0);
                decr_style.height = Some(size);
                decr_style.custom_verts =
                    down_symbol_verts(size, spacing, self.theme.colors.text1a);

                confine_style.pos_from_t = Some(size + border_size);
                confine_style.pos_from_b = Some(size + border_size);
                confine_style.pos_from_l = Some(spacing);
                confine_style.pos_from_r = Some(spacing);

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
            (&self.incr, incr_style),
            (&self.decr, decr_style),
            (&self.confine, confine_style),
            (&self.bar, bar_style),
        ]);
    }
}

fn up_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POINTS: [[f32; 2]; 3] = [[0.5, 0.25], [0.0, 0.75], [1.0, 0.75]];
    symbol_verts(target_size, spacing, color, &UNIT_POINTS)
}

fn down_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
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
