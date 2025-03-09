use std::sync::Arc;
use std::time::Duration;

use basalt::interface::{BinStyle, ChildFloatMode};
use basalt::interval::IntvlHookCtrl;
use basalt::render::{MSAA, Renderer, RendererError};
use basalt::window::WindowOptions;
use basalt::{Basalt, BasaltOptions};
use basalt_widgets::{RadioButtonGroup, Theme, WidgetContainer};

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

        let _vert_scaler = background
            .create_widget()
            .vert_scaler()
            .max_value(100.0)
            .small_step(1.0)
            .medium_step(5.0)
            .large_step(10.0)
            .build()
            .unwrap();

        // Progress Bar

        let progress_bar = background
            .create_widget()
            .progress_bar()
            .set_pct(100.0)
            .build();

        progress_bar.on_press(|progress_bar, pct| {
            progress_bar.set_pct(pct);
        });

        let wk_progress_bar = Arc::downgrade(&progress_bar);
        let mut progress = 0.0;

        let hook_id =
            basalt
                .interval_ref()
                .do_every(Duration::from_millis(10), None, move |elapsed_op| {
                    if let Some(elapsed) = elapsed_op {
                        progress += elapsed.as_millis() as f32 / 20.0;

                        if progress > 100.0 {
                            progress = 0.0;
                        }

                        match wk_progress_bar.upgrade() {
                            Some(progress_bar) => {
                                progress_bar.set_pct(progress);
                                IntvlHookCtrl::Continue
                            },
                            None => IntvlHookCtrl::Remove,
                        }
                    } else {
                        IntvlHookCtrl::Continue
                    }
                });

        basalt.interval_ref().start(hook_id);

        // Radio Buttons

        #[derive(PartialEq, Debug)]
        enum RadioValue {
            A,
            B,
            C,
        }

        let radio_group = RadioButtonGroup::new();

        let _radio_a = background
            .create_widget()
            .radio_button(RadioValue::A)
            .group(&radio_group)
            .build();

        let _radio_b = background
            .create_widget()
            .radio_button(RadioValue::B)
            .group(&radio_group)
            .build();

        let _radio_c = background
            .create_widget()
            .radio_button(RadioValue::C)
            .group(&radio_group)
            .build();

        radio_group.on_change(move |radio_op| {
            println!("radio value: {:?}", radio_op.map(|radio| radio.value_ref()));
        });

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
