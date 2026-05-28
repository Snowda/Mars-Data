# Mars Data

> Parses & combines Martian data sources

![Linux Build Status](https://github.com/Snowda/Mars-Data/workflows/Linux/badge.svg)
![License](https://img.shields.io/github/license/Snowda/Mars-Data)

Mars Data is a Rust command-line tool and library that:

* Fetches **live surface weather** from NASA's Curiosity rover (REMS instrument) and prints a per-sol report.
* Calculates the **speed-of-light communications delay** between Earth and Mars (one-way and round-trip) for the current moment.

The weather data is read from the public Mars Science Laboratory feed, so no API key or configuration is required.

## Requirements

Before you begin, ensure you have met the following requirements:

* Rust 1.96 or newer (latest stable recommended) – [How to install Rust](https://www.rust-lang.org/en-US/install.html)
* An internet connection (the weather report fetches live data)

## Installation

```sh
git clone https://github.com/Snowda/Mars-Data.git
cd Mars-Data
cargo build --release
```

## Usage

Run it:

```sh
cargo run
```

Example of what it reports:

* Earth–Mars communications delay (one-way and round-trip)
* Terrestrial date and Martian sol
* Minimum / maximum air and ground temperature (°C)
* Atmospheric pressure and its trend (rising / falling)
* Absolute humidity, wind speed and direction
* UV irradiance index and atmospheric opacity
* Martian season, sunrise and sunset

Run the test suite and linter:

```sh
cargo test
cargo cranky
```

## Data parsed

The latest sol is deserialized into a strongly-typed `WeatherSample`, with enums for wind
direction, atmospheric opacity, UV index, and pressure trend, and `chrono` types for dates
and times. Missing values (the feed's `"--"` sentinel) are represented as `None`.

## Roadmap

- [x] Source Migration
	- [x] Migrate off the retired InSight feed to the live Curiosity (MSL) feed
	- [ ] Add the PDS archive as a source — https://atmos.nmsu.edu/PDS/data/mslrem_1001/
- [ ] Data Engineering
	- [ ] Create a JSON output
	- [ ] API output
	- [ ] Database Integration
- [ ] Frontend
	- [ ] Replace console output
	- [ ] Website frontend
- [ ] Data Story
	- [ ] Add satellite imagery

## Data Sources

1. Mars Science Laboratory – Curiosity (REMS) – live feed: `https://mars.nasa.gov/rss/api/?feed=weather&category=msl&feedtype=json`
2. Mars Science Laboratory – Curiosity – [PDS archive](https://pds-atmospheres.nmsu.edu/data_and_services/atmospheres_data/MARS/mars_lander.html) (planned)

## Report Issues

Found a bug or have a question? [Open an issue](https://github.com/Snowda/Mars-Data/issues/new).

## Contribute

**Pull requests are warmly welcome!!!**

For major changes, please [open an issue](https://github.com/Snowda/Mars-Data/issues/new) first and let's talk about it. We are all ears!
