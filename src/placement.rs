pub use Position::{Anchor, Floating, Relative};
pub use UnitValue::{Percent, Pixels, Undefined};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct WidgetPlacement {
    pub position: Position,
    pub top: UnitValue,
    pub bottom: UnitValue,
    pub left: UnitValue,
    pub right: UnitValue,
    pub width: UnitValue,
    pub height: UnitValue,
}

pub struct WidgetPlcmtError {
    pub kind: WidgetPlcmtErrorKind,
    pub desc: &'static str,
}

pub enum WidgetPlcmtErrorKind {
    NotConstrained,
    TooConstrained,
}

impl WidgetPlacement {
    #[allow(dead_code)]
    pub(crate) fn validate(&self) -> Result<(), WidgetPlcmtError> {
        match self.position {
            Relative | Anchor => {
                match [
                    self.top.is_defined(),
                    self.bottom.is_defined(),
                    self.height.is_defined(),
                ] {
                    [false, false, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "Two of 'top`, 'bottom` and 'height' must be defined.",
                        });
                    },
                    [true, true, true] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::TooConstrained,
                            desc: "Only two of 'top`, 'bottom` and 'height' must be defined.",
                        });
                    },
                    [true, false, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'top' is defined, but either 'bottom' or 'height' must also be \
                                   defined.",
                        });
                    },
                    [false, true, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'bottom' is defined, but either 'top' or 'height' must also be \
                                   defined.",
                        });
                    },
                    [false, false, true] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'height' is defined, but either 'top' or 'bottom' must also be \
                                   defined.",
                        });
                    },
                    _ => (),
                }

                match [
                    self.left.is_defined(),
                    self.right.is_defined(),
                    self.width.is_defined(),
                ] {
                    [false, false, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "Two of 'left`, 'right` and 'width' must be defined.",
                        });
                    },
                    [true, true, true] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::TooConstrained,
                            desc: "Only two of 'left`, 'right` and 'width' must be defined.",
                        });
                    },
                    [true, false, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'left' is defined, but either 'right' or 'width' must also be \
                                   defined.",
                        });
                    },
                    [false, true, false] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'right' is defined, but either 'left' or 'width' must also be \
                                   defined.",
                        });
                    },
                    [false, false, true] => {
                        return Err(WidgetPlcmtError {
                            kind: WidgetPlcmtErrorKind::NotConstrained,
                            desc: "'width' is defined, but either 'left' or 'right' must also be \
                                   defined.",
                        });
                    },
                    _ => (),
                }

                if self.position == Anchor
                    && self.top.is_defined()
                    && self.bottom.is_defined()
                    && self.left.is_defined()
                    && self.right.is_defined()
                {
                    // TODO: this error desc is probably confusing.
                    return Err(WidgetPlcmtError {
                        kind: WidgetPlcmtErrorKind::TooConstrained,
                        desc: "'top', 'bottom', 'left' and 'right' are defined. If using both \
                               'top' & 'bottom', 'left' & 'width' or 'right' & 'width' must be \
                               used instead of 'left' & 'right'. If using both 'left' & 'right', \
                               'top' & 'height' or 'bottom' & height' must be used instead of \
                               'top & 'bottom'.",
                    });
                }
            },
            Floating => {
                if self.width == Undefined {
                    return Err(WidgetPlcmtError {
                        kind: WidgetPlcmtErrorKind::NotConstrained,
                        desc: "'width' must be defined.",
                    });
                }

                if self.height == Undefined {
                    return Err(WidgetPlcmtError {
                        kind: WidgetPlcmtErrorKind::NotConstrained,
                        desc: "'height' must be defined.",
                    });
                }
            },
        }

        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    #[default]
    Relative,
    Floating,
    Anchor,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum UnitValue {
    #[default]
    Undefined,
    Pixels(f32),
    Percent(f32),
}

impl UnitValue {
    fn is_defined(self) -> bool {
        match self {
            Undefined => false,
            _ => true,
        }
    }
}
