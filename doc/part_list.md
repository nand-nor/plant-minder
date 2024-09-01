# Parts List

## Host Controller & Radio
- A Raspberry PI is the intended target for the broker logic, and can optionally run front end logic like the rendered TUI and database. Tested on a few including rpi4 and rpi5
  - [rpi4](https://www.adafruit.com/product/4564), 
  - [rpi5](https://www.adafruit.com/product/5813). 
 
- Currently using the rpi5 with 8Gb RAM which works great. The pi must be set up to run the `openthread` stack & `otbr-agent` [follow instructions here](https://openthread.io/guides/build) and configured as a host for running openthread in RCP mode.
- Pis dont have built-in support (yet?) so you will need an 802.15.4-capable radio (external dongle) configured to run as an RCP (radio co-processor) on the pi. Tested with the following RCP dongles 
  - [nRF (nRF52840)](https://openthread.io/vendors/nordic-semiconductor)
  - [silabs EFR32 (MG21)](https://openthread.io/vendors/silicon-labs)
- Others exist (for example the esp32h2 adn esp32c6 can both run RCP mode) anything that is 1.3 ceritifed should work. See the openthread list of [supported platforms here](https://openthread.io/platforms).

## ESP32 sensor nodes

- [esp32-c6 with 4MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-C6-DEVKITM-1-N4/18667011)
- [esp32-c6 with 8MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-C6-DEVKITC-1-N8/17728861)
- [esp32-h2 with 4MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-H2-DEVKITM-1-N4/18109238)

- Other dev board options exist with these chips, for example if you want something with QWIIC (however the code in `pmindp-esp32-thread` is __not__ compatible):
  - [sparkfun thing plus with esp32c6](https://www.digikey.com/en/products/detail/sparkfun-electronics/DEV-22924/22321033) and 
  - [adafruit feather with esp32c6](https://www.adafruit.com/product/5933) 

*Note:* As of this writing I have only tested h2s with 4MB and c6s with 8MB flash

## Sensors 

### Soil Sensors
- [Seesaw Capacitive moisture sensor](https://www.adafruit.com/product/4026)

Also will be testing / developing code for additional sensors eventually:
- [Sunfounder capacitive moister sensor](https://www.digikey.com/en/products/detail/sunfounder/ST0160/22116813) 
- [SparkFun soil moisture sensor](https://www.digikey.com/en/products/detail/sparkfun-electronics/SEN-13322/5764506)

### Other sensors
- [Adafruit TSL2591 light sensor](https://www.adafruit.com/product/1980)
- [Adafruit BME680 humidity/gas/pressure sensor](https://www.adafruit.com/product/3660)

