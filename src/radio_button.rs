use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicU64};

use basalt::input::MouseButton;
use basalt::interface::{Bin, BinPosition, BinStyle};
use parking_lot::ReentrantMutex;

use crate::builder::WidgetBuilder;
use crate::{Theme, WidgetParent};

static GROUP_ID: AtomicU64 = AtomicU64::new(0);

/// An error that can occur from methods on [`RadioButtonGroup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadioButtonError {
    /// Requested an operation on a radio button that isn't in the group.
    NotInGroup,
    /// Attempted to add a button that was already in the group.
    AlreadyInGroup,
}

/// Builder for [`RadioButton`]
pub struct RadioButtonBuilder<T> {
    widget: WidgetBuilder,
    props: Properties<T>,
    selected: bool,
    group: Option<Arc<RadioButtonGroup<T>>>,
    on_change: Vec<Box<dyn FnMut(&Arc<RadioButton<T>>, bool) + Send + 'static>>,
}

struct Properties<T> {
    value: T,
}

impl<T> RadioButtonBuilder<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn with_builder(builder: WidgetBuilder, value: T) -> Self {
        Self {
            widget: builder,
            props: Properties {
                value,
            },
            selected: false,
            group: None,
            on_change: Vec::new(),
        }
    }

    /// Specify the [`RadioButtonGroup`] that the [`RadioButton`] is in.
    ///
    /// **Note**: A [`RadioButton`] can only be in one group. Calling this multiple times
    /// will result in the previous group being overwritten.
    pub fn group(mut self, group: &Arc<RadioButtonGroup<T>>) -> Self {
        self.group = Some(group.clone());
        self
    }

    /// Specify if the [`RadioButton`] should be selected after being built.
    ///
    /// **Note**: When this isn't used this defaults to `false`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add a callback to be called when the [`RadioButton`]'s selection changed.
    ///
    /// **Note**: When changing the state within the callback, no callbacks add to this
    /// [`RadioButton`] will be called with the updated state.
    ///
    /// **Panics**: When adding a callback within the callback to this [`RadioButton`].
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&Arc<RadioButton<T>>, bool) + Send + 'static,
    {
        self.on_change.push(Box::new(on_change));
        self
    }

    /// Finish building the [`RadioButton`].
    pub fn build(self) -> Arc<RadioButton<T>> {
        let window = self.widget.parent.window();
        let container = window.new_bin();

        match &self.widget.parent {
            WidgetParent::Bin(parent) => parent.add_child(container.clone()),
            _ => unimplemented!(),
        }

        let radio_button = Arc::new(RadioButton {
            theme: self.widget.theme,
            props: self.props,
            container,
            state: ReentrantMutex::new(State {
                id: RefCell::new(None),
                group: RefCell::new(None),
                selected: RefCell::new(false),
                on_change: RefCell::new(self.on_change),
            }),
        });

        match self.group {
            Some(group) => {
                if self.selected {
                    group.add_selected(&radio_button).unwrap();
                } else {
                    group.add(&radio_button).unwrap();
                }
            },
            None => {
                if self.selected {
                    radio_button.select();
                }
            },
        }

        let cb_radio_button = radio_button.clone();

        radio_button
            .container
            .on_press(MouseButton::Left, move |_, _, _| {
                cb_radio_button.select();
                Default::default()
            });

        radio_button.style_update();
        radio_button
    }
}

/// Radio button widget
pub struct RadioButton<T> {
    theme: Theme,
    props: Properties<T>,
    container: Arc<Bin>,
    state: ReentrantMutex<State<T>>,
}

struct State<T> {
    id: RefCell<Option<u64>>,
    group: RefCell<Option<Arc<RadioButtonGroup<T>>>>,
    selected: RefCell<bool>,
    on_change: RefCell<Vec<Box<dyn FnMut(&Arc<RadioButton<T>>, bool) + Send + 'static>>>,
}

impl<T> RadioButton<T> {
    /// Select this [`RadioButton`].
    pub fn select(self: &Arc<Self>) {
        let state = self.state.lock();

        match state.group.borrow().as_ref().cloned() {
            Some(group) => {
                group.select(self).unwrap();
            },
            None => {
                self.set_selected(true);
            },
        }
    }

