// Streams a feed body to its first sol without a JSON tree. The decoder reads strings in place.
// Skipped values are still parsed in full, so a body fails exactly when it fails as a serde_json::Value.
use std::fmt::{self, Formatter};

use arrayvec::ArrayString;
use jiff::civil::{Date, Time};
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::de::{self, DeserializeOwned, DeserializeSeed, Deserializer, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Number;
use serde_json::de::{Deserializer as JsonDeserializer, SliceRead};

use super::{AtmoOpacity, FetchError, NO_READING, PressureDirection, SEASON_CAP, SolarLongitude, Temperature, UvIndex};
use super::{WeatherSample, WindDirection, truncate_season};

const SOLES: &str = "soles";
const DATE_FORMAT: &str = "%Y-%m-%d";
const TIME_FORMAT: &str = "%H:%M";
const SOL_FIELD_COUNT: usize = 17;

// What a decoder makes of each JSON shape. A decoder parses any shape it does not override, then gives `absent`.
pub trait Shape<'de>: Sized {
    type Output;

    fn absent(self) -> Self::Output;

    fn text(self, _text: &str) -> Self::Output {
        return self.absent();
    }

    fn unsigned(self, _n: u64) -> Self::Output {
        return self.absent();
    }

    fn signed(self, _n: i64) -> Self::Output {
        return self.absent();
    }

    fn float(self, _n: f64) -> Self::Output {
        return self.absent();
    }

    fn seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Output, A::Error> {
        drain_seq(seq)?;
        return Ok(self.absent());
    }

    fn map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Output, A::Error> {
        drain_map(map)?;
        return Ok(self.absent());
    }
}

// Accepts any JSON value and hands it to its shape.
pub struct Lenient<S>(S);

