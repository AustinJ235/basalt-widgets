use basalt::interface::{Color, FontWeight};

pub struct Theme {
    pub spacing: f32,
    pub text_height: f32,
    pub font_family: String,
    pub font_weight: FontWeight,
    pub border: Option<f32>,
    pub roundness: Option<f32>,
    pub colors: ThemeColors,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            spacing: 12.0,
            text_height: 14.0,
            font_family: String::from("Sans Serif"),
            font_weight: FontWeight::Normal,
            border: Some(1.0),
            roundness: Some(3.0),
            colors: ThemeColors::light(),
        }
    }

    pub fn dark() -> Self {
        todo!()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

pub struct ThemeColors {
    pub back1: Color,
    pub back2: Color,
    pub accent1: Color,
    pub accent2: Color,
    pub text1: Color,
    pub text2: Color,
    pub border1: Color,
}

impl ThemeColors {
    pub fn light() -> Self {
        Self {
            back1: Color::shex("EDEDEE"),
            back2: Color::shex("F4F5F5"),
            accent1: Color::shex("f9787f"),
            accent2: Color::shex("EB6B73"),
            text1: Color::shex("8F9194"),
            text2: Color::shex("EDEDEE"),
            border1: Color::shex("C2C3C3"),
        }
    }

    pub fn dark() -> Self {
        todo!()
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::light()
    }
}
