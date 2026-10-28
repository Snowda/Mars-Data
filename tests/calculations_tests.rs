use std::ops::RangeInclusive;

use jiff::Timestamp;
use mars_data::mars::calculations::{delay_band_secs, mars_comms_delay, mars_comms_return_delay};

// The recomputed return leg can differ slightly from the outbound leg as Mars moves, but
// stays within twice the one-way delay.
const RETURN_DELAY_TOLERANCE: f64 = 2.1;

fn assert_in_one_way_band(delay: f64) {
    let band: RangeInclusive<f64> = delay_band_secs();
    assert!(delay.is_finite(), "delay {delay} must be finite");
    assert!(band.contains(&delay), "one-way delay {delay} s outside physical band {band:?} s");
}

#[test]
fn test_mars_comms_delay_fixed_date() -> Result<(), jiff::Error> {
    // Using a fixed date for deterministic testing.
    let test_date: Timestamp = "2025-09-10T12:00:00Z".parse()?;
    let delay = mars_comms_delay(Some(test_date));

    assert!(delay > 0.0, "Delay should be positive");
    assert_in_one_way_band(delay);
    return Ok(());
}

#[test]
fn test_mars_comms_delay_none_param() {
    // Test with None parameter, which should use current time.
    let delay = mars_comms_delay(None);

    assert!(delay > 0.0, "Delay should be positive");
    assert_in_one_way_band(delay);
}

#[test]
fn test_mars_comms_return_delay() -> Result<(), jiff::Error> {
    // Using a fixed date for deterministic testing.
    let test_date: Timestamp = "2025-09-10T12:00:00Z".parse()?;
    let delay = mars_comms_return_delay(Some(test_date));

    // The return delay should be at least as large as the one-way delay.
    let one_way_delay = mars_comms_delay(Some(test_date));
    assert!(delay >= one_way_delay, "Return delay should be at least as large as one-way delay");
    assert_in_one_way_band(delay);
    return Ok(());
}

#[test]
fn test_mars_comms_return_delay_none_param() {
    // Test with None parameter, which should use current time.
    let delay = mars_comms_return_delay(None);

    assert!(delay > 0.0, "Return delay should be positive");
    assert_in_one_way_band(delay);
}

#[test]
fn test_relation_between_comms_delay_and_return_delay() -> Result<(), jiff::Error> {
    // Using a fixed date for deterministic testing.
    let test_date: Timestamp = "2025-09-10T12:00:00Z".parse()?;

    let one_way_delay = mars_comms_delay(Some(test_date));
    let return_delay = mars_comms_return_delay(Some(test_date));

    // The return delay should be greater than or equal to the one-way delay but no more than
    // twice it (it would be exactly twice if Mars's position relative to Earth never changed).
    assert!(return_delay >= one_way_delay && return_delay <= RETURN_DELAY_TOLERANCE * one_way_delay,
            "Return delay {} should be between one-way delay {} and twice that value {}",
            return_delay, one_way_delay, RETURN_DELAY_TOLERANCE * one_way_delay);
    return Ok(());
}
