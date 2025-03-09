use std::cell::RefCell;
use std::sync::Arc;

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

/// Builder for [`ProgressBar`].
pub struct ProgressBarBuilder {
    widget: WidgetBuilder,
    props: Properties,
    on_press: Vec<Box<dyn FnMut(&Arc<ProgressBar>, f32) + Send + 'static>>,
}

#[derive(Default)]
struct Properties {
    pct: f32,
    width: Option<f32>,
    height: Option<f32>,
}

impl ProgressBarBuilder {
    pub(crate) fn with_builder(builder: WidgetBuilder) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
            on_press: Vec::new(),
        }
    }

    /// Set the initial percent.
    ///
    /// **Note**: When this isn't used the percent will be `0.0`.
    pub fn set_pct(mut self, pct: f32) -> Self {
        self.props.pct = pct.clamp(0.0, 100.0);
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

    /// Add a callback to be called when the [`ProgressBar`] is pressed.
    ///
    /// The callback is called with the cursors percent along the [`ProgressBar`].
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(&Arc<ProgressBar>, f32) + Send + 'static,
    {
        self.on_press.push(Box::new(on_press));
        self
    }

    /// Finish building the [`ProgressBar`].
    pub fn build(self) -> Arc<ProgressBar> {
        let window = self.widget.parent.window();
        let mut bins = window.new_bins(2).into_iter();
        let container = bins.next().unwrap();
        let fill = bins.next().unwrap();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => unimplemented!(),
        }

        container.add_child(fill.clone());
        let initial_pct = self.props.pct;

        let progress_bar = Arc::new(ProgressBar {
            theme: self.widget.theme,
            props: self.props,
            container,
            fill,
            state: ReentrantMutex::new(State {
                pct: RefCell::new(initial_pct),
                on_press: RefCell::new(self.on_press),
            }),
        });

        let cb_progress_bar = progress_bar.clone();

        progress_bar
            .container
            .on_press(MouseButton::Left, move |_, w_state, _| {
                cb_progress_bar.proc_press(w_state.cursor_pos());
                Default::default()
            });

        let cb_progress_bar = progress_bar.clone();

        progress_bar
            .fill
            .on_press(MouseButton::Left, move |_, w_state, _| {
                cb_progress_bar.proc_press(w_state.cursor_pos());
                Default::default()
            });

        progress_bar.style_update();
        progress_bar
    }
}

/// Progress bar widget
pub struct ProgressBar {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    fill: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    pct: RefCell<f32>,
    on_press: RefCell<Vec<Box<dyn FnMut(&Arc<ProgressBar>, f32) + Send + 'static>>>,
}

impl ProgressBar {
    /// Set the percent
    pub fn set_pct(self: &Arc<Self>, pct: f32) {
        let pct = pct.clamp(0.0, 100.0);

        self.fill
            .style_update(BinStyle {
                width_pct: Some(pct),
                ..self.fill.style_copy()
            })
            .expect_valid();

        *self.state.lock().pct.borrow_mut() = pct;
    }

    /// Get the current percent
    pub fn pct(&self) -> f32 {
        *self.state.lock().pct.borrow()
    }

    /// Add a callback to be called when the [`ProgressBar`] is pressed.
    ///
    /// The callback is called with the cursors percent along the [`ProgressBar`].
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_press<F>(&self, on_press: F)
    where
        F: FnMut(&Arc<ProgressBar>, f32) + Send + 'static,
    {
        self.state
            .lock()
            .on_press
            .borrow_mut()
            .push(Box::new(on_press));
    }

    fn proc_press(self: &Arc<Self>, cursor: [f32; 2]) {
        let bpu = self.container.post_update();

        let pct =
            (((cursor[0] - bpu.tli[0]) / (bpu.tri[0] - bpu.tli[0])) * 100.0).clamp(0.0, 100.0);

        let state = self.state.lock();

        for on_press in state.on_press.borrow_mut().iter_mut() {
            on_press(self, pct);
        }
    }

    fn style_update(self: &Arc<Self>) {
        let widget_height = match self.props.height {
            Some(height) => height,
            None => self.theme.spacing * 2.0,
        };

        let widget_width = match self.props.width {
            Some(width) => width,
            None => self.theme.spacing * 8.0,
        };

        let widget_height_1_2 = widget_height / 2.0;
        let pct = *self.state.lock().pct.borrow();

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            height: Some(widget_height),
            width: Some(widget_width),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            back_color: Some(self.theme.colors.back2),
            ..Default::default()
        };

        let mut fill_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_l: Some(0.0),
            width_pct: Some(pct),
            back_color: Some(self.theme.colors.accent1),
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
        }

        if let Some(roundness) = self.theme.roundness {
            let radius = widget_height_1_2.min(roundness);
            container_style.border_radius_tl = Some(radius);
            container_style.border_radius_tr = Some(radius);
            container_style.border_radius_bl = Some(radius);
            container_style.border_radius_br = Some(radius);

            fill_style.border_radius_tr = Some(radius);
            fill_style.border_radius_br = Some(radius);
        }

        self.container.style_update(container_style).expect_valid();
        self.fill.style_update(fill_style).expect_valid();
    }
}
