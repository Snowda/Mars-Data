
use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use chrono::{DateTime, Datelike, Utc};
use chrono::Duration as ChronoDuration;

pub fn mars_comms_delay(start_time: Option<DateTime<Utc>>) -> f64 {
    // Calculates the speed of light communications delay to Mars
    let dt: DateTime<Utc> = start_time.unwrap_or_else(|| Utc::now());

    let (_is_common_era, year): (bool, u32) = dt.year_ce(); // TODO use common era?

    let gregorian: Date = Date{year: year.try_into().unwrap(), month: dt.month().try_into().unwrap(),
        decimal_day: dt.day().try_into().unwrap(), cal_type: CalType::Gregorian};
    let julian_day: f64 = julian_day(&gregorian);
    let (_, rad_vec_mars): (_, f64) = geocent_apprnt_ecl_coords(&Planet::Mars, julian_day);

    return rad_vec_mars / SPEED_OF_LIGHT; // Microseconds
}

pub fn mars_comms_return_delay(start_time: Option<DateTime<Utc>>) -> f64 { // TODO different return type
    // Calculates the speed of light communications delay to mars
    let dt: DateTime<Utc> = start_time.unwrap_or_else(|| Utc::now());

    let microseconds: f64 = mars_comms_delay(None);
    // Return trip from time of initial arrival
    return mars_comms_delay(Some(dt + ChronoDuration::microseconds(microseconds as i64))); // TODO apply the microsecond difference to the start time and calculate again
}
