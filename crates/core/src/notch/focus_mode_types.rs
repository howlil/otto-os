#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CategoryColor {
    #[default]
    Orange,
    Blue,
    Indigo,
    Green,
    Purple,
    Teal,
    Pink,
}

impl CategoryColor {
    pub const ALL: [Self; 7] = [
        Self::Orange,
        Self::Blue,
        Self::Indigo,
        Self::Green,
        Self::Purple,
        Self::Teal,
        Self::Pink,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Orange => "orange",
            Self::Blue => "blue",
            Self::Indigo => "indigo",
            Self::Green => "green",
            Self::Purple => "purple",
            Self::Teal => "teal",
            Self::Pink => "pink",
        }
    }

    #[must_use]
    pub fn from_code(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "blue" => Self::Blue,
            "indigo" => Self::Indigo,
            "green" => Self::Green,
            "purple" => Self::Purple,
            "teal" => Self::Teal,
            "pink" => Self::Pink,
            _ => Self::Orange,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Orange => "Orange",
            Self::Blue => "Blue",
            Self::Indigo => "Indigo",
            Self::Green => "Green",
            Self::Purple => "Purple",
            Self::Teal => "Teal",
            Self::Pink => "Pink",
        }
    }

    #[must_use]
    pub const fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Orange => (255, 159, 10),
            Self::Blue => (10, 132, 255),
            Self::Indigo => (94, 92, 230),
            Self::Green => (48, 209, 88),
            Self::Purple => (191, 90, 242),
            Self::Teal => (64, 200, 224),
            Self::Pink => (255, 55, 95),
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Orange => Self::Blue,
            Self::Blue => Self::Indigo,
            Self::Indigo => Self::Green,
            Self::Green => Self::Purple,
            Self::Purple => Self::Teal,
            Self::Teal => Self::Pink,
            Self::Pink => Self::Orange,
        }
    }
}
