use std::cell::RefCell;
use std::sync::Arc;

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle, BinVert, Color};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetContainer};

/// Builder for [`CheckBox`]
pub struct CheckBoxBuilder<'a, C, T> {
    widget: WidgetBuilder<'a, C>,
    props: Properties<T>,
    selected: bool,
    on_change: Vec<Box<dyn FnMut(&Arc<CheckBox<T>>, bool) + Send + 'static>>,
}

struct Properties<T> {
    value: T,
}

impl<'a, C, T> CheckBoxBuilder<'a, C, T>
where
    C: WidgetContainer,
    T: Send + Sync + 'static,
{
    pub(crate) fn with_builder(builder: WidgetBuilder<'a, C>, value: T) -> Self {
        Self {
            widget: builder,
            props: Properties {
                value,
            },
            selected: false,
            on_change: Vec::new(),
        }
    }

    /// Specify if the [`CheckBox`] should be selected after being built.
    ///
    /// **Note**: When this isn't used this defaults to `false`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add a callback to be called when the [`CheckBox`]'s state changed.
    ///
    /// **Note**: When changing the state within the callback, no callbacks will be called with
    /// the updated state.
    ///
    /// **Panics**: When adding a callback within the callback.
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<CheckBox<T>>, bool) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`CheckBox`].
    pub fn build(self) -> Arc<CheckBox<T>> {
        let window = self
            .widget
            .container
            .container_bin()
            .window()
            .expect("The widget container must have an associated window.");

        let mut new_bins = window.new_bins(2).into_iter();
        let container = new_bins.next().unwrap();
        let fill = new_bins.next().unwrap();

        self.widget
            .container
            .container_bin()
            .add_child(container.clone());

        container.add_child(fill.clone());

        let check_box = Arc::new(CheckBox {
            theme: self.widget.theme,
            props: self.props,
            container,
            fill,
            state: ReentrantMutex::new(State {
                selected: RefCell::new(false),
                on_change: RefCell::new(self.on_change),
            }),
        });

        let cb_check_box = check_box.clone();

        check_box
            .container
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_check_box.toggle_select();
                Default::default()
            });

        let cb_check_box = check_box.clone();

        check_box.fill.on_press(MouseButton::Left, move |_, _, _| {
            cb_check_box.toggle_select();
            Default::default()
        });

        check_box.style_update();
        check_box
    }
}

/// Check box widget
pub struct CheckBox<T> {
    theme: Theme,
    props: Properties<T>,
    container: Arc<Bin>,
    fill: Arc<Bin>,
    state: ReentrantMutex<State<T>>,
}

struct State<T> {
    selected: RefCell<bool>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<CheckBox<T>>, bool) + Send + 'static>>>,
}

impl<T> CheckBox<T> {
    /// Select this [`CheckBox`].
    pub fn select(self: &Arc<Self>) {
        self.set_selected(true);
    }

    /// Unselect this [`CheckBox`]
    pub fn unselect(self: &Arc<Self>) {
        self.set_selected(false);
    }

    /// Toggle the selection of this [`CheckBox`].
    ///
    /// Returns the new selection state.
    pub fn toggle_select(self: &Arc<Self>) -> bool {
        let state = self.state.lock();
        let selected = !*state.selected.borrow();
        self.set_selected(selected);
        selected
    }

    /// Check if the [`CheckBox`] is selected.
    pub fn is_selected(&self) -> bool {
        *self.state.lock().selected.borrow()
    }

    /// Obtain a reference the value.
    pub fn value_ref(&self) -> &T {
        &self.props.value
    }

    /// Add a callback to be called when the [`CheckBox`]'s selection changed.
    ///
    /// **Note**: When changing the state within the callback, no callbacks add to this
    /// [`CheckBox`] will be called with the updated state.
    ///
    /// **Panics**: When adding a callback within the callback to this [`CheckBox`].
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<CheckBox<T>>, bool) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    fn set_selected(self: &Arc<Self>, selected: bool) {
        let state = self.state.lock();

        if *state.selected.borrow() == selected {
            return;
        }

        *state.selected.borrow_mut() = selected;
        let mut fill_style = self.fill.style_copy();

        if selected {
            fill_style.hidden = None;
        } else {
            fill_style.hidden = Some(true);
        }

        self.fill.style_update(fill_style).expect_valid();

        if let Ok(mut on_change_cbs) = state.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(self, selected);
            }
        }
    }

    fn style_update(&self) {
        let width = self.theme.base_size; // TODO: Configurable
        let width_1_2 = width / 2.0;
        let check_space = (width_1_2 / 12.0).round().max(1.0);

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            width: Some(width),
            height: Some(width),
            back_color: Some(self.theme.colors.back2),
            border_radius_tl: Some(width_1_2),
            border_radius_tr: Some(width_1_2),
            border_radius_bl: Some(width_1_2),
            border_radius_br: Some(width_1_2),
            ..Default::default()
        };

        let mut fill_style = BinStyle {
            hidden: Some(true),
            position: Some(BinPosition::Parent),
            pos_from_t: Some(0.0),
            pos_from_b: Some(0.0),
            pos_from_l: Some(0.0),
            pos_from_r: Some(0.0),
            custom_verts: check_symbol_verts(width, check_space, self.theme.colors.accent1),
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
            let radius = roundness.min(width_1_2);
            container_style.border_radius_tl = Some(radius);
            container_style.border_radius_tr = Some(radius);
            container_style.border_radius_bl = Some(radius);
            container_style.border_radius_br = Some(radius);
        }

        if self.is_selected() {
            fill_style.hidden = None;
        }

        self.container.style_update(container_style).expect_valid();
        self.fill.style_update(fill_style).expect_valid();
    }
}

impl<T> CheckBox<T>
where
    T: Clone,
{
    /// Obtain a copy of the value.
    pub fn value(&self) -> T {
        self.props.value.clone()
    }
}

fn check_symbol_verts(target_size: f32, spacing: f32, color: Color) -> Vec<BinVert> {
    const UNIT_POS: [[f32; 2]; 6] = [
        [0.912, 0.131],
        [1.000, 0.218],
        [0.087, 0.432],
        [0.000, 0.519],
        [0.349, 0.694],
        [0.349, 0.868],
    ];

    let size = target_size - (spacing * 2.0);
    let mut verts = Vec::with_capacity(12);

    for i in [5, 1, 0, 5, 0, 4, 5, 4, 2, 5, 2, 3] {
        verts.push(BinVert {
            position: (
                (UNIT_POS[i][0] * size) + spacing,
                (UNIT_POS[i][1] * size) + spacing,
                0,
            ),
            color,
        });
    }

    verts
}
