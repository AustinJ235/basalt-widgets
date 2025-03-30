use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle, TextHoriAlign, TextVertAlign, TextWrap};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::scroll_bar::down_symbol_verts;
use crate::{ScrollBar, Theme, WidgetContainer};

pub struct SelectBuilder<'a, C, I> {
    widget: WidgetBuilder<'a, C>,
    props: Properties,
    select: Option<I>,
    options: BTreeMap<I, String>,
    on_select: Vec<Box<dyn FnMut(&Arc<Select<I>>, I) + Send + 'static>>,
}

struct Properties {
    drop_down_items: usize,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            drop_down_items: 3,
        }
    }
}

impl<'a, C, I> SelectBuilder<'a, C, I>
where
    C: WidgetContainer,
    I: Ord + Copy + Send + 'static,
{
    pub(crate) fn with_builder(builder: WidgetBuilder<'a, C>) -> Self {
        Self {
            widget: builder,
            props: Default::default(),
            select: None,
            options: BTreeMap::new(),
            on_select: Vec::new(),
        }
    }

    pub fn add_option<L>(mut self, option_id: I, label: L) -> Self
    where
        L: Into<String>,
    {
        self.options.insert(option_id, label.into());
        self
    }

    pub fn select(mut self, option_id: I) -> Self {
        self.select = Some(option_id);
        self
    }

    pub fn drop_down_items(mut self, count: usize) -> Self {
        self.props.drop_down_items = count;
        self
    }

    pub fn on_select<F>(mut self, on_select: F) -> Self
    where
        F: FnMut(&Arc<Select<I>>, I) + Send + 'static,
    {
        self.on_select.push(Box::new(on_select));
        self
    }

    pub fn build(self) -> Arc<Select<I>> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut new_bins = window.new_bins(4 + self.options.len()).into_iter();
        let container = new_bins.next().unwrap();
        let popup = new_bins.next().unwrap();
        let arrow_down = new_bins.next().unwrap();
        let option_list = new_bins.next().unwrap();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        container.add_child(arrow_down.clone());
        container.add_child(popup.clone());
        popup.add_child(option_list.clone());

        let scroll_bar = popup
            .create_widget()
            .with_theme(self.widget.theme.clone())
            .scroll_bar(option_list.clone())
            .step(
                self.widget.theme.spacing
                    + self.widget.theme.base_size
                    + self.widget.theme.border.unwrap_or(0.0),
            )
            .build();

        let options_state = RefCell::new(BTreeMap::from_iter(self.options.into_iter().map(
            |(id, label)| {
                let bin = new_bins.next().unwrap();
                option_list.add_child(bin.clone());
                (
                    id,
                    OptionState {
                        label,
                        bin,
                    },
                )
            },
        )));

        let select = Arc::new(Select {
            theme: self.widget.theme,
            props: self.props,
            container,
            popup,
            arrow_down,
            scroll_bar,
            option_list,
            state: ReentrantMutex::new(State {
                select: RefCell::new(self.select),
                options: options_state,
                on_select: RefCell::new(self.on_select),
            }),
        });

        let cb_select = select.clone();

        select
            .container
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_select.toggle_popup();
                Default::default()
            });

        let cb_select = select.clone();

        select
            .arrow_down
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_select.toggle_popup();
                Default::default()
            });

        let cb_select = select.clone();
        let mut currently_focused = false;

        select
            .container
            .attach_input_hook(window.on_bin_focus_change(move |_, w_state, _| {
                let now_focused = match w_state.focused_bin_id() {
                    Some(bin_id) => {
                        bin_id == cb_select.container.id()
                            || bin_id == cb_select.popup.id()
                            || bin_id == cb_select.arrow_down.id()
                            || bin_id == cb_select.option_list.id()
                            || cb_select.scroll_bar.has_bin_id(bin_id)
                    },
                    None => false,
                };

                if currently_focused {
                    if !now_focused {
                        currently_focused = false;
                        cb_select.hide_popup();
                    }
                } else {
                    currently_focused = now_focused;
                }

                Default::default()
            }));

        select
            .state
            .lock()
            .options
            .borrow()
            .iter()
            .for_each(|(id, option_state)| {
                select.add_option_select_hook(*id, &option_state.bin);
            });

        select.style_update();
        select.rebuild_list();
        select
    }
}

