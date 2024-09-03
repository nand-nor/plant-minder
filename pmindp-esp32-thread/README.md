# `pmindp-esp32-thread` 

Code in this dir is used to program esp32 dev boards to act as remote sensor nodes, reporting sensed soil data (and other sensed data, depending on attached sensors) to an RPi via the Thread protocol / CoAP. 

Supports the `esp32c6` and `esp32h2` model dev boards, as well as a number of sensor types. Features are used to conditionally compile for either the `esp32c6` or `esp32h2` dev boards and sensor initialization and reporting. See below for list of supported sensors and their associated feature flags.

# Contents
- [Build Steps](#build)
  - [Assigning a Plant Database ID via cfg.toml](#plant-database-recors-and-cfg.toml)
  - [Example Log Output](#working-example-log-output)
- [Status](#status)
  - [What Works](#what-works)
  - [What Does Not Work](#what-does-not-work)
  - [Sensor Support](#sensor-support)
      - [Soil Sensors](#soil-sensors) 
      - [Light Sensors](#light-sensors) 
      - [Gas/Humidity Sensors](#gas/humidity-ensors) 
- [Design Details](#design-details)
   - [Generic Sensor Types](#generic-sensor-types)
- [Limitations and Future Work](#limitations-and-future-work)   


# Build

Requires nightly, `espflash` toolchain and `riscv32imac-unknown-none-elf` target

For example, to build for an esp32c6 dev board with an atsamd10 sensor attached, use the following: 
```
cargo +nightly espflash flash --monitor --bin main --features="esp32c6","atsamd10" --target=riscv32imac-unknown-none-elf --release --port <PORT> 
```

Make sure to erase flash first (and before deploying newly compiled code):
```
cargo espflash erase-flash --port <PORT>
```

If you only have one dev board attached to your dev machine then you can omit the `port` arg.

## Plant Database Records and `cfg.toml`

Each sensor can be built to associate it with a specific plant you are monitoring. This is useful for database purposes; you can associate a known plant type with the stream of sensor data. 

As you build and flash the code onto a sensor, note which plant you want to put it in and give it a unique name in the name field of `cfg.toml` file. This is used by the RPi to associate sensor data with a plant record. More build configuration options will likely be added to this file in the future. The `cfg.toml` file should look like this (see `pmindp-sensor/lib.rs`):
```
[pmindp-sensor]
pot_num = 17
name = "SunroomJade"
species = "Jade"
growth_stage = GrowthStage::Vegetative
```

## Working example log output

In the following log output, the device has an `atsam10`, `tsl2591`, and `bme860` sensor attached. It has an associated plant record name of "Orchid". When things are working you will see serial output like this: 
```
...
INFO - otPlatRadioSetShortAddress 0x4080c270 55439
0x4080c270 - ot::gInstanceRaw
    at ??:??
WARN - timer interrupt triggered at 4034
INFO - trigger_tx_done
INFO - EventAckRxDone
WARN - timer interrupt triggered at 23922
INFO - Received CoAP request '1289 Get soilmoisture' from fd2e:c69b:fa93:1:134c:7a9f:bbe7:e5b5
INFO - Currently assigned addresses
INFO - fd2e:c69b:fa93:1:a2ee:6ebc:c675:9cc6
INFO - fdde:ad00:beef::ff:fe00:d88f
INFO - fdde:ad00:beef:0:f436:cf62:7907:c78
INFO - fe80::e0d4:9263:87d:bd77
INFO - Role: Child, Eui [
    0xF0,
    0xF5,
    0xBD,
    0x1,
    0x65,
    0xF4,
] Plant Name "Orchid" Port 1289
INFO - Handshake complete
INFO - Sending SensorReading { soil: Soil { moisture: 322, temp: 86.89116 }, light: Some(Light { fs: 46863, lux: 1133.0674 }), gas: Some(Gas { temp: 88.772, p: 679.21, h: 54.009, gas: 162754 }), ts: 0 }
WARN - timer interrupt triggered at 45628
WARN - timer interrupt triggered at 45633
INFO - trigger_tx_done
INFO - EventAckRxDone
WARN - timer interrupt triggered at 45644
WARN - timer interrupt triggered at 45649
INFO - trigger_tx_done
WARN - timer interrupt triggered at 45657
WARN - timer interrupt triggered at 45662
INFO - trigger_tx_done
INFO - EventAckRxDone
INFO - Sending SensorReading { soil: Soil { moisture: 322, temp: 86.52355 }, light: Some(Light { fs: 46952, lux: 1135.2434 }), gas: Some(Gas { temp: 88.772, p: 679.21, h: 54.003, gas: 169140 }), ts: 0 }
...
```

And you should also see on the RPi the received data.

<img src="../doc/sensor_esp32c6.jpg" width="250" height="300"> <-- soil sensor with breadboard

Folks interested can set it up with a few different constructions/prototypes using protoboard (requires soldering obviously) for example:
<img src="../doc/protoboard.jpg" width="600" height="800"> 


# Status

The following is a high-level description of what currently works followed by a by-no-means comprehensive list of what does not work (but which I hope to soon address). I also cover what sensors the code currently supports and the feature flags used to build for them. 

## What works:
- Devices attach to an established Thread network (with hardcoded creds) and act as MTD (child) devices on the network
- Sensor control (reading data) with a number of sensors currently supported (See next section) 
- Minimal CoAP logic exists
    - Oberver registration
    - Sensor publishing at compile-time-fixed intervals
- Feature-flag enabled sensor configuration
- Compile-time configuration of reported plant record identifier (name, species, etc.)
- Some minimal logic for `tsl2591` light sensors to dynamically adjust based on current light conditions (This works but the logic could be optimised to be more performant e.g. to find the optimal config faster)

## What does not work:
- FTD/Router node support (limitation of `esp-openthread` but Im workin on it!)
- Robust CoAP support 
- Thread commissioning/joiner support  (limitation of `esp-openthread` but Im workin on it!)
- NVM storage
- better error handling; theres lots of places with unwraps that could panic 
    - w.r.t. error handling / panic, need some kind of watchdog to trigger board reset if there is a panic, right now panics will halt all operation
- Plenty more (see [Limitations and Future Work](#limitations-and-future-work))

## Sensor Support

A number of sensor types are currently supported: soil, light, and gas/humidity. Code is configured to support up to 5 sensors per esp32: one soil sensor (required), 2 light sensors, a gas/humidity sensor, and misc/other (basically TBD what this type will be).

Code must be compiled with at least a soil sensor type set up; only one soil sensor type and one physical soil sensor is currently allowed (any attempt to build with more will generate a compile time error).  

Note that any i2c device attached must have a unique address although future enhancements may involve allowing configuration for i2c mux device to allow up to 8 i2c devices with the same address.

The following sensor types and models are supported, listed with the feature flag to use to enable them. Product links can be found in [the parts list](./doc/part_list.md) 

### Soil Sensors

| Sensor/Product Name         | Feature flag  |
|-----------------------------|---------------|
| Seesaw soil sensor atsamd10 | `atsamd10`    |
| Sparkfun resistive probe    |`probe_circuit`|

### Light Sensors

| Sensor/Product Name         | Feature flag  |
|-----------------------------|---------------|
| Adafruit lux/light sensor   | `tsl2591`     |

### Gas/Humidity Sensors

| Sensor/Product Name         | Feature flag  |
|-----------------------------|---------------|
| Adafruit/bosch Gas/humidity |`bme680`       |
| Adafruit SHT40 humidity/temp|`sht40`        |

### Planned Support
- sunfounder soil sensor st0160
- NPK sensors (will need to do some legwork to determine if I can support modbus on this device, that is still TBD)
- Plenty of others; maybe a VC02 sensor or pH 

# Design Details

Thread provides the transport layer for reporting sensor data to the RPi. Once programmed, esp32 dev boards come up as minimal thread devices (MTD) or child nodes. The code is currently designed to allow attachment to the Thread mesh network via hardcoded operational dataset. This is needed until the `esp-openthread` repo supports joiner functionality. 

At a high level the controlling logic is a simple event loop. After a series of configuration steps, the node will join the Thread network, open a socket on a pre-determined port known to the RPi (broker layer), and enter the main event loop. 

In the event loop it will service any tasklets/pending processes that arise due to normal `openthread` operation. It will continue to run this loop just processing normal `openthread` operation until it receives a CoAP observer registration from the RPi. 

Once CoAP registration is received, the node will start reporting sensed data at a fixed interval, depending on which sensors are currently configured/attached to the board. As part of the event loop, it will check to see if a registration request has been made. If yes, it checks to see if the sensor(s) should be read, which is configured via timer so reports are on fixed intervals. If the timer has expired since the last read, then the platform will call read on each attached sensor and send data via the mesh. 

If the node experiences some unrecoverable sensor error or otherwise drops off the Thread network, it will exit the event loop, which causes the node to reset itself. When it comes up post-reset (or any power event) it will join the thread network as a fully new node. The broker logic running on the RPi will pick it up as the same node from prior to the reset (using the EUI); the RPi will re-register with the node to receive sensor data without any human intervention. The tracked data will continue to be associated with the plant using the device's EUI/reported plant record. 

## Generic Sensor Types

The `pmindp-esp32-thread` crate depends on the `pmindp-sensor` crate, which defines a number of sensor traits and structs. There is a platform level trait, `pmindp_sensor::SensorPlatform`, where on a fixed interval the esp32 will iterate over a vector of generic sensor objects, and if instantiated, call the generic `read` method implemented for each sensor in the vector. This is done via each supported sensor's implemetation of `pmindp_sensor::Sensor` which defines the sensor-specific `read` operation. 

The `pmindp-sensor` crate defines the data structs that the nodes use to report sensed data to the RPi. Each sensor type has an associated struct that gets populated and written into a buffer (for sending) on the platform-level call to `pmindp_sensor::Sensor::read`. Each attached sensor will write to the buffer, which is then serialized and sent to the RPi via the Thread mesh. 

The supported sensors implemented in this crate can be conditionally compiled using feature flags, and the design supports multiple compositions of sensors (e.g. a soil sensor and a humidity sensor, or a soil sensor, a light sensor, and pressure/gas/humidity sensor).

# Limitations and Future Work

Currently there are some big limitations that I hope to address in the near term. The biggest limitations are lack of range/scalability due to how I have implemented some broker layer logic, and due to how the sensors are very simple in their implementation & currently only support acting as child devices on the thread network (MTDs). Effectively the current design requires there be only one router in the mesh which imposes some severe limitations including (but not limited to):

- Range of the child (sensor) nodes is limited: sensor nodes cant be too far away from the RPi or they will drop off the network
- Single router means less overall mesh coverage; range of mesh network limited to what can be reached from a single hop from the single router node. The network will not leverage benefits of a full mesh. The resulting network topology will have a star, or, a hub and spoke topology, where the single router (RPi) is the center
- Single router also means there is a single point of failure: If that device fails, the whole network will drop and there will be no recovery/self healing (which is one of the ocol things Thread offers)
- Smaller number of supportable nodes: size of the mesh is limited to the number of child nodes the one router (the RPi) can support. 

Future planned work for addressing these limitations: 
- Modify design of OT monitor (in `pmind-broker`) to leverage SRP/DNS-SD to discover nodes with "sensor services"
- Add logic in `pmindp-esp32-thread` to program sensor nodes to register sensor services (via SRP) so their IP address can be discovered using DNS-SD 
- Work on `esp-openthread` repo to add FTD support (lots needed there)

Other work needed is to improve error handling and recovery. Future optimizations will involve better recovery and logic to enable nodes to store data in NVS so they can perhaps store certain info like the dataset / can come back online after a power event and register with the same addresses etc. 

There are also many places in both this code and in the branch of `esp-openthread` I am using where there are unwraps which need to be improved so that we dont panic anywhere. If there is a panic the reset logic will not trigger and the node will remain offline. So this needs some attention