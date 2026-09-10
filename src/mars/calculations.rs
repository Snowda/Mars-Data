
use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use jiff::{SignedDuration, Timestamp, Zoned};
use jiff::tz::TimeZone;

const MICROS_PER_SEC: f64 = 1_000_000.0;

pub fn mars_comms_delay(start_time: Option<Timestamp>) -> f64 {
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    let civil: Zoned = dt.to_zoned(TimeZone::UTC);

    let gregorian: Date = Date{year: civil.year(), month: civil.month().cast_unsigned(),
        decimal_day: f64::from(civil.day()), cal_type: CalType::Gregorian};
    let julian_day: f64 = julian_day(&gregorian);
    let (_, rad_vec_mars): (_, f64) = geocent_apprnt_ecl_coords(&Planet::Mars, julian_day);

    return rad_vec_mars / SPEED_OF_LIGHT; // Microseconds
}

pub fn mars_comms_return_delay(start_time: Option<Timestamp>) -> f64 { // TODO different return type
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    let microseconds: f64 = mars_comms_delay(None);
    // Convert the one-way delay to a duration via the float constructor so there is no
    // lossy float->int cast; a non-finite value falls back to zero rather than panicking.
    let return_offset: SignedDuration =
        SignedDuration::try_from_secs_f64(microseconds / MICROS_PER_SEC).unwrap_or(SignedDuration::ZERO);
    // Return trip from time of initial arrival
    return mars_comms_delay(Some(dt + return_offset)); // TODO apply the microsecond difference to the start time and calculate again
}
