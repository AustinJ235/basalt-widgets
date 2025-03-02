use std::cell::RefCell;
use std::sync::Arc;

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

/// Builder for [`SwitchButton`]
pub struct SwitchButtonBuilder {
    widget: WidgetBuilder,
    props: Properties,
    on_change: Vec<Box<dyn FnMut(&Arc<SwitchButton>, bool) + Send + 'static>>,
}

#[derive(Default)]
struct Properties {
    enabled: bool,
    width: Option<f32>,
    height: Option<f32>,
}

impl SwitchButtonBuilder {
    pub(crate) fn with_builder(builder: WidgetBuilder) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
            on_change: Vec::new(),
        }
    }

    /// Set the initial enabled state.
    ///
    /// **Note**: When this isn't used the initial value will be `false`.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.props.enabled = enabled;
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

    /// Add a callback to be called when the [`SwitchButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<SwitchButton>, bool) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`SwitchButton`].
    pub fn build(self) -> Arc<SwitchButton> {
        let window = self.widget.parent.window();
        let mut bins = window.new_bins(2).into_iter();
        let container = bins.next().unwrap();
        let knob = bins.next().unwrap();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => unimplemented!(),
        }

        container.add_child(knob.clone());
        let enabled = self.props.enabled;

        let switch_button = Arc::new(SwitchButton {
            theme: self.widget.theme,
            props: self.props,
            container,
            knob,
            state: ReentrantMutex::new(State {
                enabled: RefCell::new(enabled),
                on_change: RefCell::new(self.on_change),
            }),
        });

        let cb_switch_button = switch_button.clone();

        switch_button
            .container
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_switch_button.toggle();
                Default::default()
            });

        let cb_switch_button = switch_button.clone();

        switch_button
            .knob
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_switch_button.toggle();
                Default::default()
            });

        switch_button.style_update();
        switch_button
    }
}

/// Switch button widget
pub struct SwitchButton {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    knob: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    enabled: RefCell<bool>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<SwitchButton>, bool) + Send + 'static>>>,
}

impl SwitchButton {
    /// Set the enabled state.
    pub fn set(self: &Arc<Self>, enabled: bool) {
        let state = self.state.lock();
        *state.enabled.borrow_mut() = enabled;

        let widget_height = match self.props.height {
            Some(height) => height,
            None => self.theme.spacing * 2.0,
        };

        if enabled {
            self.container
                .style_update(BinStyle {
                    back_color: Some(self.theme.colors.accent1),
                    ..self.container.style_copy()
                })
                .expect_valid();

            self.knob
                .style_update(BinStyle {
                    pos_from_r: Some(widget_height * 0.1),
                    pos_from_l: None,
                    ..self.knob.style_copy()
                })
                .expect_valid();
        } else {
            self.container
                .style_update(BinStyle {
                    back_color: Some(self.theme.colors.back3),
                    ..self.container.style_copy()
                })
                .expect_valid();

            self.knob
                .style_update(BinStyle {
                    pos_from_l: Some(widget_height * 0.1),
                    pos_from_r: None,
                    ..self.knob.style_copy()
                })
                .expect_valid();
        }

        if let Ok(mut on_change_cbs) = state.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(self, enabled);
            }
        }
    }

    /// Toggle the enabled state returning the new enabled state.
    pub fn toggle(self: &Arc<Self>) -> bool {
        let state = self.state.lock();
        let enabled = !*state.enabled.borrow();
        self.set(enabled);
        enabled
    }

    /// Get the current enabled state.
    pub fn get(&self) -> bool {
        *self.state.lock().enabled.borrow()
    }

    /// Add a callback to be called when the [`SwitchButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<SwitchButton>, bool) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    fn style_update(&self) {
        let widget_height = match self.props.height {
            Some(height) => height,
            None => self.theme.spacing * 2.0,
        };

        let widget_width = match self.props.width {
            Some(width) => width.max(widget_height),
            None => widget_height * 2.0,
        };

        let enabled = *self.state.lock().enabled.borrow();

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            height: Some(widget_height),
            width: Some(widget_width),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            border_radius_tl: Some(widget_height / 2.0),
            border_radius_tr: Some(widget_height / 2.0),
            border_radius_bl: Some(widget_height / 2.0),
            border_radius_br: Some(widget_height / 2.0),
            ..Default::default()
        };

        let knob_size = widget_height - (widget_height * 0.2);

        let mut knob_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(widget_height * 0.1),
            pos_from_b: Some(widget_height * 0.1),
            width: Some(knob_size),
            back_color: Some(self.theme.colors.back1),
            border_radius_tl: Some(knob_size / 2.0),
            border_radius_tr: Some(knob_size / 2.0),
            border_radius_bl: Some(knob_size / 2.0),
            border_radius_br: Some(knob_size / 2.0),
            ..Default::default()
        };

        if enabled {
            container_style.back_color = Some(self.theme.colors.accent1);
            knob_style.pos_from_r = Some(widget_height * 0.1);
        } else {
            container_style.back_color = Some(self.theme.colors.back3);
            knob_style.pos_from_l = Some(widget_height * 0.1);
        }

        if let Some(border_size) = self.theme.border {
            container_style.border_size_t = Some(border_size);
            container_style.border_size_b = Some(border_size);
            container_style.border_size_l = Some(border_size);
            container_style.border_size_r = Some(border_size);
            container_style.border_color_t = Some(self.theme.colors.border1);
            container_style.border_color_b = Some(self.theme.colors.border1);
            container_style.border_color_l = Some(self.theme.colors.border1);
            container_style.border_color_r = Some(self.theme.colors.border1);

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
        self.knob.style_update(knob_style).expect_valid();
    }
}