impl<'de, S: Shape<'de>> Visitor<'de> for Lenient<S> {
    type Value = S::Output;

    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return f.write_str("any JSON value");
    }

    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<S::Output, E> {
        return Ok(self.0.absent());
    }

    fn visit_unit<E: de::Error>(self) -> Result<S::Output, E> {
        return Ok(self.0.absent());
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<S::Output, E> {
        return Ok(self.0.unsigned(v));
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<S::Output, E> {
        return Ok(self.0.signed(v));
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<S::Output, E> {
        return Ok(self.0.float(v));
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<S::Output, E> {
        return Ok(self.0.text(v));
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<S::Output, A::Error> {
        return self.0.seq(seq);
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<S::Output, A::Error> {
        return self.0.map(map);
    }
}

impl<'de, S: Shape<'de>> DeserializeSeed<'de> for Lenient<S> {
    type Value = S::Output;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<S::Output, D::Error> {
        return deserializer.deserialize_any(self);
    }
}

pub struct Discard;

impl Shape<'_> for Discard {
    type Output = ();

    fn absent(self) {}
}

fn drain_seq<'de, A: SeqAccess<'de>>(mut seq: A) -> Result<(), A::Error> {
    while seq.next_element_seed(Lenient(Discard))?.is_some() {}
    return Ok(());
}

fn drain_map<'de, A: MapAccess<'de>>(mut map: A) -> Result<(), A::Error> {
    while map.next_entry_seed(Lenient(Discard), Lenient(Discard))?.is_some() {}
    return Ok(());
}

// A value from one JSON form or another. The "--" no-data mark and every other form give None.
pub struct Reading<T> {
    from_text: fn(&str) -> Option<T>,
    from_unsigned: fn(u64) -> Option<T>,
    from_signed: fn(i64) -> Option<T>,
    from_float: fn(f64) -> Option<T>,
}

impl<T> Shape<'_> for Reading<T> {
    type Output = Option<T>;

    fn absent(self) -> Option<T> {
        return None;
    }

    fn text(self, text: &str) -> Option<T> {
        if text == NO_READING {
            return None;
        }
        return (self.from_text)(text);
    }

    fn unsigned(self, n: u64) -> Option<T> {
        return (self.from_unsigned)(n);
    }

    fn signed(self, n: i64) -> Option<T> {
        return (self.from_signed)(n);
    }

    fn float(self, n: f64) -> Option<T> {
        return (self.from_float)(n);
    }
}

const fn not_a_label<N: Copy, T>(_n: N) -> Option<T> {
    return None;
}

// A measurement as a JSON number or numeric string. Number::as_f64 widens integers as Value did.
const fn measurement() -> Lenient<Reading<f64>> {
    return Lenient(Reading {
        from_text: |text: &str| text.parse::<f64>().ok(),
        from_unsigned: |n: u64| Number::from(n).as_f64(),
        from_signed: |n: i64| Number::from(n).as_f64(),
        from_float: Some,
    });
}

// A whole count. The decoder refuses a fraction in either form and does not truncate it.
const fn count() -> Lenient<Reading<u32>> {
    return Lenient(Reading {
        from_text: |text: &str| text.parse::<u32>().ok(),
        from_unsigned: |n: u64| u32::try_from(n).ok(),
        from_signed: |n: i64| u32::try_from(n).ok(),
        from_float: not_a_label,
    });
}

const fn label<T>(from_text: fn(&str) -> Option<T>) -> Lenient<Reading<T>> {
    return Lenient(Reading { from_text, from_unsigned: not_a_label, from_signed: not_a_label, from_float: not_a_label });
}

fn feed_enum<T: DeserializeOwned>(text: &str) -> Option<T> {
    let deserializer: StrDeserializer<'_, ValueError> = text.into_deserializer();
    return T::deserialize(deserializer).ok();
}

fn date(text: &str) -> Option<Date> {
    return Date::strptime(DATE_FORMAT, text).ok();
}

fn time(text: &str) -> Option<Time> {
    return Time::strptime(TIME_FORMAT, text).ok();
}

fn season(text: &str) -> Option<ArrayString<SEASON_CAP>> {
    return Some(truncate_season(text));
}

fn solar_longitude(degrees: u32) -> Option<SolarLongitude> {
    return u16::try_from(degrees).ok().and_then(|degrees: u16| SolarLongitude::try_from(degrees).ok());
}

#[derive(Clone, Copy)]
pub enum NumberField {
    Sol,
    SolarLongitude,
    MinTemp,
    MaxTemp,
    MinGroundTemp,
    MaxGroundTemp,
    Pressure,
    AbsHumidity,
    WindSpeed,
}

#[derive(Clone, Copy)]
pub enum LabelField {
    TerrestrialDate,
    PressureDirection,
    WindDirection,
    AtmoOpacity,
    UvIndex,
    Season,
    Sunrise,
    Sunset,
}

#[derive(Clone, Copy)]
pub enum Field {
    Number(NumberField),
    Label(LabelField),
    Other,
}

struct FeedName {
    name: &'static str,
    field: Field,
}

const SOL_FIELDS: [FeedName; SOL_FIELD_COUNT] = [
    FeedName { name: "sol", field: Field::Number(NumberField::Sol) },
    FeedName { name: "ls", field: Field::Number(NumberField::SolarLongitude) },
    FeedName { name: "min_temp", field: Field::Number(NumberField::MinTemp) },
    FeedName { name: "max_temp", field: Field::Number(NumberField::MaxTemp) },
    FeedName { name: "min_gts_temp", field: Field::Number(NumberField::MinGroundTemp) },
    FeedName { name: "max_gts_temp", field: Field::Number(NumberField::MaxGroundTemp) },
    FeedName { name: "pressure", field: Field::Number(NumberField::Pressure) },
    FeedName { name: "abs_humidity", field: Field::Number(NumberField::AbsHumidity) },
    FeedName { name: "wind_speed", field: Field::Number(NumberField::WindSpeed) },
    FeedName { name: "terrestrial_date", field: Field::Label(LabelField::TerrestrialDate) },
    FeedName { name: "pressure_string", field: Field::Label(LabelField::PressureDirection) },
    FeedName { name: "wind_direction", field: Field::Label(LabelField::WindDirection) },
    FeedName { name: "atmo_opacity", field: Field::Label(LabelField::AtmoOpacity) },
    FeedName { name: "local_uv_irradiance_index", field: Field::Label(LabelField::UvIndex) },
    FeedName { name: "season", field: Field::Label(LabelField::Season) },
    FeedName { name: "sunrise", field: Field::Label(LabelField::Sunrise) },
    FeedName { name: "sunset", field: Field::Label(LabelField::Sunset) },
];

pub struct SolKey;

impl Shape<'_> for SolKey {
    type Output = Field;

    fn absent(self) -> Field {
        return Field::Other;
    }

    fn text(self, text: &str) -> Field {
        return SOL_FIELDS.iter().find(|known: &&FeedName| known.name == text).map_or(Field::Other, |known: &FeedName| known.field);
    }
}

// Each read overwrites the field, so the last of duplicate keys wins, as in a JSON object.
fn read_number<'de, A: MapAccess<'de>>(sample: &mut WeatherSample, field: NumberField, map: &mut A) -> Result<(), A::Error> {
    let celsius = |reading: Option<f64>| -> Option<Temperature> { return reading.map(Temperature::from_celsius) };
    return match field {
        NumberField::Sol => map.next_value_seed(count()).map(|sol: Option<u32>| sample.sol = sol.unwrap_or(0)),
        NumberField::SolarLongitude => {
            map.next_value_seed(count()).map(|ls: Option<u32>| sample.solar_longitude = ls.and_then(solar_longitude))
        },
        NumberField::MinTemp => map.next_value_seed(measurement()).map(|t: Option<f64>| sample.min_temp = celsius(t)),
        NumberField::MaxTemp => map.next_value_seed(measurement()).map(|t: Option<f64>| sample.max_temp = celsius(t)),
        NumberField::MinGroundTemp => map.next_value_seed(measurement()).map(|t: Option<f64>| sample.min_ground_temp = celsius(t)),
        NumberField::MaxGroundTemp => map.next_value_seed(measurement()).map(|t: Option<f64>| sample.max_ground_temp = celsius(t)),
        NumberField::Pressure => map.next_value_seed(measurement()).map(|p: Option<f64>| sample.pressure = p),
        NumberField::AbsHumidity => map.next_value_seed(measurement()).map(|h: Option<f64>| sample.abs_humidity = h),
        NumberField::WindSpeed => map.next_value_seed(measurement()).map(|s: Option<f64>| sample.wind_speed = s),
    };
}

fn read_label<'de, A: MapAccess<'de>>(sample: &mut WeatherSample, field: LabelField, map: &mut A) -> Result<(), A::Error> {
    return match field {
        LabelField::TerrestrialDate => map.next_value_seed(label(date)).map(|d: Option<Date>| sample.terrestrial_date = d),
        LabelField::PressureDirection => map
            .next_value_seed(label(feed_enum::<PressureDirection>))
            .map(|p: Option<PressureDirection>| sample.pressure_change_direction = p),
        LabelField::WindDirection => {
            map.next_value_seed(label(feed_enum::<WindDirection>)).map(|w: Option<WindDirection>| sample.wind_direction = w)
        },
        LabelField::AtmoOpacity => {
            map.next_value_seed(label(feed_enum::<AtmoOpacity>)).map(|o: Option<AtmoOpacity>| sample.atmo_opacity = o)
        },
        LabelField::UvIndex => map.next_value_seed(label(feed_enum::<UvIndex>)).map(|u: Option<UvIndex>| sample.uv_index = u),
        LabelField::Season => map.next_value_seed(label(season)).map(|s: Option<ArrayString<SEASON_CAP>>| sample.season = s),
        LabelField::Sunrise => map.next_value_seed(label(time)).map(|t: Option<Time>| sample.sunrise = t),
        LabelField::Sunset => map.next_value_seed(label(time)).map(|t: Option<Time>| sample.sunset = t),
    };
}

// One sol report. A report that is not an object reads as all-absent, as Value::get gave.
pub struct Sol;

impl<'de> Shape<'de> for Sol {
    type Output = WeatherSample;

    fn absent(self) -> WeatherSample {
        return WeatherSample {
            terrestrial_date: None,
            sol: 0,
            min_temp: None,
            max_temp: None,
            min_ground_temp: None,
            max_ground_temp: None,
            pressure: None,
            pressure_change_direction: None,
            solar_longitude: None,
            abs_humidity: None,
            wind_speed: None,
            wind_direction: None,
            atmo_opacity: None,
            uv_index: None,
            season: None,
            sunrise: None,
            sunset: None,
        };
    }

    fn map<A: MapAccess<'de>>(self, mut map: A) -> Result<WeatherSample, A::Error> {
        let mut sample: WeatherSample = self.absent();
        while let Some(field) = map.next_key_seed(Lenient(SolKey))? {
            match field {
                Field::Number(number) => read_number(&mut sample, number, &mut map)?,
                Field::Label(name) => read_label(&mut sample, name, &mut map)?,
                Field::Other => map.next_value_seed(Lenient(Discard))?,
            }
        }
        return Ok(sample);
    }
}

