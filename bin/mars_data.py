"""Combines of mars_weather and communications_delay_to_mars"""
#!/usr/bin/env python
# coding: utf-8
import re
from datetime import datetime, timedelta
from json import loads
from requests import get
from scipy.constants import c as speed_of_light
from astropy.time import Time
from astropy.coordinates import solar_system_ephemeris, EarthLocation, get_body

WEATHER_URL = "http://marsweather.ingenology.com/v1/latest/"

def mars_comms_delay():
    """ Calculates the speed of light communications delay to mars"""
    time_now = Time(datetime.now().strftime("%Y-%m-%d %H:%M"))
    reference_location = EarthLocation.of_site('greenwich')  # TODO get users location
    with solar_system_ephemeris.set('de432s'):
        mars = get_body('mars', time_now, reference_location)
        earth = get_body('earth', time_now, reference_location)
        microseconds = earth.separation_3d(mars).value / speed_of_light
        return timedelta(microseconds=int(microseconds))
    return None

def mars_comms_return_delay(start_time=None):
    """ Calculates the speed of light communications delay to mars"""
    if start_time is None:
        start = datetime.now()
    else:
        start = start_time
    time_now = Time(start.strftime("%Y-%m-%d %H:%M"))
    reference_location = EarthLocation.of_site('greenwich')  # TODO get users location
    with solar_system_ephemeris.set('de432s'):
        mars = get_body('mars', time_now, reference_location)
        earth = get_body('earth', time_now, reference_location)

        microseconds = earth.separation_3d(mars).value / speed_of_light
        bounce = timedelta(microseconds=int(microseconds))
        bounce_time = Time((start+bounce).strftime("%Y-%m-%d %H:%M"))  # Start of return trip
        mars = get_body('mars', bounce_time, reference_location)
        earth = get_body('earth', bounce_time, reference_location)
        result = earth.separation_3d(mars).value / speed_of_light
        return timedelta(microseconds=int(microseconds+result))

    return None

def display_data():
    """Displays the data"""
    connection = get(WEATHER_URL, timeout=2)
    if connection.status_code != 200:
        print(connection.status_code, connection.reason)
        return

    report = loads(connection.content)['report']

    terrestrial_date = report['terrestrial_date']
    sol = report['sol']
    min_temp = report['min_temp']
    max_temp = report['max_temp']
    min_temp_fahrenheit = report['min_temp_fahrenheit']
    max_temp_fahrenheit = report['max_temp_fahrenheit']
    pressure = report['pressure']
    pressure_string = report['pressure_string']
    mars_season = report['ls']
    abs_humidity = report['abs_humidity']
    wind_speed = report['wind_speed']
    wind_direction = report['wind_direction']
    atmo_opacity = report['atmo_opacity']
    season = report['season']
    sunrise = report['sunrise']
    sunset = report['sunset']

    print("Date :" + str(terrestrial_date) + " (sol " + str(sol) + ")")

    print("Today's lowest temperature was %s C" % (str(min_temp)))
    print("Today's highest temperature was %s C" % (str(min_temp)))

    if str(pressure_string) == "Higher":
        atmo_status = "rising"
    else:
        atmo_status = "falling"
    print("Atmospheric pressure is " + str(pressure) + " and " + str(atmo_status))
    print("Mars season : %s" % (str(mars_season)))
    if str(abs_humidity) != "None":
        print("Humidity " + str(abs_humidity))
    if str(wind_speed) != "None":
        print("The wind is blowing" + str(wind_direction) + " at a speed of " 
              + str(wind_speed) + "km/s")
    print("The weather is " + str(atmo_opacity))
    print(season)
    print("The sun rises at " + str(sunrise))
    print("The sun sets at " + str(sunset))

if __name__ == '__main__':
    print(mars_comms_delay())
    print(mars_comms_return_delay())
    display_data()
