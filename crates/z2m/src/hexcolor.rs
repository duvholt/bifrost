use std::fmt::Display;

use serde::{Deserialize, Serialize};

use hue::xy::XY;

use crate::error::Z2mError;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(into = "String", try_from = "&str")]
pub struct HexColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl HexColor {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub fn to_xy_color(&self) -> XY {
        XY::from_rgb(self.r, self.g, self.b).0
    }
}

impl From<[u8; 3]> for HexColor {
    fn from([r, g, b]: [u8; 3]) -> Self {
        Self::new(r, g, b)
    }
}

impl From<HexColor> for String {
    fn from(value: HexColor) -> Self {
        format!("{value}")
    }
}

impl Display for HexColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl TryFrom<&str> for HexColor {
    type Error = Z2mError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 7 || !value.starts_with('#') {
            return Err(Z2mError::InvalidHexColor);
        }
        let r = u8::from_str_radix(&value[1..3], 16)?;
        let g = u8::from_str_radix(&value[3..5], 16)?;
        let b = u8::from_str_radix(&value[5..7], 16)?;
        Ok(Self { r, g, b })
    }
}

#[cfg(test)]
mod tests {
    use crate::hexcolor::HexColor;

    #[test]
    fn make_hexcolor() {
        let h = HexColor::new(0, 0, 0);
        assert_eq!(h.to_string(), "#000000");

        let h = HexColor::new(255, 255, 255);
        assert_eq!(h.to_string(), "#ffffff");

        let h = HexColor::new(255, 0, 0);
        assert_eq!(h.to_string(), "#ff0000");

        let h = HexColor::new(0, 255, 0);
        assert_eq!(h.to_string(), "#00ff00");

        let h = HexColor::new(0, 0, 255);
        assert_eq!(h.to_string(), "#0000ff");

        let h = HexColor::new(128, 192, 255);
        assert_eq!(h.to_string(), "#80c0ff");
    }

    #[test]
    fn parse_hexcolor() {
        assert_eq!(
            HexColor::try_from(HexColor::new(0, 1, 2).to_string().as_str()).unwrap(),
            HexColor::new(0, 1, 2)
        );
        assert_eq!(
            HexColor::try_from(HexColor::new(192, 199, 255).to_string().as_str()).unwrap(),
            HexColor::new(192, 199, 255)
        );
        assert_eq!(
            HexColor::try_from(HexColor::new(255, 255, 255).to_string().as_str()).unwrap(),
            HexColor::new(255, 255, 255)
        );
    }
}
