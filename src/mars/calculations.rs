
use std::ops::RangeInclusive;

use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use jiff::{SignedDuration, Timestamp, Zoned};
use jiff::tz::TimeZone;

// IAU 2012 astronomical unit (exact): converts the AU radius vector to metres before the c divide.
const AU_IN_METRES: f64 = 149_597_870_700.0;

const SECONDS_PER_DAY: f64 = 86_400.0;
const SECONDS_PER_HOUR: f64 = 3_600.0;
const SECONDS_PER_MINUTE: f64 = 60.0;
const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

// Earth-Mars distance, ~0.372 AU at perihelic opposition to ~2.683 AU at conjunction, widened for the astro approximation.
pub const EARTH_MARS_DISTANCE_AU: RangeInclusive<f64> = 0.35..=2.70;

#[must_use]
pub const fn light_time_secs(distance_au: f64) -> f64 {
    return distance_au * AU_IN_METRES / SPEED_OF_LIGHT;
}

// Every one-way delay mars_comms_delay can return lies in this band.
#[must_use]
pub const fn delay_band_secs() -> RangeInclusive<f64> {
    return light_time_secs(*EARTH_MARS_DISTANCE_AU.start())..=light_time_secs(*EARTH_MARS_DISTANCE_AU.end());
}

pub fn mars_comms_delay(start_time: Option<Timestamp>) -> f64 {
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    let civil: Zoned = dt.to_zoned(TimeZone::UTC);

    // Fold time-of-day into Meeus' decimal day so sub-day resolution is kept, not snapped to midnight.
    let fractional_seconds: f64 = f64::from(civil.hour()) * SECONDS_PER_HOUR
        + f64::from(civil.minute()) * SECONDS_PER_MINUTE
        + f64::from(civil.second())
        + f64::from(civil.subsec_nanosecond()) / NANOS_PER_SECOND;
    let decimal_day: f64 = f64::from(civil.day()) + fractional_seconds / SECONDS_PER_DAY;

    let gregorian: Date = Date{year: civil.year(), month: civil.month().cast_unsigned(),
        decimal_day, cal_type: CalType::Gregorian};
    let julian_day: f64 = julian_day(&gregorian);
    let (_, rad_vec_mars): (_, f64) = geocent_apprnt_ecl_coords(&Planet::Mars, julian_day);

    return light_time_secs(rad_vec_mars);
}

// Float constructor avoids a lossy float->int cast; non-finite/out-of-range input falls back to zero. Extracted to be directly testable.
fn seconds_to_offset(seconds: f64) -> SignedDuration {
    return SignedDuration::try_from_secs_f64(seconds).unwrap_or(SignedDuration::ZERO);
}

