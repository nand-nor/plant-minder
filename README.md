# Plant-Minder (WIP)
RPI4 + soil sensors to track when my plants need watering. This repo contains (mostly) all needed code for deployiong a simple plant monitoring system, using i2c soil sensors. The soil sensors can be controlled locally or remotely. Remote control is achieved via ESP32 microcontrollers running openthread, and local/wired control is done via i2c bus.  

## Description

The Plant-Minder monitoring system uses a distributed system of microcontrollers to control and report sensor data. Microcontrollers sense and report soil moisture data voa a wireless mesh protocol, which is received by a raspberry pi. The pi has logic to determine soil conditions / trends and will ultimately alert me with a big obvious visual display whenever I need to water my plants.

```
         _            ________________________
        |            |                        |
        |            |      TUI Front End     |
        |            |________________________|
        |                ^               |
    RPI |                | Events        | Subscribe 
        |             ___|_______________v____          __________
        |            |                        |        |          |
        |            |     Broker / Backend   | <----> | Database |
        |            |________________________|        |__________|
        |                ^            |
        |             ___|____________v_______
        |            |                        |
        |            |       openthread       |
        |_           |________________________|
                       ^          ^          ^             
                       | 802.15.4 |          |      
                       |          |          |                
                     __|___    ___|___    ___|___ 
                    | ESP32|  | ESP32 |  | ESP32 |
                     ------    -------    ------- 
                       ^          ^          ^
                       | i2c      |          |
                    ___v____   ___v____   ___v____    
                   | Sensor | | Sensor | | Sensor | 
                    --------   --------   --------  
```

### Sensor control & reporting via esp32 to RPI

Remote microcontrollers are used to control and report sensor data to the RPI at configurable intervals via thread, a wireless mesh protocol that runs on top of 802.15.4. 

This is done via esp32 dev boards that are configured to control the soil sensor as an i2c device as well as run the openthread stack. Sensed soil data is reported via openthread, which provides the transport layer for reporting sensor data to the RPI controller.

Remote sensor/esp32 dev boards use `esp-hal` and `openthread` (via `esp-openthread` repo) to run bare metal as a minimal thread device (MTD). 

The `pmindp-esp32-thread` package contains all the code for building & flashing the esp32 dev boards with sensor devices, which only supports by espressif boards that have an 802.15.4 native radio (so currently on esp32-c6 and esp32-h2). There is no support for NCP or RCP modes in the `esp-openthread` repo. 

Also note that to support this deployment mode, the RPI must be configured to run the openthread stack with an RCP radio. The plant-minder system currently assumes that the RPI is acting as a border agent but future iterations may change this (there is no real requirement currently for bidirectional IP connectivity). 

For the soil sensor, the code currently only supports [Seesaw Capacitive moisture sensor (ATSAMD10)](https://www.adafruit.com/product/4026). Although I do have some plans to eventually  support other sensors (both different soil sensors and other sensor types like humidity / light/etc.)

![esp32-c6 controller with sensor on pins 5 & 6](./doc/sensor_esp32c6.jpg)
![esp32-c6 running on battery](./doc/battery.jpg)

## Status

In general I would estimate this is roughly at 35% complete. Lots of work is still needed. But basic sensor control / running openthread on the esp32 devices, and receiving reported sensor data on the pi is working.

### Sensor layer
- Base i2c control for ATSAMD10 chip ([seesaw soil sensor](https://www.adafruit.com/product/4026)) in `pmindp-sensor`
- simple wired control example running on pi with TCA9548A i2c expander in `pmindd` (build the `plant-minder-wired` bin)
- Full build for wireless control via openthread is (mostly) finished, everything needed for programming esp32-c6 or esp32-h2 boards is in `pmindp-esp32-thread` 

A few other pieces are still needed here & this is under active development.  


#### Future plans for sensor layer

One major goal is more complex OT device type support for remote sensor controllers. The `esp-openthread` repo currently only supports running esp32 boards as MTDs. Work is ongoing to add support for running as both FTDs and as SED/SSEDs

Another goal is to eventually support other moisture sensors
- [Sunfounder capacitive moister sensor](https://www.digikey.com/en/products/detail/sunfounder/ST0160/22116813) 
- [SparkFun soil moisture sensor](https://www.digikey.com/en/products/detail/sparkfun-electronics/SEN-13322/5764506)

Additional sensor types will also eventually be added, targeting humidity co2 and light sensors.

### Broker layer
Under active development 

### Front end / TUI rendering layer
Not yet started

## Less fun build configuration: wired sensor builds with fully local sensor control & reporting on RPI (via TCA9548A) 

There is also the option to run things wired where the RPI locally controls wired soil sensors via i2c mux. No need to set up a Thread mesh for this (but it is also less interesting so will be supported at lowest priority)

There is a simple wired control example that is capable of running on pi with TCA9548A i2c expander in `pmindd` package (build the `plant-minder-wired` bin). 

Build supports up to 8 sensors that can all have the same unchangeable i2c address. Note you do not need the muxer if your sensors have configurable / changeable i2c addresses. The soil sensors I have been working with cant support configuring 8 unique addresses so I have hooked them up to the pi using an i2c 1-to-8 muxer, specifically the
[TCA9548A i2c expander](https://www.adafruit.com/product/2717). 

Currently am instantiating/controlling the muxer using the existing (but unmaintained) 
rust TCA9548A crate, `xca9548a`. The crate provides an impl of a `Xca9548a` switch
object which can be split into 8 `I2cSlave` objects, which is what is used for
each i2c channel as needed for controlling each sensor. 

For early prototyping I soldered up a little plug-n-play system using a proto pi hat (no eeprom) so I can easily swap in/out different sensors: 

![i2c muxer soldered onto perma proto hat with some extra headers for plug-n-play ability](./doc/i2cmux_build.jpg)

#### Note on async support

The ideal set up will be to control these sensors asynchronously but some work is needed to make that a well-implemented reality. The existing TCA9548A crate does not support `embedded_hal_async`.  To fake it till I can make that code, I have added wrappers around the `I2cSlave` object but the i2c calls are still blocking (which is bad). Supporting this build mode is a much lower priority but someday plan to add this (or get rid of wired build option completely)
