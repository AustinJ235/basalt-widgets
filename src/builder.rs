use std::sync::Arc;

use basalt::interface::Bin;
use basalt::window::Window;

use crate::{
    ButtonBuilder, HoriScalerBuilder, SpinButtonBuilder, SwitchButtonBuilder, Theme,
    ToggleButtonBuilder, WidgetParent,
};

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

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn button(self) -> ButtonBuilder {
        ButtonBuilder::with_builder(self)
    }

    pub fn spin_button(self) -> SpinButtonBuilder {
        SpinButtonBuilder::with_builder(self)
    }

    pub fn toggle_button(self) -> ToggleButtonBuilder {
        ToggleButtonBuilder::with_builder(self)
    }

    pub fn switch_button(self) -> SwitchButtonBuilder {
        SwitchButtonBuilder::with_builder(self)
    }

    pub fn hori_scaler(self) -> HoriScalerBuilder {
        HoriScalerBuilder::with_builder(self)
    }
}
