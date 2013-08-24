"""
Combination of mars_weather.py and communications_delay_to_mars.py
"""
import ephem, urllib, urllib2, json, re, pygeoip

def comms_delay_to_mars():
    """
    Calculates the communications time delay to mars due to the speed of light
    """
    mars_ephem = ephem.Mars()
    mars_ephem.compute()

    time_seconds = mars_ephem.earth_distance * ephem.meters_per_au / ephem.c
    time_minutes = time_seconds / 60
    time_remaining = time_seconds % 60

    if(str(time_minutes) == "1") :
        minute_string = "minute"
    else :
        minute_string = "minutes"
    
    if(str(time_remaining) == "1" ) :
        second_string = "second"
    else :
        second_string = "seconds"

    print("There is a %.0f %s %.3f %s communication delay to Mars." 
        % (time_minutes, minute_string, time_remaining, second_string))

def internet_on():
    """Checks if the internet connetion is working"""
    return target_online('http://www.google.com') #google is always online

def target_online(url_to_check, return_string=False):
    """Checks if the supplied URL is online"""
    try:
        connection = urllib2.urlopen(url_to_check, timeout=2).read()
    except urllib2.URLError:
        if(return_string):
            if internet_on():
                print "Connetion to target timed out. Try again."
                return False
            else :
                print "No internet connection. Check your connectivity."
                return False
        else:   
            return False
    else:
        if(return_string):
            return connection
        else:
            return True

def get_country_name():
    """
    Using IP lookup tables, checks user's location. Won't work behind proxy
    """
    ip_check_url = 'http://checkip.dyndns.org'
    if(target_online(ip_check_url)):
        response = re.search(re.compile(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"), 
            urllib.urlopen(ip_check_url).read()).group()
        geo_ip = pygeoip.GeoIP('../data/GeoIP.dat')
        return geo_ip.country_name_by_name(response) #country_code_by_name
    else :
        return "URL Check IP offline"

def display_data():
    """Displays the data"""
    weather_url = "http://marsweather.ingenology.com/v1/latest/"
    connection = target_online(weather_url, return_string=True)
    if connection:
        overview = json.loads(connection)
        report = overview['report']

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

        print "date :"+str(terrestrial_date)+" (sol "+str(sol)+")"

        place = get_country_name()

        if(place == "United States" or place == "Belize" 
            or place == "Bermuda" or place == "Palau"):
            print "The lowest temperature today was: " \
                +str(min_temp_fahrenheit)+" F ("+str(min_temp)+" C)"
            print "The highest temperature today was: " \
                +str(max_temp_fahrenheit)+" F ("+str(max_temp)+" C)"
        else :
            print "The lowest temperature today was: "+str(min_temp)+" C"
            print "The highest temperature today was: "+str(max_temp)+" C"

        if(str(pressure_string)=="Higher") :
            atmo_status = "rising"
        else :
            atmo_status = "falling"
        print "Atmospheric pressure is "+str(pressure)+" and "+str(atmo_status)
        print "Mars season : "+str(mars_season)
        if(str(abs_humidity)!="None"):
            print "Humidity "+str(abs_humidity)
        if(str(wind_speed) != "None") :
            print "The wind is blowing"+str(wind_direction) \
                +" at a speed of "+str(wind_speed)+"km/s"
        print "The weather is "+str(atmo_opacity)
        print season
        print "The sun rises at "+str(sunrise)
        print "The sun sets at "+str(sunset)
    else:
        if internet_on():
            print "Mars weather API offline"
        else:
            print "No internet connection"
    connection..close()  # best practice to close the file


def main():
    """This is the main"""
    comms_delay_to_mars()
    display_data()

if __name__ == '__main__':
    main()
