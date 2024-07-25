# `pmindp-esp32-thread` 

Program esp32 dev boards to act as remote sensor nodes / CoAP servers for Observe option (very loosely based on RFC RFC 7641) 

## Build

Requires nightly, `espflash` toolchain and `riscv32imac-unknown-none-elf` target

For example, to build for an esp32c6 dev board: 
```
cargo +nightly espflash flash --monitor --bin main --features="esp32c6" --release --port <PORT>
```

Make sure to erase flash first (and before deploying newly compiled code):
```
cargo espflash erase-flash --port <PORT>
```

If you only have one dev board attached to your dev machine then you can omit the `port` arg

When things are working you will see serial output like this: 
```
...
INFO - otPlatRadioSetShortAddress 0x4080c2b8 17473
0x4080c2b8 - ot::gInstanceRaw
    at ??:??
WARN - timer interrupt triggered at 3405
WARN - timer interrupt triggered at 3412
INFO - trigger_tx_done
WARN - timer interrupt triggered at 4036
WARN - timer interrupt triggered at 23304
INFO - Received CoAP request '1217 Get soilmoisture' from fdc9:fdb2:9fe8:1:42e5:3794:5da0:ceaa
INFO - Currently assigned addresses
INFO - fdc9:fdb2:9fe8:1:26a1:dada:a463:7170
INFO - fdde:ad00:beef::ff:fe00:4441
INFO - fdde:ad00:beef:0:938e:cc3c:5ef3:e63
INFO - fe80::540b:1bb0:3d33:81b7
INFO - Role: Child, Eui [
    0x60,
    0x55,
    0xF9,
    0xF7,
    0x7,
    0x78,
] port 1217
INFO - Handshake complete
INFO - moisture 945
INFO - temperature 90.76903
INFO - Full spectrum luminosity 2814
INFO - lux 62.783764
INFO - Role: Child
WARN - timer interrupt triggered at 33359
INFO - trigger_tx_done
WARN - timer interrupt triggered at 33370
WARN - timer interrupt triggered at 33375
INFO - trigger_tx_done
INFO - EventAckRxDone
INFO - moisture 956
INFO - temperature 90.96068
INFO - Full spectrum luminosity 2838
INFO - lux 63.703
INFO - Role: Child
...
```

And you should also see on the RPI the received data.



## Error handling / Recovery
Right now, if the node experiences an unrecoverable error it will trigger a reset which will knock the node offline temporarily, but it will come back online and rejoin. When it comes up it will join the thread network as a fully new node. The broker logic running on the RPI will pick it up as a new node & will re-register to receive sensor data without any human intervention. 

Future optimizations will involve better recovery and logic to enable nodes to store data in NVS so they can perhaps store certain info like the dataset / can come back online after a power event and register with the same addresses etc

There are also many places in both this code and in the branch of `esp-openthread` I am using where there are unwraps which need to be improved so that we dont panic anywhere. If there is a panic the reset logic will not trigger and the node will remain offline. So this needs some attention

## Needs (in no particular order)
- Dynamic configuration of the light sensor based on senses light conditions (varying gain and integration time)
- Data published as `json_serde`-parseable UDP packets (ideally can keep it simple so can derive serialization and deserialization traits) 
- More sensors???
- NVM storage support
- better error handling / lots of places with unwraps that could cause panic
- Most importantly: FTD/router node support! Needs work in `esp-openthread`

