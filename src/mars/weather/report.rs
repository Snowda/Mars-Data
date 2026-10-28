// Writes one weather sample to the log as a readable report.
use tracing::info;

use super::PressureDirection;
use super::WeatherSample;

impl WeatherSample {
    pub fn print_report(&self) -> &Self {
        info!("Date: {:?} (sol {})", self.terrestrial_date, self.sol);
        self.print_temperatures();
        self.print_pressure();
        self.print_atmosphere();
        self.print_sky();
        return self;
    }

    // Air and ground temperature extremes.
    fn print_temperatures(&self) {
        match &self.min_temp {
            Some(temp) => info!("Today's lowest air temperature was {}", temp),
            None => info!("No minimum air temperature data available"),
        }

        match &self.max_temp {
            Some(temp) => info!("Today's highest air temperature was {}", temp),
            None => info!("No maximum air temperature data available"),
        }

        match &self.min_ground_temp {
            Some(temp) => info!("Today's lowest ground temperature was {}", temp),
            None => info!("No minimum ground temperature data available"),
        }

        match &self.max_ground_temp {
            Some(temp) => info!("Today's highest ground temperature was {}", temp),
            None => info!("No maximum ground temperature data available"),
        }
    }

    // Atmospheric pressure trend and solar longitude.
    fn print_pressure(&self) {
        match &self.pressure_change_direction {
            Some(PressureDirection::Rising)  => info!("Atmospheric pressure is rising."),
            Some(PressureDirection::Falling) => info!("Atmospheric pressure is falling."),
            None => info!("No pressure change direction data available"),
        }

        info!("Atmospheric pressure is {:?} and {:?}", self.pressure, self.pressure_change_direction);
        info!("Solar longitude (degrees): {:?}", self.solar_longitude.map(u16::from));
    }

    // Humidity, wind, and UV irradiance.
    fn print_atmosphere(&self) {
        match &self.abs_humidity {
            Some(humidity) => info!("Humidity: {}", humidity),
            None => info!("No Humidity Available"),
        }

        match (&self.wind_speed, &self.wind_direction) {
            (Some(speed), Some(direction)) => info!("The wind is blowing {} at a speed of {} km/s", direction, speed),
            _ => info!("No wind data available"),
        }

        match &self.uv_index {
            Some(uv) => info!("UV irradiance index: {}", uv),
            None => info!("No UV data available"),
        }
    }

    // Sky conditions, season label, and sun times.
    fn print_sky(&self) {
        info!("The weather is {:?}", self.atmo_opacity);
        info!("{:?}", self.season);
        info!("The sun rises at {:?}", self.sunrise);
        info!("The sun sets at {:?}", self.sunset);
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::PressureDirection;
    use crate::mars::weather::tests::full_sample;

    #[test]
    fn print_report_returns_same_instance() {
        let sample = full_sample();
        let returned = sample.print_report();
        assert!(ptr::eq(returned, &sample));
    }

    #[test]
    fn print_report_handles_all_none_optionals() {
        let mut sample = full_sample();
        sample.min_temp = None;
        sample.max_temp = None;
        sample.min_ground_temp = None;
        sample.max_ground_temp = None;
        sample.pressure_change_direction = None;
        sample.abs_humidity = None;
        sample.wind_speed = None;
        sample.wind_direction = None;
        sample.uv_index = None;
        let returned = sample.print_report();
        assert!(ptr::eq(returned, &sample));
    }

    #[test]
    fn print_report_reports_falling_pressure() {
        let mut sample = full_sample();
        sample.pressure_change_direction = Some(PressureDirection::Falling);
        assert!(ptr::eq(sample.print_report(), &sample));
    }

    #[test]
    fn print_report_handles_partial_wind_data() {
        let mut sample = full_sample();
        sample.wind_speed = Some(5.0);
        sample.wind_direction = None;
        assert!(ptr::eq(sample.print_report(), &sample));
    }
}
