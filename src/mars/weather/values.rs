// Typed values of one weather reading: compass, sky, UV, pressure trend, temperature, and solar longitude.
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

// Solar longitude wraps to 0 after a full orbit.
const DEGREES_PER_ORBIT: u16 = 360;

// Feed labels decode through these serde names (plus aliases); `Unknown` catches labels we do not recognise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WindDirection {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
    #[serde(other)]
    Unknown,
}

impl Display for WindDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            WindDirection::N       => write!(f, "N"),
            WindDirection::NE      => write!(f, "NE"),
            WindDirection::E       => write!(f, "E"),
            WindDirection::SE      => write!(f, "SE"),
            WindDirection::S       => write!(f, "S"),
            WindDirection::SW      => write!(f, "SW"),
            WindDirection::W       => write!(f, "W"),
            WindDirection::NW      => write!(f, "NW"),
            WindDirection::Unknown => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AtmoOpacity {
    Sunny,
    Cloudy,
    Dusty,
    #[serde(other)]
    Unknown,
}

impl Display for AtmoOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            AtmoOpacity::Sunny   => write!(f, "Sunny"),
            AtmoOpacity::Cloudy  => write!(f, "Cloudy"),
            AtmoOpacity::Dusty   => write!(f, "Dusty"),
            AtmoOpacity::Unknown => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UvIndex {
    Low,
    Moderate,
    High,
    // The API name stays VeryHigh; the alias accepts the feed's spelling.
    #[serde(alias = "Very_High")]
    VeryHigh,
    Extreme,
    #[serde(other)]
    Unknown,
}

impl Display for UvIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            UvIndex::Low      => write!(f, "Low"),
            UvIndex::Moderate => write!(f, "Moderate"),
            UvIndex::High     => write!(f, "High"),
            UvIndex::VeryHigh => write!(f, "Very High"),
            UvIndex::Extreme  => write!(f, "Extreme"),
            UvIndex::Unknown  => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PressureDirection {
    // The feed reports the trend as Higher or Lower; any other label is no trend at all.
    #[serde(alias = "Higher")]
    Rising,
    #[serde(alias = "Lower")]
    Falling,
}

impl Display for PressureDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            PressureDirection::Rising  => write!(f, "Rising"),
            PressureDirection::Falling => write!(f, "Falling"),
        };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Temperature {
    celsius: f64,
}

impl Temperature {
    #[must_use]
    pub const fn from_celsius(celsius: f64) -> Self {
        return Temperature { celsius };
    }
}

impl Display for Temperature {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return write!(f, "{} °C", self.celsius);
    }
}

// Mars' orbital position in whole degrees, 0 to 359; the feed calls it `ls`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SolarLongitude(u16);

impl TryFrom<u16> for SolarLongitude {
    type Error = String;

    fn try_from(degrees: u16) -> Result<Self, Self::Error> {
        if degrees >= DEGREES_PER_ORBIT {
            return Err(format!("solar longitude {degrees} is not below {DEGREES_PER_ORBIT} degrees"));
        }
        return Ok(SolarLongitude(degrees));
    }
}

