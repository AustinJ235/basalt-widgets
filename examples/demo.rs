use basalt::interface::{BinStyle, ChildFloatMode};
use basalt::render::{MSAA, Renderer, RendererError};
use basalt::window::WindowOptions;
use basalt::{Basalt, BasaltOptions};
use basalt_widgets::{Container, Theme};

fn main() {
    Basalt::initialize(BasaltOptions::default(), move |basalt_res| {
        let basalt = basalt_res.unwrap();
        let theme = Theme::default();

        let window = basalt
            .window_manager_ref()
            .create(WindowOptions {
                title: String::from("app"),
                inner_size: Some([400; 2]),
                ..WindowOptions::default()
            })
            .unwrap();

        let background = window.new_bin();

        background
            .style_update(BinStyle {
                pos_from_t: Some(0.0),
                pos_from_b: Some(0.0),
                pos_from_l: Some(0.0),
                pos_from_r: Some(0.0),
                back_color: Some(theme.colors.back1),
                child_float_mode: Some(ChildFloatMode::Column),
                ..BinStyle::default()
            })
            .expect_valid();

        let _button = background.create_widget().button().text("Button").build();

        let _spin_button = background
            .create_widget()
            .spin_button()
            .max_value(100)
            .medium_step(5)
            .large_step(10)
            .build()
            .unwrap();

        let _toggle_button = background
            .create_widget()
            .toggle_button()
            .enabled_text("On")
            .disabled_text("Off")
            .build();

        let _switch_button = background.create_widget().switch_button().build();

        let _hori_scaler = background
            .create_widget()
            .hori_scaler()
            .max_value(100.0)
            .small_step(1.0)
            .medium_step(5.0)
            .large_step(10.0)
            .build()
            .unwrap();

        let mut renderer = Renderer::new(window).unwrap();

        renderer.interface_only().msaa(MSAA::X8);

        match renderer.run() {
            Ok(_) | Err(RendererError::Closed) => (),
            Err(e) => {
                println!("{:?}", e);
            },
        }

        basalt.exit();
    });
}
