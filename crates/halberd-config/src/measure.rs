//! How lengths are shown and typed: Hammer units, metres or feet.

use serde::{Deserialize, Serialize};

/// One Hammer unit in metres, at the scale Source uses for characters:
/// 1 unit is 1 inch, so a 72-unit player is 1.83 m tall.
pub const METRES_PER_UNIT: f64 = 0.0254;
/// Hammer units in a foot (a unit is an inch).
pub const UNITS_PER_FOOT: f64 = 12.0;

/// The unit lengths are shown and typed in. Maps are always stored in
/// Hammer units; this only changes what the editor displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LengthUnit {
    /// Hammer units, as Hammer shows them.
    #[default]
    Units,
    /// Metres.
    Metres,
    /// Feet (a Hammer unit is an inch, so 12 units are a foot).
    Feet,
    /// A value this version does not know (from a newer Halberd or a typo);
    /// replaced by the default when settings are loaded.
    #[serde(other)]
    Unknown,
}

impl LengthUnit {
    /// The choices to offer, in order.
    pub const CHOICES: [LengthUnit; 3] = [Self::Units, Self::Metres, Self::Feet];

    /// Name for menus.
    pub fn label(self) -> &'static str {
        match self {
            Self::Units | Self::Unknown => "Hammer units",
            Self::Metres => "Metres",
            Self::Feet => "Feet",
        }
    }

    /// Short suffix after a number, such as "128 u" or "3.25 m".
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Units | Self::Unknown => " u",
            Self::Metres => " m",
            Self::Feet => " ft",
        }
    }

    /// A length in Hammer units, converted to this unit.
    pub fn from_units(self, units: f64) -> f64 {
        match self {
            Self::Units | Self::Unknown => units,
            Self::Metres => units * METRES_PER_UNIT,
            Self::Feet => units / UNITS_PER_FOOT,
        }
    }

    /// A length in this unit, converted to Hammer units.
    pub fn to_units(self, value: f64) -> f64 {
        match self {
            Self::Units | Self::Unknown => value,
            Self::Metres => value / METRES_PER_UNIT,
            Self::Feet => value * UNITS_PER_FOOT,
        }
    }

    /// How much a number field changes per point dragged: about a unit.
    pub fn drag_speed(self) -> f64 {
        self.from_units(1.0)
    }

    /// Decimal places worth showing: none for units, centimetres for metres.
    pub fn decimals(self) -> usize {
        match self {
            Self::Units | Self::Unknown => 0,
            Self::Metres => 2,
            Self::Feet => 1,
        }
    }

    /// A length in Hammer units, written in this unit, such as "3.25 m".
    pub fn format(self, units: f64) -> String {
        format!(
            "{:.*}{}",
            self.decimals(),
            self.from_units(units),
            self.suffix()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_is_one_point_eight_three_metres() {
        assert_eq!(LengthUnit::Metres.format(72.0), "1.83 m");
        assert_eq!(LengthUnit::Units.format(72.0), "72 u");
        assert_eq!(LengthUnit::Metres.format(128.0), "3.25 m");
        assert_eq!(LengthUnit::Feet.format(72.0), "6.0 ft");
        assert_eq!(LengthUnit::Feet.to_units(1.5), 18.0);
    }

    #[test]
    fn conversions_round_trip() {
        for unit in LengthUnit::CHOICES {
            let back = unit.to_units(unit.from_units(128.0));
            assert!((back - 128.0).abs() < 1e-9, "{unit:?}");
        }
        assert!((LengthUnit::Metres.to_units(1.0) - 39.370_078).abs() < 1e-5);
    }
}
