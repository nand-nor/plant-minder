# Parts List

## Controller:
- RPI (I am using a 4 with 8GB RAM)

## Soil Sensors: 
- [Seesaw Capacitive moisture sensor](https://www.adafruit.com/product/4026)

Also will be testing / developing code for additional sensors eventually:
- [Sunfounder capacitive moister sensor](https://www.digikey.com/en/products/detail/sunfounder/ST0160/22116813) 
- [SparkFun soil moisture sensor](https://www.digikey.com/en/products/detail/sparkfun-electronics/SEN-13322/5764506)


## Wireless builds:

### Controller
- pi must be set up to run the openthread stack [follow instructions here](https://openthread.io/guides/build)
- You will also need an 802.15.4 radio dongle configured to run as an RCP--build OT with:
```
-DOT_APP_RCP=ON
-DOT_RCP=ON
```
See build instructions above. Tested to work with the following RCP dongles (others are also possible, in theory anything that is 1.3 ceritifed should work)
- nRF (nRF52840 etc.)
- silabs EFR32 (MG12 / MG13 / MG21)

### Remote sensor nodes:

- [esp32-c6 with 4MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-C6-DEVKITM-1-N4/18667011)
- [esp32-c6 with 8MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-C6-DEVKITC-1-N8/17728861)
- [esp32-h2 with 4MB flash](https://www.digikey.com/en/products/detail/espressif-systems/ESP32-H2-DEVKITM-1-N4/18109238)

*Note:* As of this writing I have only tested with c6 boards with 8 MB flash. As noted eslewhere the ultimate goal is to configure these devices to act as router nodes so likely more flash is better for such purposes 

Other dev board options exist with these chips, for example if you want something with QWIIC:
[sparkfun thing plus (esp32-c6)](https://www.digikey.com/en/products/detail/sparkfun-electronics/DEV-22924/22321033)

### Wired builds:
- [TCA9548A i2c expander](https://www.adafruit.com/product/2717)
- [Perma-proto hat](https://www.adafruit.com/product/2310)
- Also needs some minimal soldering, jumper wires, and some spare headers for plug-n-play set up with the perma proto hat but you can solder / make it permanent or use breadboards its all optional  