    /// Unselect this [`RadioButton`]
    pub fn unselect(self: &Arc<Self>) {
        let state = self.state.lock();

        if *state.selected.borrow() {
            match state.group.borrow().as_ref().cloned() {
                Some(group) => {
                    group.clear_selection();
                },
                None => {
                    self.set_selected(false);
                },
            }
        }
    }

    /// Obtain a reference the value.
    pub fn value_ref(&self) -> &T {
        &self.props.value
    }

    /// Add a callback to be called when the [`RadioButton`]'s selection changed.
    ///
    /// **Note**: When changing the state within the callback, no callbacks add to this
    /// [`RadioButton`] will be called with the updated state.
    ///
    /// **Panics**: When adding a callback within the callback to this [`RadioButton`].
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(&Arc<RadioButton<T>>, bool) + Send + 'static,
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
        let mut container_style = self.container.style_copy();

        // TODO: Better Display

        if selected {
            container_style.back_color = Some(self.theme.colors.accent1);
        } else {
            container_style.back_color = Some(self.theme.colors.back2);
        }

        self.container.style_update(container_style).expect_valid();

        if let Ok(mut on_change_cbs) = state.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(self, selected);
            }
        }
    }

    fn style_update(&self) {
        let width = self.theme.spacing; // TODO: Configurable
        let width_1_2 = width / 2.0;

        let mut container_style = BinStyle {
            position: Some(BinPosition::Floating),
            margin_t: Some(self.theme.spacing),
            margin_b: Some(self.theme.spacing),
            margin_l: Some(self.theme.spacing),
            margin_r: Some(self.theme.spacing),
            back_color: Some(self.theme.colors.back2),
            width: Some(width),
            height: Some(width),
            border_radius_tl: Some(width_1_2),
            border_radius_tr: Some(width_1_2),
            border_radius_bl: Some(width_1_2),
            border_radius_br: Some(width_1_2),
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

        self.container.style_update(container_style).expect_valid();
    }
}

impl<T> RadioButton<T>
where
    T: Clone,
{
    /// Obtain a copy of the value.
    pub fn value(&self) -> T {
        self.props.value.clone()
    }
}

struct GroupState<T> {
    buttons: RefCell<BTreeMap<u64, Arc<RadioButton<T>>>>,
    selection: RefCell<Option<u64>>,
    next_id: RefCell<u64>,
    on_change: RefCell<Vec<Box<dyn FnMut(Option<&Arc<RadioButton<T>>>) + Send + 'static>>>,
}

impl<T> GroupState<T> {
    fn call_on_change(&self, button_op: Option<&Arc<RadioButton<T>>>) {
        if let Ok(mut on_change_cbs) = self.on_change.try_borrow_mut() {
            for on_change in on_change_cbs.iter_mut() {
                on_change(button_op);
            }
        }
    }
}

/// Group of [`RadioButton`]'s
///
/// **Note**: This does not provide any styling, but exists purely for logic.
pub struct RadioButtonGroup<T> {
    id: u64,
    state: ReentrantMutex<GroupState<T>>,
}

