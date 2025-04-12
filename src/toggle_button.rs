use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

use basalt::input::MouseButton;
use basalt::interface::UnitValue::Pixels;
use basalt::interface::{
    Bin, BinStyle, Position, TextAttrs, TextBody, TextHoriAlign, TextVertAlign, TextWrap,
    Visibility,
};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetContainer};

/// Builder for [`ToggleButton`]
pub struct ToggleButtonBuilder<'a, C> {
    widget: WidgetBuilder<'a, C>,
    props: Properties,
    on_change: Vec<Box<dyn FnMut(&Arc<ToggleButton>, bool) + Send + 'static>>,
}

#[derive(Default)]
struct Properties {
    disabled_text: String,
    enabled_text: String,
    enabled: bool,
    width: Option<f32>,
    height: Option<f32>,
    text_height: Option<f32>,
}

impl<'a, C> ToggleButtonBuilder<'a, C>
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

    /// Set the text to be displayed when disabled.
    ///
    /// **Note**: When this isn't used the disabled text will be empty.
    pub fn disabled_text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        self.props.disabled_text = text.into();
        self
    }

    /// Set the text to be displayed when enabled.
    ///
    /// **Note**: When this isn't used the enabled text will be empty.
    pub fn enabled_text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        self.props.enabled_text = text.into();
        self
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

    /// **Temporary**
    pub fn text_height(mut self, text_height: f32) -> Self {
        self.props.text_height = Some(text_height);
        self
    }

    /// Add a callback to be called when the [`ToggleButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<ToggleButton>, bool) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`ToggleButton`].
    pub fn build(self) -> Arc<ToggleButton> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let container = window.new_bin();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        let enabled = self.props.enabled;

        let toggle_button = Arc::new(ToggleButton {
            theme: self.widget.theme,
            props: self.props,
            container,
            state: ReentrantMutex::new(State {
                enabled: RefCell::new(enabled),
                on_change: RefCell::new(self.on_change),
            }),
        });

        let cursor_inside = Arc::new(AtomicBool::new(false));
        let button_pressed = Arc::new(AtomicBool::new(false));

        let cb_toggle_button = toggle_button.clone();
        let cb_cursor_inside = cursor_inside.clone();
        let cb_button_pressed = button_pressed.clone();

        toggle_button.container.on_enter(move |_, _| {
            cb_cursor_inside.store(true, atomic::Ordering::SeqCst);

            if !cb_button_pressed.load(atomic::Ordering::SeqCst) && !cb_toggle_button.get() {
                let mut style = cb_toggle_button.container.style_copy();
                style.back_color = cb_toggle_button.theme.colors.accent1;
                style.text_body.base_attrs.color = cb_toggle_button.theme.colors.text1b;

                cb_toggle_button
                    .container
                    .style_update(style)
                    .expect_valid();
            }

            Default::default()
        });

        let cb_toggle_button = toggle_button.clone();
        let cb_cursor_inside = cursor_inside.clone();
        let cb_button_pressed = button_pressed.clone();

        toggle_button.container.on_leave(move |_, _| {
            cb_cursor_inside.store(false, atomic::Ordering::SeqCst);

            if !cb_button_pressed.load(atomic::Ordering::SeqCst) && !cb_toggle_button.get() {
                let mut style = cb_toggle_button.container.style_copy();
                style.back_color = cb_toggle_button.theme.colors.back3;
                style.text_body.base_attrs.color = cb_toggle_button.theme.colors.text1a;

                cb_toggle_button
                    .container
                    .style_update(style)
                    .expect_valid();
            }

            Default::default()
        });

        let cb_toggle_button = toggle_button.clone();
        let cb_button_pressed = button_pressed.clone();

        toggle_button
            .container
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_button_pressed.store(true, atomic::Ordering::SeqCst);
                cb_toggle_button.toggle();
                Default::default()
            });

        let cb_toggle_button = toggle_button.clone();
        let cb_cursor_inside = cursor_inside;
        let cb_button_pressed = button_pressed;

        toggle_button
            .container
            .on_release(MouseButton::Left, move |_, _, _| {
                cb_button_pressed.store(false, atomic::Ordering::SeqCst);

                if !cb_toggle_button.get() {
                    let mut style = cb_toggle_button.container.style_copy();

                    if cb_cursor_inside.load(atomic::Ordering::SeqCst) {
                        style.back_color = cb_toggle_button.theme.colors.accent1;
                        style.text_body.base_attrs.color = cb_toggle_button.theme.colors.text1b;
                    } else {
                        style.back_color = cb_toggle_button.theme.colors.back3;
                        style.text_body.base_attrs.color = cb_toggle_button.theme.colors.text1a;
                    }

                    cb_toggle_button
                        .container
                        .style_update(style)
                        .expect_valid();
                }

                Default::default()
            });

        toggle_button.style_update();
        toggle_button
    }
}

