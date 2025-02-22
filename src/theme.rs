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
    pub back3: Color,
    pub back4: Color,
    pub accent1: Color,
    pub accent2: Color,
    pub text1a: Color,
    pub text1b: Color,
    pub border1: Color,
    pub border2: Color,
}

impl ThemeColors {
    pub fn light() -> Self {
        Self {
            back1: Color::shex("fae5ee"),
            back2: Color::shex("f4e1ea"),
            back3: Color::shex("e0ced6"),
            back4: Color::shex("d4c2ca"),
            accent1: Color::shex("ff0071"),
            accent2: Color::shex("f2006c"),
            text1a: Color::shex("261d21"),
            text1b: Color::shex("fae5ee"),
            border1: Color::shex("5e585b"),
            border2: Color::shex("685e63"),
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