impl<T> RadioButtonGroup<T> {
    /// Create a new [`RadioButtonGroup`].
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            id: GROUP_ID.fetch_add(1, atomic::Ordering::SeqCst),
            state: ReentrantMutex::new(GroupState {
                buttons: RefCell::new(BTreeMap::new()),
                selection: RefCell::new(None),
                next_id: RefCell::new(0),
                on_change: RefCell::new(Vec::new()),
            }),
        })
    }

    /// Clear any existing selection of [`RadioButton`].
    pub fn clear_selection(&self) {
        let state = self.state.lock();

        if let Some(button_id) = state.selection.borrow_mut().take() {
            match state.buttons.borrow().get(&button_id) {
                Some(button) => {
                    button.set_selected(false);
                },
                None => unreachable!(),
            }

            state.call_on_change(None);
        }
    }

    /// Select a specific [`RadioButton`].
    pub fn select(&self, radio_button: &Arc<RadioButton<T>>) -> Result<(), RadioButtonError> {
        let state = self.state.lock();
        let b_state = radio_button.state.lock();

        if b_state.group.borrow().is_none()
            || b_state.group.borrow().as_ref().unwrap().id != self.id
        {
            return Err(RadioButtonError::NotInGroup);
        }

        if let Some(old_button_id) = state.selection.borrow_mut().take() {
            let old_button = state.buttons.borrow().get(&old_button_id).cloned().unwrap();
            old_button.set_selected(false);
        }

        *state.selection.borrow_mut() = Some(b_state.id.borrow().unwrap());
        radio_button.set_selected(true);
        state.call_on_change(Some(radio_button));
        Ok(())
    }

    /// Add a callback to be called when a [`RadioButton`] is selected.
    ///
    /// **Note**: When changing the state within the callback, no callbacks add to this
    /// [`RadioButtonGroup`] will be called with the updated state. Callbacks added specify to
    /// [`RadioButton`] will still be called.
    ///
    /// **Panics**: When adding a callback within the callback to this [`RadioButtonGroup`].
    pub fn on_change<F>(&self, on_change: F)
    where
        F: FnMut(Option<&Arc<RadioButton<T>>>) + Send + 'static,
    {
        self.state
            .lock()
            .on_change
            .borrow_mut()
            .push(Box::new(on_change));
    }

    /// Add a [`RadioButton`] to this [`RadioButtonGroup`].
    pub fn add(
        self: &Arc<Self>,
        radio_button: &Arc<RadioButton<T>>,
    ) -> Result<(), RadioButtonError> {
        let state = self.state.lock();
        let b_state = radio_button.state.lock();

        if let Some(old_group) = b_state.group.borrow().as_ref().cloned() {
            if old_group.id == self.id {
                return Err(RadioButtonError::AlreadyInGroup);
            }

            old_group.remove(radio_button).unwrap();
        }

        let id = {
            let mut next_id = state.next_id.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        *b_state.group.borrow_mut() = Some(self.clone());
        *b_state.id.borrow_mut() = Some(id);
        state.buttons.borrow_mut().insert(id, radio_button.clone());
        Ok(())
    }

    /// Same as [`RadioButtonGroup::add`], but selects the added [`RadioButton`] after its addition.
    pub fn add_selected(
        self: &Arc<Self>,
        radio_button: &Arc<RadioButton<T>>,
    ) -> Result<(), RadioButtonError> {
        let _state = self.state.lock();
        let _b_state = radio_button.state.lock();
        self.add(radio_button)?;
        self.select(radio_button).unwrap();
        Ok(())
    }

    /// Remove a [`RadioButton`] from this group.
    pub fn remove(&self, radio_button: &Arc<RadioButton<T>>) -> Result<(), RadioButtonError> {
        let state = self.state.lock();
        let b_state = radio_button.state.lock();

        if b_state.group.borrow().is_none()
            || b_state.group.borrow().as_ref().unwrap().id != self.id
        {
            return Err(RadioButtonError::NotInGroup);
        }

        state
            .buttons
            .borrow_mut()
            .remove(&b_state.id.borrow().unwrap());

        let mut selection_changed = false;

        if state.selection.borrow().is_some()
            && state.selection.borrow().unwrap() == b_state.id.borrow().unwrap()
        {
            *state.selection.borrow_mut() = None;
            selection_changed = true;
        }

        *b_state.group.borrow_mut() = None;
        *b_state.id.borrow_mut() = None;

        if selection_changed {
            state.call_on_change(None);
        }

        Ok(())
    }
}

impl<T> RadioButtonGroup<T>
where
    T: PartialEq,
{
    /// Attempt to select a [`RadioButton`] given a value.
    ///
    /// **Note**: This is a no-op if the group doesn't contain a [`RadioButton`] with the value.
    pub fn select_by_value(&self, value: &T) {
        let state = self.state.lock();

        let button_op = state
            .buttons
            .borrow()
            .values()
            .find(|b| b.props.value == *value)
            .cloned();

        if let Some(button) = button_op {
            self.select(&button).unwrap();
        }
    }

    /// Attempt to remove a [`RadioButton`] given a value.
    ///
    /// **Returns**: `true` if a [`RadioButton`] was removed.
    pub fn remove_by_value(&self, value: &T) -> bool {
        let state = self.state.lock();

        let button_op = state
            .buttons
            .borrow()
            .values()
            .find(|b| b.props.value == *value)
            .cloned();

        if let Some(button) = button_op {
            self.remove(&button).unwrap();
            return true;
        }

        false
    }
}
