use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle};
use parking_lot::Mutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

pub struct SwitchButtonBuilder {
    widget: WidgetBuilder,
    props: Properties,
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
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.props.enabled = enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.props.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.props.height = Some(height);
        self
    }

    pub fn build(self) -> Arc<SwitchButton> {
        let window = self.widget.parent.window();
        let mut bins = window.new_bins(2).into_iter();
        let container = bins.next().unwrap();
        let knob = bins.next().unwrap();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => (),
        }

        container.add_child(knob.clone());
        let enabled = self.props.enabled;

        let switch_button = Arc::new(SwitchButton {
            theme: self.widget.theme,
            props: self.props,
            container,
            knob,
            state: Mutex::new(State {
                enabled,
            }),
        });

        let button_enabled = Arc::new(AtomicBool::new(false));

        for target in [&switch_button.container, &switch_button.knob] {
            let cb_switch_button = switch_button.clone();
            let cb_button_enabled = button_enabled.clone();

            target.on_press(MouseButton::Left, move |_, _, _| {
                let enabled = !cb_button_enabled.fetch_not(atomic::Ordering::SeqCst);
                cb_switch_button.state.lock().enabled = enabled;

                let widget_height = match cb_switch_button.props.height {
                    Some(height) => height,
                    None => cb_switch_button.theme.spacing * 2.0,
                };

                if enabled {
                    cb_switch_button
                        .container
                        .style_update(BinStyle {
                            back_color: Some(cb_switch_button.theme.colors.accent1),
                            ..cb_switch_button.container.style_copy()
                        })
                        .expect_valid();

                    cb_switch_button
                        .knob
                        .style_update(BinStyle {
                            pos_from_r: Some(widget_height * 0.1),
                            pos_from_l: None,
                            ..cb_switch_button.knob.style_copy()
                        })
                        .expect_valid();
                } else {
                    cb_switch_button
                        .container
                        .style_update(BinStyle {
                            back_color: Some(cb_switch_button.theme.colors.back3),
                            ..cb_switch_button.container.style_copy()
                        })
                        .expect_valid();

                    cb_switch_button
                        .knob
                        .style_update(BinStyle {
                            pos_from_l: Some(widget_height * 0.1),
                            pos_from_r: None,
                            ..cb_switch_button.knob.style_copy()
                        })
                        .expect_valid();
                }

                Default::default()
            });
        }

        switch_button.style_update();
        switch_button
    }
}

pub struct SwitchButton {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    knob: Arc<Bin>,
    state: Mutex<State>,
}

struct State {
    enabled: bool,
}

impl SwitchButton {
    fn style_update(&self) {
        let widget_height = match self.props.height {
            Some(height) => height,
            None => self.theme.spacing * 2.0,
        };

        let widget_width = match self.props.width {
            Some(width) => width.max(widget_height),
            None => widget_height * 2.0,
        };

        let enabled = self.state.lock().enabled;

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
