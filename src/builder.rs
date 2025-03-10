//! Builder types

use std::sync::Arc;

use basalt::interface::Bin;
use basalt::window::Window;

pub use crate::button::ButtonBuilder;
pub use crate::check_box::CheckBoxBuilder;
pub use crate::hori_scaler::HoriScalerBuilder;
pub use crate::progress_bar::ProgressBarBuilder;
pub use crate::radio_button::RadioButtonBuilder;
pub use crate::spin_button::SpinButtonBuilder;
pub use crate::switch_button::SwitchButtonBuilder;
pub use crate::toggle_button::ToggleButtonBuilder;
pub use crate::vert_scaler::VertScalerBuilder;
use crate::{Theme, WidgetParent};

/// General builder for widgets.
pub struct WidgetBuilder {
    pub(crate) parent: WidgetParent,
    pub(crate) theme: Theme,
}

impl WidgetBuilder {
    pub(crate) fn with_bin(bin: Arc<Bin>) -> Self {
        Self {
            parent: WidgetParent::Bin(bin),
            theme: Default::default(),
        }
    }

    pub(crate) fn with_window(window: Arc<Window>) -> Self {
        Self {
            parent: WidgetParent::Window(window),
            theme: Default::default(),
        }
    }

    /// Specifiy a theme to be used.
    ///
    /// **Note**: When not used the theme will be Basalt's default light theme.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Transition into building a [`Button`](crate::Button)
    pub fn button(self) -> ButtonBuilder {
        ButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`SpinButton`](crate::SpinButton)
    pub fn spin_button(self) -> SpinButtonBuilder {
        SpinButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`ToggleButton`](crate::ToggleButton)
    pub fn toggle_button(self) -> ToggleButtonBuilder {
        ToggleButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`SwitchButton`](crate::SwitchButton)
    pub fn switch_button(self) -> SwitchButtonBuilder {
        SwitchButtonBuilder::with_builder(self)
    }

    /// Transition into building a [`HoriScaler`](crate::HoriScaler)
    pub fn hori_scaler(self) -> HoriScalerBuilder {
        HoriScalerBuilder::with_builder(self)
    }

    /// Transition into building a [`VertScaler`](crate::VertScaler)
    pub fn vert_scaler(self) -> VertScalerBuilder {
        VertScalerBuilder::with_builder(self)
    }

    /// Transition into building a [`ProgressBar`](crate::ProgressBar)
    pub fn progress_bar(self) -> ProgressBarBuilder {
        ProgressBarBuilder::with_builder(self)
    }

    /// Transition into building a [`RadioButton`](crate::RadioButton)
    pub fn radio_button<T>(self, value: T) -> RadioButtonBuilder<T>
    where
        T: Send + Sync + 'static,
    {
        RadioButtonBuilder::with_builder(self, value)
    }

    /// Transition into building a [`CheckBox`](crate::CheckBox)
    pub fn check_box<T>(self, value: T) -> CheckBoxBuilder<T>
    where
        T: Send + Sync + 'static,
    {
        CheckBoxBuilder::with_builder(self, value)
    }
}
