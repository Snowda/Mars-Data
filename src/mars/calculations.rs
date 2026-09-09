
use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use jiff::{SignedDuration, Timestamp, Zoned};
use jiff::tz::TimeZone;

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
    // Fractional microseconds are below SignedDuration's resolution, so truncating to i64 is intended.
    #[allow(clippy::cast_possible_truncation)]
    let micros_delay: i64 = microseconds as i64;
    // Return trip from time of initial arrival
    return mars_comms_delay(Some(dt + SignedDuration::from_micros(micros_delay))); // TODO apply the microsecond difference to the start time and calculate again
}