pub struct Select<I> {
    theme: Theme,
    props: Properties,
    container: Arc<Bin>,
    popup: Arc<Bin>,
    arrow_down: Arc<Bin>,
    scroll_bar: Arc<ScrollBar>,
    option_list: Arc<Bin>,
    state: ReentrantMutex<State<I>>,
}

struct State<I> {
    select: RefCell<Option<I>>,
    options: RefCell<BTreeMap<I, OptionState>>,
    on_select: RefCell<Vec<Box<dyn FnMut(&Arc<Select<I>>, I) + Send + 'static>>>,
}

struct OptionState {
    label: String,
    bin: Arc<Bin>,
}

impl<I> Select<I>
where
    I: Ord + Copy + Send + 'static,
{
    pub fn on_select<F>(&self, on_select: F)
    where
        F: FnMut(&Arc<Select<I>>, I) + Send + 'static,
    {
        self.state
            .lock()
            .on_select
            .borrow_mut()
            .push(Box::new(on_select));
    }

    pub fn select(&self, option_id: I) {
        let state = self.state.lock();

        let label = {
            let mut select = state.select.borrow_mut();
            let options = state.options.borrow();

            if let Some(cur_sel_id) = *select {
                if cur_sel_id == option_id {
                    return;
                }
            }

            *select = Some(option_id);

            match options.get(&option_id) {
                Some(option_state) => option_state.label.clone(),
                None => String::new(),
            }
        };

        self.container
            .style_update(BinStyle {
                text: label,
                ..self.container.style_copy()
            })
            .expect_valid();

        self.rebuild_list();
    }

    fn add_option_select_hook(self: &Arc<Self>, id: I, bin: &Arc<Bin>) {
        let cb_select = self.clone();

        bin.on_press(MouseButton::Left, move |_, _, _| {
            cb_select.select(id);
            Default::default()
        });
    }

    fn toggle_popup(&self) {
        if self
            .popup
            .style_inspect(|popup_style| popup_style.hidden.is_some())
        {
            self.display_popup();
        } else {
            self.hide_popup();
        }
    }

    fn hide_popup(&self) {
        let mut style_update_batch = Vec::new();
        let mut popup_style = self.popup.style_copy();
        popup_style.hidden = Some(true);
        style_update_batch.push((&self.popup, popup_style));

        if let Some(border_radius) = self.theme.roundness {
            let mut container_style = self.container.style_copy();
            container_style.border_radius_bl = Some(border_radius);
            container_style.border_radius_br = Some(border_radius);
            style_update_batch.push((&self.container, container_style));
        }

        Bin::style_update_batch(style_update_batch);
    }

    fn display_popup(&self) {
        let mut style_update_batch = Vec::new();
        let mut popup_style = self.popup.style_copy();
        popup_style.hidden = None;
        style_update_batch.push((&self.popup, popup_style));

        if self.theme.roundness.is_some() {
            let mut container_style = self.container.style_copy();
            container_style.border_radius_bl = None;
            container_style.border_radius_br = None;
            style_update_batch.push((&self.container, container_style));
        }

        let index = {
            let state = self.state.lock();
            let select = state.select.borrow();
            let options = state.options.borrow();

            match *select {
                Some(sel_id) => {
                    match options.keys().enumerate().find(|(_, id)| **id == sel_id) {
                        Some(some) => some.0,
                        None => 0,
                    }
                },
                None => 0,
            }
        };

        self.popup_jump_to_index(index);
        Bin::style_update_batch(style_update_batch);
    }

    fn popup_jump_to_index(&self, index: usize) {
        let jump_index = index
            .checked_sub(self.props.drop_down_items / 3)
            .unwrap_or(0);

        let jump_to = jump_index as f32
            * (self.theme.base_size + self.theme.spacing + self.theme.border.unwrap_or(0.0));

        self.scroll_bar.jump_to(jump_to);
    }

    fn rebuild_list(&self) {
        let state = self.state.lock();
        let options = state.options.borrow();
        let select = state.select.borrow();
        let num_options = options.len();

        if !options.is_empty() {
            let mut styles = Vec::with_capacity(num_options);

            for (i, (id, option_state)) in options.iter().enumerate() {
                let mut option_style = BinStyle {
                    position: Some(BinPosition::Parent),
                    pos_from_t: Some(
                        i as f32
                            * (self.theme.spacing
                                + self.theme.base_size
                                + self.theme.border.unwrap_or(0.0)),
                    ),
                    pos_from_l: Some(0.0),
                    pos_from_r: Some(0.0),
                    text: option_state.label.clone(),
                    height: Some(self.theme.spacing + self.theme.base_size),
                    pad_l: Some(self.theme.spacing),
                    pad_r: Some(self.theme.spacing),
                    text_height: Some(self.theme.text_height),
                    text_color: Some(self.theme.colors.text1a),
                    text_hori_align: Some(TextHoriAlign::Left),
                    text_vert_align: Some(TextVertAlign::Center),
                    text_wrap: Some(TextWrap::None),
                    font_family: Some(self.theme.font_family.clone()),
                    font_weight: Some(self.theme.font_weight),
                    ..Default::default()
                };

                if i != num_options - 1 {
                    if let Some(border_size) = self.theme.border {
                        option_style.border_size_b = Some(border_size);
                        option_style.border_color_b = Some(self.theme.colors.border2);
                    }
                }

                if let Some(select_id) = *select {
                    if select_id == *id {
                        option_style.back_color = Some(self.theme.colors.accent1);
                        option_style.text_color = Some(self.theme.colors.text1b);
                    }
                }

                styles.push(option_style);
            }

            Bin::style_update_batch(
                options
                    .values()
                    .map(|option_state| &option_state.bin)
                    .zip(styles),
            );
        }
    }

    fn style_update(&self) {
        let widget_height = self.theme.spacing + self.theme.base_size;
        let widget_width = widget_height * 5.0;
        let border_size = self.theme.border.unwrap_or(0.0);

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            width: Some(widget_width),
            height: Some(widget_height),
            pad_l: Some(self.theme.spacing),
            pad_r: Some(widget_height),
            back_color: Some(self.theme.colors.back3),
            text_height: Some(self.theme.text_height),
            text_color: Some(self.theme.colors.text1a),
            text_hori_align: Some(TextHoriAlign::Left),
            text_vert_align: Some(TextVertAlign::Center),
            text_wrap: Some(TextWrap::None),
            font_family: Some(self.theme.font_family.clone()),
            font_weight: Some(self.theme.font_weight),
            overflow_y: Some(true),
            overflow_x: Some(true),
            ..Default::default()
        };

        let arrow_down_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_r: Some(0.0),
            width: Some(widget_height),
            custom_verts: down_symbol_verts(
                widget_height,
                self.theme.spacing,
                self.theme.colors.text1a,
            ),
            ..Default::default()
        };

        let mut popup_style = BinStyle {
            hidden: Some(true),
            position: Some(BinPosition::Parent),
            pos_from_t_pct: Some(100.0),
            pos_from_t_offset: Some(border_size),
            pos_from_l: Some(0.0),
            pos_from_r: Some(0.0),
            height: Some(
                (widget_height * self.props.drop_down_items as f32)
                    + (border_size
                        * (self.props.drop_down_items.checked_sub(1).unwrap_or(0) as f32)),
            ),
            back_color: Some(self.theme.colors.back2),
            add_z_index: Some(100),
            ..Default::default()
        };

        let option_list_style = BinStyle {
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_l: Some(0.0),
            pos_from_r: Some(ScrollBar::size(&self.theme)),
            pos_from_b: Some(0.0),
            ..Default::default()
        };

        {
            let state = self.state.lock();

            if let Some(option_id) = &*state.select.borrow() {
                if let Some(option_state) = state.options.borrow().get(option_id) {
                    container_style.text = option_state.label.clone();
                }
            }
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

            popup_style.border_size_b = Some(border_size);
            popup_style.border_size_l = Some(border_size);
            popup_style.border_size_r = Some(border_size);
            popup_style.border_color_b = Some(self.theme.colors.border1);
            popup_style.border_color_l = Some(self.theme.colors.border1);
            popup_style.border_color_r = Some(self.theme.colors.border1);
        }

        if let Some(border_radius) = self.theme.roundness {
            container_style.border_radius_tl = Some(border_radius);
            container_style.border_radius_tr = Some(border_radius);
            container_style.border_radius_bl = Some(border_radius);
            container_style.border_radius_br = Some(border_radius);

            popup_style.border_radius_bl = Some(border_radius);
            popup_style.border_radius_br = Some(border_radius);
        }

        Bin::style_update_batch([
            (&self.container, container_style),
            (&self.arrow_down, arrow_down_style),
            (&self.popup, popup_style),
            (&self.option_list, option_list_style),
        ]);
    }
}
