#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_inception)]
#![allow(clippy::doc_lazy_continuation)]

pub mod builder;
pub mod error;

mod button;
mod check_box;
mod progress_bar;
mod radio_button;
mod scaler;
mod spin_button;
mod switch_button;
mod theme;
mod toggle_button;

use std::sync::Arc;

use basalt::interface::Bin;
use basalt::window::Window;

use self::builder::WidgetBuilder;
pub use self::button::Button;
pub use self::check_box::CheckBox;
pub use self::progress_bar::ProgressBar;
pub use self::radio_button::{RadioButton, RadioButtonGroup};
pub use self::scaler::{Scaler, ScalerOrientation, ScalerRound};
pub use self::spin_button::SpinButton;
pub use self::switch_button::SwitchButton;
pub use self::theme::{Theme, ThemeColors};
pub use self::toggle_button::ToggleButton;

/// Trait used by containers that support containing widgets.
pub trait WidgetContainer {
    fn create_widget(&self) -> WidgetBuilder;
    // fn container_bin(&self) -> &Arc<Bin>;
    // fn default_theme(&self) -> Option<Theme>;
}

impl WidgetContainer for Arc<Window> {
    fn create_widget(&self) -> WidgetBuilder {
        WidgetBuilder::with_window(self.clone())
    }
}

impl WidgetContainer for Arc<Bin> {
    fn create_widget(&self) -> WidgetBuilder {
        WidgetBuilder::with_bin(self.clone())
    }
}

enum WidgetParent {
    Window(Arc<Window>),
    Bin(Arc<Bin>),
}

impl WidgetParent {
    fn window(&self) -> Arc<Window> {
        match self {
            Self::Window(window) => window.clone(),
            Self::Bin(bin) => bin.window().unwrap(),
        }
    }
}