impl From<SolarLongitude> for u16 {
    fn from(longitude: SolarLongitude) -> Self {
        return longitude.0;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::AtmoOpacity;
    use super::PressureDirection;
    use super::SolarLongitude;
    use super::Temperature;
    use super::UvIndex;
    use super::WindDirection;
    use crate::mars::openapi_contract::{self, assert_enum_matches};

    // Each list's match stops compiling when a variant is added, forcing it into the list and the spec.
    fn all_wind_directions() -> [WindDirection; 9] {
        let all: [WindDirection; 9] = [
            WindDirection::N, WindDirection::NE, WindDirection::E, WindDirection::SE, WindDirection::S,
            WindDirection::SW, WindDirection::W, WindDirection::NW, WindDirection::Unknown,
        ];
        all.iter().for_each(|d: &WindDirection| match d {
            WindDirection::N | WindDirection::NE | WindDirection::E | WindDirection::SE | WindDirection::S
            | WindDirection::SW | WindDirection::W | WindDirection::NW | WindDirection::Unknown => {},
        });
        return all;
    }

    fn all_atmo_opacities() -> [AtmoOpacity; 4] {
        let all: [AtmoOpacity; 4] = [AtmoOpacity::Sunny, AtmoOpacity::Cloudy, AtmoOpacity::Dusty, AtmoOpacity::Unknown];
        all.iter().for_each(|o: &AtmoOpacity| match o {
            AtmoOpacity::Sunny | AtmoOpacity::Cloudy | AtmoOpacity::Dusty | AtmoOpacity::Unknown => {},
        });
        return all;
    }

    fn all_uv_indices() -> [UvIndex; 6] {
        let all: [UvIndex; 6] =
            [UvIndex::Low, UvIndex::Moderate, UvIndex::High, UvIndex::VeryHigh, UvIndex::Extreme, UvIndex::Unknown];
        all.iter().for_each(|u: &UvIndex| match u {
            UvIndex::Low | UvIndex::Moderate | UvIndex::High | UvIndex::VeryHigh | UvIndex::Extreme | UvIndex::Unknown => {},
        });
        return all;
    }

    fn all_pressure_directions() -> [PressureDirection; 2] {
        let all: [PressureDirection; 2] = [PressureDirection::Rising, PressureDirection::Falling];
        all.iter().for_each(|p: &PressureDirection| match p {
            PressureDirection::Rising | PressureDirection::Falling => {},
        });
        return all;
    }

    #[test]
    fn spec_enums_list_exactly_the_rust_variants() {
        let spec: Value = openapi_contract::spec();
        assert_enum_matches(&spec, "WindDirection", &all_wind_directions());
        assert_enum_matches(&spec, "AtmoOpacity", &all_atmo_opacities());
        assert_enum_matches(&spec, "UvIndex", &all_uv_indices());
        assert_enum_matches(&spec, "PressureDirection", &all_pressure_directions());
    }

    #[test]
    fn wind_direction_display_covers_all_variants() {
        for (direction, expected) in [
            (WindDirection::N,  "N"),
            (WindDirection::NE, "NE"),
            (WindDirection::E,  "E"),
            (WindDirection::SE, "SE"),
            (WindDirection::S,  "S"),
            (WindDirection::SW, "SW"),
            (WindDirection::W,  "W"),
            (WindDirection::NW, "NW"),
            (WindDirection::Unknown, "Unknown"),
        ] {
            assert_eq!(format!("{}", direction), expected);
        }
    }

    #[test]
    fn atmo_opacity_display_covers_all_variants() {
        for (opacity, expected) in [
            (AtmoOpacity::Sunny,  "Sunny"),
            (AtmoOpacity::Cloudy, "Cloudy"),
            (AtmoOpacity::Dusty,  "Dusty"),
            (AtmoOpacity::Unknown, "Unknown"),
        ] {
            assert_eq!(format!("{}", opacity), expected);
        }
    }

    #[test]
    fn uv_index_display_covers_all_variants() {
        for (uv, expected) in [
            (UvIndex::Low,      "Low"),
            (UvIndex::Moderate, "Moderate"),
            (UvIndex::High,     "High"),
            (UvIndex::VeryHigh, "Very High"),
            (UvIndex::Extreme,  "Extreme"),
            (UvIndex::Unknown,  "Unknown"),
        ] {
            assert_eq!(format!("{}", uv), expected);
        }
    }

    #[test]
    fn pressure_direction_display_covers_all_variants() {
        assert_eq!(format!("{}", PressureDirection::Rising), "Rising");
        assert_eq!(format!("{}", PressureDirection::Falling), "Falling");
    }

    #[test]
    fn temperature_serializes_transparently_as_number() -> Result<(), serde_json::Error> {
        let temp: Temperature = Temperature::from_celsius(-42.5);
        let encoded: String = serde_json::to_string(&temp)?;
        assert_eq!(encoded, "-42.5");
        return Ok(());
    }

    #[test]
    fn temperature_from_celsius_stores_value() {
        let t = Temperature::from_celsius(20.0);
        assert_eq!(t.celsius, 20.0);
    }

    #[test]
    fn temperature_display_formats_celsius() {
        let t = Temperature::from_celsius(-12.5);
        assert_eq!(format!("{}", t), "-12.5 °C");
    }

    #[test]
    fn solar_longitude_deserializes_with_the_range_check() -> Result<(), serde_json::Error> {
        assert_eq!(serde_json::from_value::<SolarLongitude>(json!(359))?, SolarLongitude(359));
        assert!(serde_json::from_value::<SolarLongitude>(json!(360)).is_err(), "API input gets the same range check");
        return Ok(());
    }
}