// The feed lists the latest sol first. The decoder parses the rest only to check the JSON.
pub struct Soles;

impl<'de> Shape<'de> for Soles {
    type Output = Option<WeatherSample>;

    fn absent(self) -> Option<WeatherSample> {
        return None;
    }

    fn seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Option<WeatherSample>, A::Error> {
        let latest: Option<WeatherSample> = seq.next_element_seed(Lenient(Sol))?;
        drain_seq(seq)?;
        return Ok(latest);
    }
}

#[derive(Clone, Copy)]
pub enum FeedKey {
    Soles,
    Other,
}

impl Shape<'_> for FeedKey {
    type Output = FeedKey;

    fn absent(self) -> FeedKey {
        return FeedKey::Other;
    }

    fn text(self, text: &str) -> FeedKey {
        return if text == SOLES { FeedKey::Soles } else { FeedKey::Other };
    }
}

pub struct Feed;

impl<'de> Shape<'de> for Feed {
    type Output = Option<WeatherSample>;

    fn absent(self) -> Option<WeatherSample> {
        return None;
    }

    // The last `soles` key wins, as in a JSON object.
    fn map<A: MapAccess<'de>>(self, mut map: A) -> Result<Option<WeatherSample>, A::Error> {
        let mut latest: Option<WeatherSample> = None;
        while let Some(key) = map.next_key_seed(Lenient(FeedKey::Other))? {
            match key {
                FeedKey::Soles => latest = map.next_value_seed(Lenient(Soles))?,
                FeedKey::Other => map.next_value_seed(Lenient(Discard))?,
            }
        }
        return Ok(latest);
    }
}

