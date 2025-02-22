use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

use basalt::input::MouseButton;
use basalt::interface::{
    Bin, BinPosition, BinStyle, Color, TextHoriAlign, TextVertAlign, TextWrap,
};

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

pub struct ButtonBuilder {
    widget: WidgetBuilder,
    props: Properties,
}

#[derive(Default)]
struct Properties {
    text: String,
    width: Option<f32>,
    height: Option<f32>,
    text_height: Option<f32>,
}

impl ButtonBuilder {
    pub(crate) fn with_builder(builder: WidgetBuilder) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
        }
    }

    pub fn text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        self.props.text = text.into();
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

    pub fn text_height(mut self, text_height: f32) -> Self {
        self.props.text_height = Some(text_height);
        self
    }

    pub fn build(self) -> Arc<Button> {
        let window = self.widget.parent.window();
        let container = window.new_bin();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => (),
        }

        let button = Arc::new(Button {
            theme: self.widget.theme,
            props: self.props,
            container,
        });

        button_hooks(
            &button.container,
            [button.theme.colors.back3, button.theme.colors.text1a],
            [button.theme.colors.accent1, button.theme.colors.text1b],
            [button.theme.colors.accent2, button.theme.colors.text1b],
        );

        button.style_update();
        button
    }
}

pub struct Button {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
}

impl Button {
    fn style_update(&self) {
        let text_height = self.props.text_height.unwrap_or(self.theme.text_height);

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            back_color: Some(self.theme.colors.back3),
            text: self.props.text.clone(),
            text_height: Some(text_height),
            text_color: Some(self.theme.colors.text1a),
            text_hori_align: Some(TextHoriAlign::Center),
            text_vert_align: Some(TextVertAlign::Center),
            text_wrap: Some(TextWrap::None),
            font_family: Some(self.theme.font_family.clone()),
            font_weight: Some(self.theme.font_weight),
            ..Default::default()
        };

        match self.props.width {
            Some(width) => {
                container_style.width = Some(width);
            },
            None => {
                container_style.width = Some(0.0);
                container_style.hidden = Some(true);
                let cb_spacing = self.theme.spacing;

                self.container.on_update_once(move |container, _| {
                    container
                        .style_update(BinStyle {
                            width: Some((cb_spacing * 2.0) + container.calc_hori_overflow()),
                            hidden: None,
                            ..container.style_copy()
                        })
                        .expect_valid();
                });
            },
        }

        match self.props.height {
            Some(height) => {
                container_style.height = Some(height);
            },
            None => {
                container_style.height = Some((self.theme.spacing * 2.0) + self.theme.spacing);
            },
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
        }

        if let Some(border_radius) = self.theme.roundness {
            container_style.border_radius_tl = Some(border_radius);
            container_style.border_radius_tr = Some(border_radius);
            container_style.border_radius_bl = Some(border_radius);
            container_style.border_radius_br = Some(border_radius);
        }

        self.container.style_update(container_style).expect_valid();
    }
}

pub(crate) fn button_hooks(
    button: &Arc<Bin>,
    colors: [Color; 2],
    hover_colors: [Color; 2],
    press_colors: [Color; 2],
) {
    let cursor_inside = Arc::new(AtomicBool::new(false));
    let button_pressed = Arc::new(AtomicBool::new(false));

    let cb_cursor_inside = cursor_inside.clone();
    let cb_button_pressed = button_pressed.clone();

    button.on_enter(move |target, _| {
        cb_cursor_inside.store(true, atomic::Ordering::SeqCst);
        let button = target.into_bin().unwrap();

        if !cb_button_pressed.load(atomic::Ordering::SeqCst) {
            let mut style = button.style_copy();
            style.back_color = Some(hover_colors[0]);
            style.text_color = Some(hover_colors[1]);
            style
                .custom_verts
                .iter_mut()
                .for_each(|vert| vert.color = hover_colors[1]);
            button.style_update(style).expect_valid();
        }

        Default::default()
    });

    let cb_cursor_inside = cursor_inside.clone();
    let cb_button_pressed = button_pressed.clone();

    button.on_leave(move |target, _| {
        cb_cursor_inside.store(false, atomic::Ordering::SeqCst);
        let button = target.into_bin().unwrap();

        if !cb_button_pressed.load(atomic::Ordering::SeqCst) {
            let mut style = button.style_copy();
            style.back_color = Some(colors[0]);
            style.text_color = Some(colors[1]);
            style
                .custom_verts
                .iter_mut()
                .for_each(|vert| vert.color = colors[1]);
            button.style_update(style).expect_valid();
        }

        Default::default()
    });

    let cb_button_pressed = button_pressed.clone();

    button.on_press(MouseButton::Left, move |target, _, _| {
        cb_button_pressed.store(true, atomic::Ordering::SeqCst);
        let button = target.into_bin().unwrap();
        let mut style = button.style_copy();
        style.back_color = Some(press_colors[0]);
        style.text_color = Some(press_colors[1]);
        style
            .custom_verts
            .iter_mut()
            .for_each(|vert| vert.color = press_colors[1]);
        button.style_update(style).expect_valid();
        Default::default()
    });

    let cb_cursor_inside = cursor_inside;
    let cb_button_pressed = button_pressed;

    button.on_release(MouseButton::Left, move |target, _, _| {
        cb_button_pressed.store(false, atomic::Ordering::SeqCst);
        let button = target.into_bin().unwrap();
        let mut style = button.style_copy();

        if cb_cursor_inside.load(atomic::Ordering::SeqCst) {
            style.back_color = Some(hover_colors[0]);
            style.text_color = Some(hover_colors[1]);
            style
                .custom_verts
                .iter_mut()
                .for_each(|vert| vert.color = hover_colors[1]);
        } else {
            style.back_color = Some(colors[0]);
            style.text_color = Some(colors[1]);
            style
                .custom_verts
                .iter_mut()
                .for_each(|vert| vert.color = colors[1]);
        }

        button.style_update(style).expect_valid();
        Default::default()
    });
}
