"""Combination of mars_weather.py and communications_delay_to_mars.py"""
import re
from datetime import datetime
from json import loads
from requests import get
from pygeoip import GeoIP
from scipy.constants import c as speed_of_light
from astropy.time import Time
from astropy.coordinates import solar_system_ephemeris, EarthLocation, get_body

WEATHER_URL = "http://marsweather.ingenology.com/v1/latest/"

def comms_delay_to_mars():
    """ Calculates the speed of light communications delay to mars"""
    time_now = Time(datetime.now().strftime("%Y-%m-%d %H:%M"))
    reference_location = EarthLocation.of_site('greenwich') # TODO get users location
    with solar_system_ephemeris.set('de432s'):
        mars = get_body('mars', time_now, reference_location)
        earth = get_body('earth', time_now, reference_location)
        time_seconds = earth.separation_3d(mars).value * 1000 / speed_of_light
        time_minutes = time_seconds / 60
        time_remaining = time_seconds % 60

        if str(time_minutes) == "1":
            minute_string = "minute"
        else:
            minute_string = "minutes"

        if str(time_remaining) == "1":
            second_string = "second"
        else:
            second_string = "seconds"

        print("There is a %.0f %s %.3f %s communication delay to Mars."
              % (time_minutes, minute_string, time_remaining, second_string))

def is_internet_on():
    """Checks if the internet connetion is working"""
    connection = get('http://www.google.com', timeout=2) #google is always online
    if connection.status_code == 200:
        return True

    print(connection.status_code, connection.reason)
    return False

def get_country_name():
    """Using IP lookup tables, checks user's location. Won't work behind proxy."""
    url_data = get('http://checkip.dyndns.org', timeout=2)
    if url_data == 200:
        response = re.search(re.compile(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"),
                             url_data.content).group()
        geo_ip = GeoIP('../data/GeoIP.dat')
        return geo_ip.country_name_by_name(response) #country_code_by_name

    print(url_data.status_code, url_data.reason)
    return None

def display_data():
    """Displays the data"""
    connection = get(WEATHER_URL, timeout=2)
    if connection.status_code == 200:
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

        if get_country_name() in ("United States", "Belize", "Bermuda", "Palau"):
            print("The lowest temperature today was: " \
                + str(min_temp_fahrenheit) + " F (" + str(min_temp) + " C)")
            print("The highest temperature today was: %s F (%s C)" % (str(max_temp_fahrenheit),
                                                                      str(max_temp)))
        else:
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
            print("The wind is blowing" + str(wind_direction) + " at a speed of " \
                + str(wind_speed) + "km/s")
        print("The weather is " + str(atmo_opacity))
        print(season)
        print("The sun rises at " + str(sunrise))
        print("The sun sets at " + str(sunset))
    else:
        if is_internet_on():
            print("Mars weather API offline")
        else:
            print("No internet connection")
    connection.close()  # best practice to close the file

if __name__ == '__main__':
    comms_delay_to_mars()
    display_data()