// Decodes an MSL feed body into its latest sol. The body is untrusted upstream input (fuzzed in fuzz/).
pub fn parse_weather_body(body: &[u8]) -> Result<WeatherSample, FetchError> {
    let mut json: JsonDeserializer<SliceRead<'_>> = JsonDeserializer::from_slice(body);
    let latest: Option<WeatherSample> = Lenient(Feed).deserialize(&mut json).map_err(FetchError::InvalidJson)?;
    json.end().map_err(FetchError::InvalidJson)?;
    return latest.ok_or(FetchError::NoSolData);
}

#[cfg(test)]
mod tests {
    use arrayvec::{ArrayString, ArrayVec};
    use jiff::civil::{Date, Time};
    use std::array::IntoIter;

    use serde::de::value::{BytesDeserializer, Error as ValueError, MapDeserializer, StrDeserializer};
    use serde::de::{DeserializeOwned, DeserializeSeed, IntoDeserializer};
    use serde_json::Value;

    use super::{Discard, Feed, Lenient, Sol, parse_weather_body};
    use crate::mars::test_support::display;
    use crate::mars::weather::{AtmoOpacity, FetchError, NO_READING, PressureDirection, SEASON_CAP, SolarLongitude};
    use crate::mars::weather::{Temperature, UvIndex, WeatherSample, WindDirection, truncate_season};

    const DEBUG_BYTES: usize = 2048;
    const MUTANT_BYTES: usize = 1024;
    const MUTANTS_PER_SEED: u32 = 500;
    const MAX_EDITS: usize = 4;
    const XORSHIFT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
    const XORSHIFT_LEFT: u32 = 13;
    const XORSHIFT_RIGHT: u32 = 7;
    const XORSHIFT_LEFT_AGAIN: u32 = 17;
    // serde_json stops at 128 levels. This depth is past it.
    const PAST_RECURSION_LIMIT: usize = 200;
    // JSON punctuation, digits, escape starts, a non-UTF-8 byte and the letters of "soles".
    const ALPHABET: &[u8] = b"{}[]\",:-.0123456789eE+\\u\xff solte_";

    const SEEDS: [&[u8]; 9] = [
        include_bytes!("../../../fuzz/seeds/weather_body/00_msl_feed_two_sols"),
        include_bytes!("../../../fuzz/seeds/weather_body/01_all_missing_sentinels"),
        include_bytes!("../../../fuzz/seeds/weather_body/02_numeric_json_fields"),
        include_bytes!("../../../fuzz/seeds/weather_body/03_multibyte_season_bad_times"),
        include_bytes!("../../../fuzz/seeds/weather_body/04_non_finite_and_overflow"),
        include_bytes!("../../../fuzz/seeds/weather_body/05_empty_soles"),
        include_bytes!("../../../fuzz/seeds/weather_body/06_truncated_json"),
        include_bytes!("../../../fuzz/seeds/weather_body/07_top_level_array"),
        include_bytes!("../../../fuzz/regressions/weather_body/crash-495f2ceb32621fe772fb460b6d5ffe26621c5f77"),
    ];

