use std::sync::Arc;

use basalt::interface::UnitValue::Pixels;
use basalt::interface::{Bin, BinPostUpdate, BinStyle, Position, TextAttrs, TextBody, TextSpan};

use crate::builder::WidgetBuilder;
use crate::{ScrollAxis, ScrollBar, Theme, WidgetContainer, WidgetPlacement, text_hooks, ulps_eq};

/// Builder for [`TextEditor`]
pub struct TextEditorBuilder<'a, C> {
    widget: WidgetBuilder<'a, C>,
    props: Properties,
    text_body: TextBody,
}

#[derive(Default)]
struct Properties {
    placement: WidgetPlacement,
}

impl Properties {
    fn new(placement: WidgetPlacement) -> Self {
        Self {
            placement,
        }
    }
}

impl<'a, C> TextEditorBuilder<'a, C>
where
    C: WidgetContainer,
{
    pub(crate) fn with_builder(mut builder: WidgetBuilder<'a, C>) -> Self {
        Self {
            props: Properties::new(
                builder
                    .placement
                    .take()
                    .unwrap_or_else(|| TextEditor::default_placement(&builder.theme)),
            ),
            widget: builder,
            text_body: Default::default(),
        }
    }

    /// Set the inital text.
    pub fn with_text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        if self.text_body.spans.is_empty() {
            self.text_body.spans.push(TextSpan::from(text.into()));
        } else {
            self.text_body.spans[0] = TextSpan::from(text.into());
        }

        self
    }

    /// Set the [`TextAttrs`] used.
    pub fn with_attrs(mut self, attrs: TextAttrs) -> Self {
        self.text_body.base_attrs = attrs;
        self
    }

    /// Finish building the [`TextEditor`].
    pub fn build(self) -> Arc<TextEditor> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut bins = window.new_bins(2).into_iter();
        let container = bins.next().unwrap();
        let editor = bins.next().unwrap();

        container.add_child(editor.clone());

        let sb_size = match ScrollBar::default_placement(&self.widget.theme, ScrollAxis::Y).width {
            Pixels(px) => px,
            _ => unreachable!(),
        };

        let border_size = self.widget.theme.border.unwrap_or(0.0);

        let v_scroll_b = container
            .create_widget()
            .with_theme(self.widget.theme.clone())
            .with_placement(WidgetPlacement {
                pos_from_b: Pixels(sb_size + border_size),
                ..ScrollBar::default_placement(&self.widget.theme, ScrollAxis::Y)
            })
            .scroll_bar(&editor)
            .build();

        let h_scroll_b = container
            .create_widget()
            .with_theme(self.widget.theme.clone())
            .with_placement(WidgetPlacement {
                pos_from_r: Pixels(sb_size + border_size),
                ..ScrollBar::default_placement(&self.widget.theme, ScrollAxis::X)
            })
            .scroll_bar(&editor)
            .axis(ScrollAxis::X)
            .build();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        let text_editor = Arc::new(TextEditor {
            theme: self.widget.theme,
            props: self.props,
            container,
            editor,
            v_scroll_b,
            h_scroll_b,
        });

        let text_editor_wk1 = Arc::downgrade(&text_editor);
        let text_editor_wk2 = Arc::downgrade(&text_editor);

        text_hooks::create(
            text_hooks::Properties::EDITOR,
            text_editor.editor.clone(),
            text_editor.theme.clone(),
            Some(Arc::new(move |editor_bpu, cursor_bounds| {
                if let Some(text_editor) = text_editor_wk1.upgrade() {
                    text_editor.check_cursor_in_view(editor_bpu, cursor_bounds);
                }
            })),
            Some(Arc::new(move |amt| {
                if let Some(text_editor) = text_editor_wk2.upgrade() {
                    text_editor.v_scroll_b.scroll(amt);
                }
            })),
        );

        text_editor.style_update(Some(self.text_body));
        text_editor
    }
}

/// TextEditor widget.
pub struct TextEditor {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    editor: Arc<Bin>,
    v_scroll_b: Arc<ScrollBar>,
    h_scroll_b: Arc<ScrollBar>,
}

impl TextEditor {
    /// Obtain the default [`WidgetPlacement`](`WidgetPlacement`) given a [`Theme`](`Theme`).
    pub fn default_placement(theme: &Theme) -> WidgetPlacement {
        let height = theme.spacing + (theme.base_size * 5.0);
        let width = height * 3.0;

        WidgetPlacement {
            position: Position::Floating,
            margin_t: Pixels(theme.spacing),
            margin_b: Pixels(theme.spacing),
            margin_l: Pixels(theme.spacing),
            margin_r: Pixels(theme.spacing),
            width: Pixels(width),
            height: Pixels(height),
            ..Default::default()
        }
    }

