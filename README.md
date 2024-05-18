# plant-minder
RPI4 + soil sensors to track when my plants need watering 

Soil sensor code derived from https://github.com/adafruit/Adafruit_Seesaw and 
https://github.com/adafruit/Adafruit_CircuitPython_seesaw/

There are a few rust crates out there for controlling/interfacing with this soil
sensor but I have written something specifically for my set up and desired 
custom functionality.

I have 8 sensors and they all have the same unchangeable i2c address,
so have hooked them up to the pi using an i2c 1-to-8 muxer, specifically the
[TCA9548A i2c expander](https://www.adafruit.com/product/2717). Currently am
instantiating/controlling the muxer using the existing (but seemingly unmaintained) 
rust TCA9548A crate, `xca9548a`. The crate provides an impl of a `Xca9548a` switch
object which can be split into 8 `I2cSlave` objects, which is what is used for
each i2c channel as needed for controlling each sensor. 

I also want to run everything / control these sensors in an async context. 
Some work is needed to make this a reality; the existing TCA9548A
crate does not support `embedded_hal_async`.  To accomdate this/fake it till I can
make that code, I have added a wrapper around the `I2cSlave` object and 
implemented the needed async traits on the wrapper, but the i2c calls are still 
blocking (bad). So its just something to enable me to continue building out the rest 
of the program/async framework. The TCA9548A lib does not seem to be maintained
so updating that to work with async embedded-hal is a future TODO. 


## Dependencies

## Build

## Run