    const CRAFTED: [&[u8]; 30] = [
        br#"{"soles":[{"sol":"5"}],"soles":"x"}"#,
        br#"{"soles":"x","soles":[{"sol":"6"}]}"#,
        br#"{"soles":[{"sol":"5","sol":"x"}]}"#,
        br#"{"soles":[{"sol":"x","sol":"7"}]}"#,
        // \x5C is a backslash, so these bodies carry JSON \u escapes in keys and values.
        b"{\"sol\x5Cu0065s\":[{\"s\x5Cu006fl\":\"8\",\"season\":\"a\x5C\"b\x5Cu00e9\"}]}",
        b"{\"soles\":[{\"wind_direction\":\"\x5Cu004e\",\"sunrise\":\"0\x5Cu0036:00\",\"terrestrial_date\":\"2024\x5Cu002d01-31\"}]}",
        b"{\"soles\":[{\"wind_direction\":\"\x5Cu002d-\",\"season\":\"-\x5Cu002d\"}]}",
        br#"{"soles":[5]}"#,
        br#"{"soles":[[{"sol":"1"}]]}"#,
        br#"{"soles":["x"]}"#,
        br#"{"soles":[null,{"sol":"2"}]}"#,
        br#"{"soles":[{"sol":-1,"ls":359.0,"pressure":-0,"min_temp":-0.0,"max_temp":1e2}]}"#,
        br#"{"soles":[{"abs_humidity":18446744073709551615,"wind_speed":18446744073709551616,"min_gts_temp":-9223372036854775808}]}"#,
        br#"{"soles":[{"sol":4294967295,"ls":4294967296,"max_gts_temp":"0x10"}]}"#,
        br#"{"soles":[{"sol":null,"pressure":true,"season":{"a":[1,{"b":null}]},"sunset":["18:00"]}]}"#,
        b"[]",
        br#""soles""#,
        b"null",
        b"5",
        b"true",
        br#"{"soles":[{}]} x"#,
        b"{\"soles\":[{}]}  \n",
        br#"{"x":1e400,"soles":[{}]}"#,
        br#"{"soles":[{},{"a":"\ud800"}]}"#,
        br#"{"soles":[{}],"x":"\x"}"#,
        b"{\"x\":\"\xff\",\"soles\":[{}]}",
        b"",
        br#"{"soles":{}}"#,
        br#"{"x":{"soles":[{"sol":"1"}]}}"#,
        br#"{"soles":[{"sol":"1"}],}"#,
    ];