    fn check_cursor_in_view(&self, editor_bpu: &BinPostUpdate, mut cursor_bounds: [f32; 4]) {
        let editor_size = [
            editor_bpu.optimal_inner_bounds[1] - editor_bpu.optimal_inner_bounds[0],
            editor_bpu.optimal_inner_bounds[3] - editor_bpu.optimal_inner_bounds[2],
        ];

        let text_offset = [
            editor_bpu.optimal_inner_bounds[0] + editor_bpu.content_offset[0],
            editor_bpu.optimal_inner_bounds[2] + editor_bpu.content_offset[1],
        ];

        cursor_bounds[0] -= text_offset[0];
        cursor_bounds[1] -= text_offset[0];
        cursor_bounds[2] -= text_offset[1];
        cursor_bounds[3] -= text_offset[1];

        let target_scroll = [
            self.h_scroll_b.target_scroll(),
            self.v_scroll_b.target_scroll(),
        ];

        let editor_overflow = [
            self.h_scroll_b.target_overflow(),
            self.v_scroll_b.target_overflow(),
        ];

        let scroll_to_x_op = if cursor_bounds[0] - target_scroll[0] - self.theme.spacing < 0.0 {
            Some(cursor_bounds[0] - self.theme.spacing)
        } else if cursor_bounds[1] - target_scroll[0] + self.theme.spacing > editor_size[0] {
            Some(cursor_bounds[1] + self.theme.spacing - editor_size[0])
        } else {
            None
        };

        let scroll_to_y_op = if cursor_bounds[2] - target_scroll[1] - self.theme.spacing < 0.0 {
            Some(cursor_bounds[2] - self.theme.spacing)
        } else if cursor_bounds[3] - target_scroll[1] + self.theme.spacing > editor_size[1] {
            Some(cursor_bounds[3] + self.theme.spacing - editor_size[1])
        } else {
            None
        };

        if let Some(mut scroll_to_x) = scroll_to_x_op {
            scroll_to_x = scroll_to_x.clamp(0.0, editor_overflow[0]);

            if !ulps_eq(scroll_to_x, target_scroll[0], 8) {
                self.h_scroll_b.scroll_to(scroll_to_x);
            }
        }

        if let Some(mut scroll_to_y) = scroll_to_y_op {
            scroll_to_y = scroll_to_y.clamp(0.0, editor_overflow[1]);

            if !ulps_eq(scroll_to_y, target_scroll[1], 8) {
                self.v_scroll_b.scroll_to(scroll_to_y);
            }
        }
    }

    fn style_update(&self, text_body_op: Option<TextBody>) {
        let mut container_style = self.props.placement.clone().into_style();
        container_style.back_color = self.theme.colors.back2;
        let mut editor_style = BinStyle::default();

        if let Some(text_body) = text_body_op {
            editor_style.text_body = text_body;
        }

        editor_style.position = Position::Relative;
        editor_style.pos_from_t = Pixels(0.0);
        editor_style.pos_from_b = ScrollBar::default_placement(&self.theme, ScrollAxis::X).height;
        editor_style.pos_from_l = Pixels(0.0);
        editor_style.pos_from_r = ScrollBar::default_placement(&self.theme, ScrollAxis::Y).width;
        editor_style.text_body.base_attrs.height = Pixels(self.theme.text_height);
        editor_style.text_body.base_attrs.color = self.theme.colors.text1a;
        editor_style.text_body.base_attrs.font_family = self.theme.font_family.clone();
        editor_style.text_body.base_attrs.font_weight = self.theme.font_weight;
        editor_style.back_color = self.theme.colors.back2;
        editor_style.padding_t = Pixels(self.theme.spacing);
        editor_style.padding_b = Pixels(self.theme.spacing);
        editor_style.padding_l = Pixels(self.theme.spacing);
        editor_style.padding_r = Pixels(self.theme.spacing);

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
            container_style.border_radius_tl = Pixels(border_radius);
            container_style.border_radius_tr = Pixels(border_radius);
            container_style.border_radius_bl = Pixels(border_radius);
            container_style.border_radius_br = Pixels(border_radius);
            editor_style.border_radius_tl = Pixels(border_radius);
        }

        Bin::style_update_batch([
            (&self.container, container_style),
            (&self.editor, editor_style),
        ]);
    }
}
