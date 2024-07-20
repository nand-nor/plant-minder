# Plant-Minder (WIP)

RPI4 + soil sensors to track when my plants need watering. 

## Contents
- [Description](#description)
- [Components / Design Details](#components--workspace-design-details)
  - [Esp32 / Sensors](#esp32sensor-layer)
  - [Broker layer](#broker-layer)
  - [TUI layer](#front-end--tui-layer) 
- [Status](#status--goals--hopes--dreams)
- [Limitations](#limitations)


## Description

This repo contains (mostly) all needed code for deployiong a simple plant monitoring system, which is a distributed system of microcontrollers programmed to control and report sensor data. Microcontrollers sense and report soil moisture data via a wireless mesh protocol, which is received by a raspberry pi. The pi has logic to determine soil conditions / trends and will ultimately alert me with a big obvious visual display whenever I need to water my plants.

```
         _            ________________________             _
        |            |                        |             |             
        |   pmindd > |      TUI Front End     |             |   < plant-minder
        |            |________________________|             |    (single binary)
        |                ^               |                  |   
    RPI |                | Events        | Subscribe        | 
        |             ___|_______________v____              |          __________
        |            |                        |             |         |  sqlite  |
        |   pmindb > |     Broker / Backend   |             |-------> | database |
        |            |_________(CoAP)_________|            _|         |__________|
        |                ^            |         
        |             ___|____________v_______             
        |            |      otbr-agent/       |             
        |            |       openthread       |             
        |_           |________________________|            
                       ^          ^          ^             
                       | 802.15.4 |          |      
                       |          |          |                
                     __|___    ___|___    ___|___ 
pmindp-esp32-thread>| ESP32|  | ESP32 |  | ESP32 | <-- CoAP
                     ------    -------    ------- 
                       ^          ^          ^
                       | i2c      |          |
                    ___v____   ___v____   ___v____    
                   | Sensor | | Sensor | | Sensor | 
                    --------   --------   --------  
```
### Components / Workspace Design details
The `pmindd` crate is where the front end/TUI rendering logic is defined (or, will be, when this is closer to being done). It will (probably) run as a daemon, interfacing with the broker layer to receive and render events. Its main responsibility will be displaying sensor data as it is received from the mesh. It will do this very simply via TUI using `ratatui` 

The `pmindb` crate is a lib where the the broker/monitor layer is defined / implemented, which interfaces with the front end layer to provide the following responsibilities/functionality
- node monitoring & management
  - register new nodes as they come online (done automatically)
  - manage when nodes drop off the network
  - associate nodes that have had to reset themselves with their previous database entry (TBD)
- manage socket(s) where sensor data is received 
- push data into event queues and/or database (TBD what this piece will look like)
- expose event queues for the TUI front end to subscribe 
- provide requested info from the database 

The `otbr-agent` / `openthread` layer running on the pi is provided via a 3rd party binary; the pi must be set up to run the openthread stack via `otbr-agent`. More details / build steps available in [the parts list](./doc/part_list.md).

The `pmindp-esp32-thread` crate contains all of the code needed to program microcontrollers to control the soil sensor & to respond to CoAP registration requests from an observer (done by the broker layer in `pmindb`). 

### ESP32/Sensor Layer

Esp32 microcontrollers are used to control sensors and report data to the RPI via Thread, a wireless mesh protocol that runs on top of 802.15.4. Only 15.4 capable esp32 dev boards can be used; currently only esp32-c6 and esp32-h2 dev boards have an 802.15.4 native radio. 

The `pmindp-esp32-thread` crate contains all the code for building & flashing the esp32 dev boards with attached sensors (see photos below for example). This code is built on top of / uses libraries from `esp-hal`, and the Thread capability is provided directly via the `openthread` stack, which we can call into from Rust via the `esp-openthread` repo. The boards run bare metal (via `esp-hal`) and have code to control the soil sensor as a simple i2c device. 

As mentioned above, Thread provides the transport layer for reporting sensor data to the RPI. The code in the `pmindp-esp32-thread` crate programs the boards to program a hardcoded operational dataset to auto-attach to the Thread mesh network as a minimal thread device (MTD). It is worth noting that there is no support for NCP or RCP modes in the `esp-openthread` repo currently (these boards dont need it), so no need for dealing with any spinel shennanigans. 

For the soil sensor, the code currently only supports [Seesaw Capacitive moisture sensor (ATSAMD10)](https://www.adafruit.com/product/4026). Although I do have some plans to eventually  support other sensors (both different soil sensors and other sensor types like humidity / light/etc.).

![esp32-c6 controller with sensor on pins 5 & 6](./doc/sensor_esp32c6.jpg)

To support this deployment mode, the RPI must be configured to run the openthread stack with an RCP radio. The plant-minder system currently assumes that the RPI is acting as a border agent but future iterations may change this (there is no real requirement currently for bidirectional IP connectivity). 

The Base i2c control for the ATSAMD10 chip ([seesaw soil sensor](https://www.adafruit.com/product/4026)) is defined in `pmindp-sensor`. This is where other sensor impls will go if/when I get to that.

### Broker Layer 
Under active development. A main goal for this layer is to provide node management/monitoring, so that the system is fault tolerant and even if remote nodes fall off the network they will be picked back up and register to report sensor data as soon as they rejoin the network. This layer also is meant to handle received data and generate relevant event notifications etc. based on top-level subscriptions, so that I can ideally support different front end apps if I ever get to that point. 

Some additional [details on test layer / expected output here](./pmind-tests/README.md).


### Front end / TUI Layer
For the first iteration I am targeting a simple TUI using `ratatui`. The current plan is to have this layer render the UI / data by subscribing to sensor events via the broker layer. It will also interface with the broker layer to query the database for rendering data trends and retrieving stored state like associations of plants with sensors, plant species, ideal soil moisture conditions, that sort of thing. I am striving for this to be as simple as possible-- all I need is to be provided with a visual cue that it is time to water my plants. 

More info on current status, build info, and [other details here](./pmindd/README.md).


## Status / Goals / Hopes / Dreams

In general I would estimate this is roughly at 55% complete. Lots of work is still needed. But basic sensor control / running openthread on the esp32 devices, and receiving reported sensor data on the pi is working.

One major goal is more complex OT device type support for remote sensor controllers. The `esp-openthread` repo currently only supports running esp32 boards as MTDs. Work is ongoing to add support for running as both FTDs and as SED/SSEDs. Ideally these nodes will be able to run as FTDs when mains powered (so they can route packets for eachother) and SED (sleepy end device) waking up only to read and publish sensor data, for batter powered devices. 

Another goal is to eventually support other moisture sensors
- [Sunfounder capacitive moister sensor](https://www.digikey.com/en/products/detail/sunfounder/ST0160/22116813) 
- [SparkFun soil moisture sensor](https://www.digikey.com/en/products/detail/sparkfun-electronics/SEN-13322/5764506)

Additional sensor types will also eventually be added, targeting humidity co2 and light sensors.

## Limitations
In general there are many; this is just a hobby project being done in my spare time. But arguably the biggest limitations of the current system is that the sensors currently only support acting as child devices on the thread network (MTDs) and are very simple in their implementation. The TL;DR of these limitations means that range is limited and the system will not really have the benefits of a full mesh (it will have a star, or hub and spoke topology). Therefore the child nodes cant be too far away from the RPI or they will drop off the network. The current impl also requires that no other router nodes be on the network that may be at any point "better parents" than the RPI (e.g. have better link quality w.r.t. any given child node). 

#### More supplemental info re: above informations (skippable)
There are a few reasons for this: the OT client implementation currently monitors the thread network by periodically pulling the child ip addresses (through the `childip` cli cmd) and checking the returned values to presence of new or absence of nodes. If the child nodes have better link quality with another router, they will perform mesh link establishment/basically reparent themselves to that router. That means that the RPI wont have access to the IP addresses of those child nodes via the `childip` cli command. Obtaining addresses that parents will route packets to child nodes for, without the child node performing some additional steps (service registration), then becomes an issue. This can be solved by adding code where the child nodes register services (a "soil sensor" service for example) with an SRP service registrar, which the RPI is providing as a border router. Then the RPI (or any device on the network really) can perform a service lookup to find such services, and to obtain a routable IP address that will allow parents to route packets using that address successfully to child nodes. That will add a lot more complexity so that will be added as a future enhancement. 