pub fn mars_comms_return_delay(start_time: Option<Timestamp>) -> f64 { // TODO different return type
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    // Offset by the one-way delay AT the start time so the return leg is deterministic in dt.
    let one_way_seconds: f64 = mars_comms_delay(Some(dt));
    let return_offset: SignedDuration = seconds_to_offset(one_way_seconds);
    // Return trip delay computed from the time the signal reaches Mars.
    return mars_comms_delay(Some(dt + return_offset));
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    // IAU light-time for one astronomical unit, and how close the AU/c formula must land on it.
    const LIGHT_TIME_FOR_ONE_AU_SECS: f64 = 499.004_783_836;
    const LIGHT_TIME_ABS_TOL_SECS: f64 = 1e-6;

    // Near opposition (2020-10-13) and near conjunction (2021-10-08): Mars distance differs ~6x.
    const OPPOSITION_EPOCH_SECS: i64 = 1_602_547_200;
    const CONJUNCTION_EPOCH_SECS: i64 = 1_633_651_200;
    // Distinct dates must yield distinct distances, proving the timestamp is used.
    const DISTINCT_DATE_MIN_RATIO: f64 = 1.5;
    // Return leg advances the start by the one-way travel time; Mars barely moves, so it stays within this rel tol (~6e-6) but differs.
    const RETURN_VS_ONE_WAY_REL_TOL: f64 = 1e-3;
    // A half-day step within a single calendar day, used to prove the delay tracks time-of-day.
    const HALF_DAY_SECS: i64 = 43_200;

    fn opposition() -> Result<Timestamp, jiff::Error> {
        return Timestamp::from_second(OPPOSITION_EPOCH_SECS);
    }

    fn conjunction() -> Result<Timestamp, jiff::Error> {
        return Timestamp::from_second(CONJUNCTION_EPOCH_SECS);
    }

    fn assert_in_physical_band(delay: f64) {
        let band: RangeInclusive<f64> = delay_band_secs();
        assert!(delay.is_finite(), "delay must be finite, got {delay:e}");
        assert!(band.contains(&delay), "delay {delay:e} outside physical Earth-Mars band {band:?}");
    }

    #[test]
    fn light_time_for_one_au_matches_the_iau_value() {
        let secs: f64 = light_time_secs(1.0);
        assert!(
            (secs - LIGHT_TIME_FOR_ONE_AU_SECS).abs() < LIGHT_TIME_ABS_TOL_SECS,
            "1 AU / c must be the IAU light-time {LIGHT_TIME_FOR_ONE_AU_SECS} s, got {secs}; wrong units on AU or c"
        );
    }

    #[test]
    fn delay_band_is_the_distance_band_in_light_seconds() {
        let band: RangeInclusive<f64> = delay_band_secs();
        assert_eq!(*band.start(), light_time_secs(*EARTH_MARS_DISTANCE_AU.start()));
        assert_eq!(*band.end(), light_time_secs(*EARTH_MARS_DISTANCE_AU.end()));
        assert!(band.start() < band.end(), "opposition must be nearer than conjunction");
    }

    #[test]
    fn delay_is_positive_and_in_physical_band() -> Result<(), jiff::Error> {
        let delay: f64 = mars_comms_delay(Some(opposition()?));

        assert!(delay > 0.0, "delay must be positive, got {delay:e}");
        assert_in_physical_band(delay);
        return Ok(());
    }

    #[test]
    fn delay_is_deterministic_for_fixed_timestamp() -> Result<(), jiff::Error> {
        let first: f64 = mars_comms_delay(Some(opposition()?));
        let second: f64 = mars_comms_delay(Some(opposition()?));

        assert_eq!(first, second, "same timestamp must produce identical delay");
        return Ok(());
    }

    #[test]
    fn delay_varies_with_date() -> Result<(), jiff::Error> {
        // Opposition is nearer, conjunction farther; equal values would mean the timestamp is ignored.
        let near: f64 = mars_comms_delay(Some(opposition()?));
        let far: f64 = mars_comms_delay(Some(conjunction()?));

        assert!(
            far > near * DISTINCT_DATE_MIN_RATIO,
            "conjunction delay {far:e} should exceed opposition delay {near:e} by >{DISTINCT_DATE_MIN_RATIO}x"
        );
        assert_in_physical_band(near);
        assert_in_physical_band(far);
        return Ok(());
    }

    #[test]
    fn delay_none_defaults_to_now_and_stays_in_band() {
        // Covers the `unwrap_or_else(Timestamp::now)` branch; "now" is real geometry, still in band.
        let delay: f64 = mars_comms_delay(None);

        assert!(delay > 0.0, "delay must be positive, got {delay:e}");
        assert_in_physical_band(delay);
    }

    #[test]
    fn return_delay_is_positive_and_in_physical_band() -> Result<(), jiff::Error> {
        let delay: f64 = mars_comms_return_delay(Some(opposition()?));

        assert!(delay > 0.0, "return delay must be positive, got {delay:e}");
        assert_in_physical_band(delay);
        return Ok(());
    }

    #[test]
    fn return_delay_is_close_to_but_distinct_from_one_way() -> Result<(), jiff::Error> {
        // Return leg advances start by the one-way time before recomputing: close but not identical; equality would mean the offset was discarded.
        let one_way: f64 = mars_comms_delay(Some(opposition()?));
        let round: f64 = mars_comms_return_delay(Some(opposition()?));

        assert_ne!(round, one_way, "return delay must reflect the applied one-way offset");
        let rel_diff: f64 = (round - one_way).abs() / one_way;
        assert!(
            rel_diff < RETURN_VS_ONE_WAY_REL_TOL,
            "return delay {round} should match one-way {one_way} within {RETURN_VS_ONE_WAY_REL_TOL} (rel diff {rel_diff:e})"
        );
        return Ok(());
    }

    #[test]
    fn delay_tracks_time_of_day_within_a_calendar_day() -> Result<(), jiff::Error> {
        // Midnight vs noon on the same day must differ; equal would mean time-of-day was truncated.
        let midnight: Timestamp = Timestamp::from_second(OPPOSITION_EPOCH_SECS)?;
        let noon: Timestamp = Timestamp::from_second(OPPOSITION_EPOCH_SECS + HALF_DAY_SECS)?;

        let at_midnight: f64 = mars_comms_delay(Some(midnight));
        let at_noon: f64 = mars_comms_delay(Some(noon));

        assert_ne!(at_midnight, at_noon, "delay must change with time-of-day within a day");
        assert_in_physical_band(at_midnight);
        assert_in_physical_band(at_noon);
        return Ok(());
    }

    #[test]
    fn return_delay_is_deterministic_for_fixed_timestamp() -> Result<(), jiff::Error> {
        let first: f64 = mars_comms_return_delay(Some(opposition()?));
        let second: f64 = mars_comms_return_delay(Some(opposition()?));

        assert_eq!(first, second, "same timestamp must produce identical return delay");
        return Ok(());
    }

    #[test]
    fn return_delay_varies_with_date() -> Result<(), jiff::Error> {
        let near: f64 = mars_comms_return_delay(Some(opposition()?));
        let far: f64 = mars_comms_return_delay(Some(conjunction()?));

        assert!(
            far > near * DISTINCT_DATE_MIN_RATIO,
            "conjunction return delay {far:e} should exceed opposition {near:e} by >{DISTINCT_DATE_MIN_RATIO}x"
        );
        return Ok(());
    }

    #[test]
    fn return_delay_none_defaults_to_now_and_stays_in_band() {
        let delay: f64 = mars_comms_return_delay(None);

        assert!(delay > 0.0, "return delay must be positive, got {delay:e}");
        assert_in_physical_band(delay);
    }

    // 0.5 s expressed in nanoseconds; the fractional part of a 208.5 s offset.
    const HALF_SECOND_NANOS: i32 = 500_000_000;
    // Beyond i64::MAX seconds, so try_from_secs_f64 reports out-of-range.
    const OUT_OF_RANGE_SECONDS: f64 = 1e300;

    #[test]
    fn seconds_to_offset_maps_finite_positive() {
        let offset: SignedDuration = seconds_to_offset(208.5);

        assert_eq!(offset.as_secs(), 208, "whole seconds preserved");
        assert_eq!(offset.subsec_nanos(), HALF_SECOND_NANOS, "fractional seconds preserved");
    }

    #[test]
    fn seconds_to_offset_maps_finite_negative() {
        // Negative finite values are in range, so they convert rather than hitting the fallback.
        let offset: SignedDuration = seconds_to_offset(-3.0);

        assert_eq!(offset.as_secs(), -3, "negative finite input converts, not falls back");
    }

    #[test]
    fn seconds_to_offset_nan_falls_back_to_zero() {
        assert_eq!(seconds_to_offset(f64::NAN), SignedDuration::ZERO);
    }

    #[test]
    fn seconds_to_offset_infinities_fall_back_to_zero() {
        assert_eq!(seconds_to_offset(f64::INFINITY), SignedDuration::ZERO);
        assert_eq!(seconds_to_offset(f64::NEG_INFINITY), SignedDuration::ZERO);
    }

    #[test]
    fn seconds_to_offset_out_of_range_falls_back_to_zero() {
        assert_eq!(seconds_to_offset(OUT_OF_RANGE_SECONDS), SignedDuration::ZERO);
        assert_eq!(seconds_to_offset(-OUT_OF_RANGE_SECONDS), SignedDuration::ZERO);
    }
}
