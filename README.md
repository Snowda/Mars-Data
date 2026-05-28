# Mars Data

> Parses & combines Martian data sources

![Linux Build Status](https://github.com/Snowda/Mars-Data/workflows/Linux/badge.svg)
![License](https://img.shields.io/github/license/Snowda/Mars-Data)

Mars Data is a Rust command-line tool and library that:

* Fetches **live surface weather** from NASA's Curiosity rover (REMS instrument) and prints a per-sol report.
* Calculates the **speed-of-light communications delay** between Earth and Mars (one-way and round-trip) for the current moment.

The weather data is read from the public Mars Science Laboratory feed, so no API key or configuration is required.

## Why

There's weather on another planet right now — frost, wind, dust, a sunrise over a cold
red desert 200 million kilometres away — and the readings are public, free for anyone
to pull down.

But the raw feed makes that harder than it should be: numbers arrive as strings, missing
readings hide behind a `"--"` sentinel, nothing is typed. Mars Data clears that away and
hands you clean, typed readings — so you can stop wrangling JSON and just watch the
weather on Mars.

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

Print the latest sol as JSON instead of a console report:

```sh
cargo run -- --json
```

This prints the latest sol as a typed JSON document. Numeric fields are real numbers
(not strings), categorical fields are enums, and missing readings are simply omitted:

```json
{
  "terrestrial_date": "2025-09-10",
  "sol": 100,
  "min_temp": -80.0,
  "max_temp": -10.0,
  "min_ground_temp": -96.0,
  "max_ground_temp": 10.0,
  "pressure": 750.0,
  "pressure_change_direction": "Rising",
  "mars_season": 5,
  "abs_humidity": 0.3,
  "wind_speed": 12.0,
  "wind_direction": "NW",
  "atmo_opacity": "Sunny",
  "uv_index": "Moderate",
  "season": "month 5",
  "sunrise": "06:00:00",
  "sunset": "18:00:00"
}
```

Serve the weather as an HTTP API:

```sh
cargo run -- serve
```

This starts a server (default `0.0.0.0:3000`, configurable via `host` and `port`
in `config.toml`) that refreshes the latest sol hourly:

* `GET /health` — liveness check (returns `200 OK`)
* `GET /weather` — latest sol as JSON (returns `503` until the first fetch completes)

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

## Limitations

* **Live feed only.** Every run fetches the current sol from NASA over the network — there
  is no offline cache or bundled data, so the tool needs an internet connection to do anything useful.
* **Single rover.** Only Curiosity (MSL/REMS) is supported. The historical PDS archive and
  other missions are on the [roadmap](#roadmap), not yet implemented.
* **Latest sol only.** Reports cover the most recent published sol; there is no querying of
  past sols or date ranges.
* **Server freshness is hourly.** The `serve` mode refreshes once an hour and returns `503`
  on `/weather` until the first fetch completes.
* **Upstream-dependent.** Field availability and naming follow NASA's feed; if the feed
  changes shape or goes down, affected fields surface as `None` rather than an error.

## Roadmap

- [x] Source Migration
	- [x] Migrate off the retired InSight feed to the live Curiosity (MSL) feed
	- [ ] Add the PDS archive as a source — https://atmos.nmsu.edu/PDS/data/mslrem_1001/
- [ ] Data Engineering
	- [x] Create a JSON output
	- [x] API output
	- [ ] Database Integration
	- [ ] Forecast upcoming weather from historical sols
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