    // The serde_json::Value decoder this module replaced, kept unchanged as the oracle.
    fn parse_f64(report: &Value, key: &str) -> Option<f64> {
        let value: &Value = report.get(key)?;
        return value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()));
    }

    fn parse_u32(report: &Value, key: &str) -> Option<u32> {
        let value: &Value = report.get(key)?;
        return value.as_u64().and_then(|n| u32::try_from(n).ok())
            .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()));
    }

    fn feed_str<'a>(report: &'a Value, key: &str) -> Option<&'a str> {
        return report.get(key).and_then(Value::as_str).filter(|s: &&str| *s != NO_READING);
    }

    fn feed_enum<T: DeserializeOwned>(report: &Value, key: &str) -> Option<T> {
        let deserializer: StrDeserializer<'_, ValueError> = feed_str(report, key)?.into_deserializer();
        return T::deserialize(deserializer).ok();
    }

    fn oracle_sample(report: &Value) -> WeatherSample {
        let terrestrial_date: Option<Date> =
            feed_str(report, "terrestrial_date").and_then(|s| Date::strptime("%Y-%m-%d", s).ok());
        let sol: u32 = parse_u32(report, "sol").unwrap_or(0);
        let min_temp: Option<Temperature> = parse_f64(report, "min_temp").map(Temperature::from_celsius);
        let max_temp: Option<Temperature> = parse_f64(report, "max_temp").map(Temperature::from_celsius);
        let min_ground_temp: Option<Temperature> = parse_f64(report, "min_gts_temp").map(Temperature::from_celsius);
        let max_ground_temp: Option<Temperature> = parse_f64(report, "max_gts_temp").map(Temperature::from_celsius);
        let pressure: Option<f64> = parse_f64(report, "pressure");
        let pressure_change_direction: Option<PressureDirection> = feed_enum(report, "pressure_string");
        let solar_longitude: Option<SolarLongitude> = parse_u32(report, "ls")
            .and_then(|degrees: u32| u16::try_from(degrees).ok())
            .and_then(|degrees: u16| SolarLongitude::try_from(degrees).ok());
        let abs_humidity: Option<f64> = parse_f64(report, "abs_humidity");
        let wind_speed: Option<f64> = parse_f64(report, "wind_speed");
        let wind_direction: Option<WindDirection> = feed_enum(report, "wind_direction");
        let atmo_opacity: Option<AtmoOpacity> = feed_enum(report, "atmo_opacity");
        let uv_index: Option<UvIndex> = feed_enum(report, "local_uv_irradiance_index");
        let season: Option<ArrayString<SEASON_CAP>> = feed_str(report, "season").map(truncate_season);
        let sunrise: Option<Time> = feed_str(report, "sunrise").and_then(|s| Time::strptime("%H:%M", s).ok());
        let sunset: Option<Time> = feed_str(report, "sunset").and_then(|s| Time::strptime("%H:%M", s).ok());
        return WeatherSample {
            terrestrial_date, sol, min_temp, max_temp, min_ground_temp, max_ground_temp, pressure,
            pressure_change_direction, solar_longitude, abs_humidity, wind_speed, wind_direction,
            atmo_opacity, uv_index, season, sunrise, sunset,
        };
    }

    fn oracle_body(body: &[u8]) -> Result<WeatherSample, FetchError> {
        let json: Value = serde_json::from_slice(body).map_err(FetchError::InvalidJson)?;
        return match json.get("soles").and_then(Value::as_array).and_then(|soles| soles.first()) {
            Some(latest) => Ok(oracle_sample(latest)),
            None => Err(FetchError::NoSolData),
        };
    }

    #[derive(Debug, Default)]
    struct Tally {
        decoded: u32,
        invalid_json: u32,
        no_sol_data: u32,
    }

    // Debug text tells -0.0 from 0.0 and shows NaN and the error position, which PartialEq and JSON hide.
    fn assert_same(tally: Tally, body: &[u8]) -> Tally {
        let streamed: Result<WeatherSample, FetchError> = parse_weather_body(body);
        let expected: Result<WeatherSample, FetchError> = oracle_body(body);
        let got: ArrayString<DEBUG_BYTES> = display(&format_args!("{streamed:?}"));
        let want: ArrayString<DEBUG_BYTES> = display(&format_args!("{expected:?}"));
        assert_eq!(got, want, "the streamed decode must match the Value decode for {}", body.escape_ascii());
        return match streamed {
            Ok(_) => Tally { decoded: tally.decoded + 1, ..tally },
            Err(FetchError::InvalidJson(_)) => Tally { invalid_json: tally.invalid_json + 1, ..tally },
            Err(_) => Tally { no_sol_data: tally.no_sol_data + 1, ..tally },
        };
    }

    struct XorShift(u64);

    impl XorShift {
        fn below(&mut self, bound: usize) -> usize {
            let mut x: u64 = self.0;
            x ^= x << XORSHIFT_LEFT;
            x ^= x >> XORSHIFT_RIGHT;
            x ^= x << XORSHIFT_LEFT_AGAIN;
            self.0 = x;
            let wide_bound: u64 = u64::try_from(bound.max(1)).unwrap_or(u64::MAX);
            return usize::try_from(x % wide_bound).unwrap_or(0);
        }
    }

    #[derive(Clone, Copy)]
    enum Edit {
        Replace,
        Delete,
        Insert,
        Truncate,
    }

    // Truncation is rare, so most mutants keep their tail and reach the decoder's later fields.
    const EDITS: [Edit; 7] = [Edit::Replace, Edit::Replace, Edit::Replace, Edit::Delete, Edit::Insert, Edit::Insert, Edit::Truncate];

    fn mutate(seed: &[u8], rng: &mut XorShift) -> ArrayVec<u8, MUTANT_BYTES> {
        let mut mutant: ArrayVec<u8, MUTANT_BYTES> = seed.iter().copied().collect();
        let edits: usize = 1 + rng.below(MAX_EDITS);
        (0..edits).for_each(|_| {
            let at: usize = rng.below(mutant.len());
            let byte: u8 = ALPHABET[rng.below(ALPHABET.len())];
            match EDITS[rng.below(EDITS.len())] {
                Edit::Replace if at < mutant.len() => mutant[at] = byte,
                Edit::Delete if at < mutant.len() => drop(mutant.remove(at)),
                Edit::Insert => drop(mutant.try_insert(at, byte)),
                Edit::Truncate => mutant.truncate(at),
                Edit::Replace | Edit::Delete => {},
            }
        });
        return mutant;
    }

    #[test]
    fn streamed_decode_matches_the_value_decode_on_seeds_and_crafted_bodies() {
        let seeds: Tally = SEEDS.iter().fold(Tally::default(), |tally: Tally, body: &&[u8]| assert_same(tally, body));
        let all: Tally = CRAFTED.iter().fold(seeds, |tally: Tally, body: &&[u8]| assert_same(tally, body));
        let is_every_outcome: bool = all.decoded > 0 && all.invalid_json > 0 && all.no_sol_data > 0;
        assert!(is_every_outcome, "the fixed bodies must reach every outcome: {all:?}");
    }

    #[test]
    fn streamed_decode_matches_the_value_decode_past_the_recursion_limit() {
        let deep: [u8; PAST_RECURSION_LIMIT] = [b'['; PAST_RECURSION_LIMIT];
        let mut in_sol: ArrayVec<u8, MUTANT_BYTES> = br#"{"soles":[{"x":"#.iter().copied().collect();
        in_sol.extend(deep);
        let tally: Tally = assert_same(assert_same(Tally::default(), &deep), &in_sol);
        assert_eq!(tally.invalid_json, 2, "both depths must hit serde_json's recursion limit: {tally:?}");
    }

    #[test]
    fn streamed_decode_matches_the_value_decode_on_mutated_seeds() {
        let mut rng: XorShift = XorShift(XORSHIFT_SEED);
        let tally: Tally = SEEDS.iter().fold(Tally::default(), |tally: Tally, seed: &&[u8]| {
            return (0..MUTANTS_PER_SEED).fold(tally, |inner: Tally, _| assert_same(inner, &mutate(seed, &mut rng)));
        });
        let is_every_outcome: bool = tally.decoded > 0 && tally.invalid_json > 0 && tally.no_sol_data > 0;
        assert!(is_every_outcome, "mutants must reach every outcome, or a path goes uncompared: {tally:?}");
    }

    // JSON keys are always strings. The in-memory deserializers of serde reach the non-string key paths.
    #[test]
    fn non_string_keys_read_as_unknown_fields() -> Result<(), ValueError> {
        let sol_map: MapDeserializer<'_, IntoIter<(u64, u64), 1>, ValueError> = MapDeserializer::new([(1, 5)].into_iter());
        let sample: WeatherSample = Lenient(Sol).deserialize(sol_map)?;
        assert_eq!(sample.sol, 0, "a numeric key names no field, so its value is skipped");
        let feed_map: MapDeserializer<'_, IntoIter<(u64, u64), 1>, ValueError> = MapDeserializer::new([(1, 5)].into_iter());
        let latest: Option<WeatherSample> = Lenient(Feed).deserialize(feed_map)?;
        assert!(latest.is_none(), "a numeric key is not soles");
        return Ok(());
    }

    #[test]
    fn a_shape_serde_json_never_sends_names_what_was_expected() {
        let Err(error) = Lenient(Discard).deserialize(BytesDeserializer::<ValueError>::new(b"x")) else {
            panic!("raw bytes are not a JSON value");
        };
        let text: ArrayString<DEBUG_BYTES> = display(&error);
        assert!(text.contains("any JSON value"), "the error names the accepted input: {text}");
    }

    #[test]
    fn last_duplicate_key_wins_at_both_levels() -> Result<(), FetchError> {
        let sample: WeatherSample = parse_weather_body(br#"{"soles":"x","soles":[{"sol":"5","sol":"7"}]}"#)?;
        assert_eq!(sample.sol, 7, "a later key replaces an earlier one, as in a JSON object");
        let dropped: Result<WeatherSample, FetchError> = parse_weather_body(br#"{"soles":[{"sol":"5"}],"soles":"x"}"#);
        assert!(matches!(dropped, Err(FetchError::NoSolData)), "a later non-array soles hides the earlier list: {dropped:?}");
        return Ok(());
    }
}
