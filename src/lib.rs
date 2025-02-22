mod builder;
mod button;
mod theme;

use std::sync::Arc;

use basalt::interface::Bin;
use basalt::window::Window;

pub use self::builder::WidgetBuilder;
pub use self::button::{Button, ButtonBuilder};
pub use self::theme::{Theme, ThemeColors};

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
