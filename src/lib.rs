pub mod builder;
pub mod error;

mod button;
mod hori_scaler;
mod spin_button;
mod switch_button;
mod theme;
mod toggle_button;

use std::sync::Arc;

use basalt::interface::Bin;
use basalt::window::Window;

use self::builder::WidgetBuilder;
pub use self::button::Button;
pub use self::hori_scaler::{HoriScaler, ScalerRound};
pub use self::spin_button::SpinButton;
pub use self::switch_button::SwitchButton;
pub use self::theme::{Theme, ThemeColors};
pub use self::toggle_button::ToggleButton;

pub trait Container {
    fn create_widget(&self) -> WidgetBuilder;
}

impl Container for Arc<Window> {
    fn create_widget(&self) -> WidgetBuilder {
        WidgetBuilder::with_window(self.clone())
    }
}

impl Container for Arc<Bin> {
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
