//! Builder types

pub use crate::button::ButtonBuilder;
pub use crate::check_box::CheckBoxBuilder;
pub use crate::progress_bar::ProgressBarBuilder;
pub use crate::radio_button::RadioButtonBuilder;
pub use crate::scaler::ScalerBuilder;
pub use crate::scroll_bar::ScrollBarBuilder;
pub use crate::select::SelectBuilder;
pub use crate::spin_button::SpinButtonBuilder;
pub use crate::switch_button::SwitchButtonBuilder;
pub use crate::toggle_button::ToggleButtonBuilder;
use crate::{Theme, WidgetContainer, WidgetPlacement};

/// General builder for widgets.
pub struct WidgetBuilder<'a, C> {
    pub(crate) container: &'a C,
    pub(crate) theme: Theme,
    pub(crate) placement: Option<WidgetPlacement>,
}

impl<'a, C> From<&'a C> for WidgetBuilder<'a, C>
where
    C: WidgetContainer,
{
    fn from(container: &'a C) -> Self {
        Self {
            theme: container.default_theme(),
            container,
            placement: None,
        }
    }
}

impl<'a, C> WidgetBuilder<'a, C>
where
    C: WidgetContainer,
{
    /// Specify a theme to be used.
    ///
    /// **Note**: When not used the theme will be Basalt's default light theme.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Specify how the widget should be placed.
    pub fn with_placement(mut self, placement: WidgetPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    /// Transition into building a [`Button`](crate::Button)
    pub fn button(self) -> ButtonBuilder<'a, C> {
        ButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`SpinButton`](crate::SpinButton)
    pub fn spin_button(self) -> SpinButtonBuilder<'a, C> {
        SpinButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`ToggleButton`](crate::ToggleButton)
    pub fn toggle_button(self) -> ToggleButtonBuilder<'a, C> {
        ToggleButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`SwitchButton`](crate::SwitchButton)
    pub fn switch_button(self) -> SwitchButtonBuilder<'a, C> {
        SwitchButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`Scaler`](crate::Scaler)
    pub fn scaler(self) -> ScalerBuilder<'a, C> {
        ScalerBuilder::with_builder(self)
    }

    /// Transition into building a [`ProgressBar`](crate::ProgressBar)
    pub fn progress_bar(self) -> ProgressBarBuilder<'a, C> {
        ProgressBarBuilder::with_builder(self)
    }

    /// Transition into building a [`RadioButton`](crate::RadioButton)
    pub fn radio_button<T>(self, value: T) -> RadioButtonBuilder<'a, C, T>
    where
        T: Send + Sync + 'static,
    {
        RadioButtonBuilder::with_builder(self, value)
    }

    /// Transition into building a [`CheckBox`](crate::CheckBox)
    pub fn check_box<T>(self, value: T) -> CheckBoxBuilder<'a, C, T>
    where
        T: Send + Sync + 'static,
    {
        CheckBoxBuilder::with_builder(self, value)
    }

    /// Transition into building a [`ScrollBar`](crate::ScrollBar)
    pub fn scroll_bar<T>(self, target: T) -> ScrollBarBuilder<'a, C>
    where
        T: WidgetContainer,
    {
        ScrollBarBuilder::with_builder(self, target)
    }

    /// Transition into building a [`Select`](crate::Select)
    pub fn select<I>(self) -> SelectBuilder<'a, C, I>
    where
        I: Ord + Copy + Send + 'static,
    {
        SelectBuilder::with_builder(self)
    }
}
