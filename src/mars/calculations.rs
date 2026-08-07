
use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use jiff::{SignedDuration, Timestamp, Zoned};
use jiff::tz::TimeZone;

pub fn mars_comms_delay(start_time: Option<Timestamp>) -> f64 {
    // Calculates the speed of light communications delay to Mars
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    let civil: Zoned = dt.to_zoned(TimeZone::UTC);

    let gregorian: Date = Date{year: civil.year(), month: civil.month() as u8,
        decimal_day: civil.day() as f64, cal_type: CalType::Gregorian};
    let julian_day: f64 = julian_day(&gregorian);
    let (_, rad_vec_mars): (_, f64) = geocent_apprnt_ecl_coords(&Planet::Mars, julian_day);

    return rad_vec_mars / SPEED_OF_LIGHT; // Microseconds
}

pub fn mars_comms_return_delay(start_time: Option<Timestamp>) -> f64 { // TODO different return type
    // Calculates the speed of light communications delay to mars
    let dt: Timestamp = start_time.unwrap_or_else(Timestamp::now);

    let microseconds: f64 = mars_comms_delay(None);
    // Return trip from time of initial arrival
    return mars_comms_delay(Some(dt + SignedDuration::from_micros(microseconds as i64))); // TODO apply the microsecond difference to the start time and calculate again
}