/// Toggle button widget
pub struct ToggleButton {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    state: ReentrantMutex<State>,
}

struct State {
    enabled: RefCell<bool>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<ToggleButton>, bool) + Send + 'static>>>,
}

impl ToggleButton {
    /// Set the enabled state.
    pub fn set(self: &Arc<Self>, enabled: bool) {
        let state = self.state.lock();
        *state.enabled.borrow_mut() = enabled;

        let mut style = self.container.style_copy();
        style.back_color = self.theme.colors.accent2;
        style.text_body.base_attrs.color = self.theme.colors.text1b;

        style.text_body.spans[0].text = if enabled {
            self.props.enabled_text.clone()
        } else {
            self.props.disabled_text.clone()
        };

        self.container.style_update(style).expect_valid();

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

    /// Add a callback to be called when the [`ToggleButton`]'s value changed.
    ///
    /// **Note**: When changing the value within the callback, no callbacks will be called with
    ///  the updated value.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<ToggleButton>, bool) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    fn style_update(&self) {
        let text_height = self.props.text_height.unwrap_or(self.theme.text_height);

        let mut container_style = BinStyle {
            position: Position::Floating,
            margin_t: Pixels(self.theme.spacing),
            margin_b: Pixels(self.theme.spacing),
            margin_l: Pixels(self.theme.spacing),
            margin_r: Pixels(self.theme.spacing),
            text_body: TextBody {
                spans: vec![Default::default()],
                hori_align: TextHoriAlign::Center,
                vert_align: TextVertAlign::Center,
                text_wrap: TextWrap::None,
                base_attrs: TextAttrs {
                    height: Pixels(text_height),
                    font_family: self.theme.font_family.clone(),
                    font_weight: self.theme.font_weight,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        if *self.state.lock().enabled.borrow() {
            container_style.back_color = self.theme.colors.accent2;
            container_style.text_body.base_attrs.color = self.theme.colors.text1b;
        } else {
            container_style.back_color = self.theme.colors.back3;
            container_style.text_body.base_attrs.color = self.theme.colors.text1a;
        }

        let initial_text = if self.props.enabled {
            self.props.enabled_text.clone()
        } else {
            self.props.disabled_text.clone()
        };

        match self.props.width {
            Some(width) => {
                container_style.width = Pixels(width);
                container_style.text_body.spans[0].text = initial_text;
            },
            None => {
                container_style.text_body.spans[0].text = (0..self
                    .props
                    .disabled_text
                    .len()
                    .max(self.props.enabled_text.len()))
                    .map(|_| 'X')
                    .collect();

                container_style.width = Pixels(0.0);
                container_style.visibility = Visibility::Hide;
                let cb_spacing = self.theme.spacing;

                self.container.on_update_once(move |container, _| {
                    let width = Pixels((cb_spacing * 2.0) + container.calc_hori_overflow());

                    container.style_modify(|style| {
                        style.visibility = Visibility::Inheirt;
                        style.width = width;
                        style.text_body.spans[0].text = initial_text.clone();
                    });
                });
            },
        }

        match self.props.height {
            Some(height) => {
                container_style.height = Pixels(height);
            },
            None => {
                container_style.height = Pixels((self.theme.spacing * 2.0) + self.theme.spacing);
            },
        }

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
            container_style.border_radius_tl = border_radius;
            container_style.border_radius_tr = border_radius;
            container_style.border_radius_bl = border_radius;
            container_style.border_radius_br = border_radius;
        }

        self.container.style_update(container_style).expect_valid();
    }
}
