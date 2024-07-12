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
WARN - timer interrupt triggered at 23474
Received CoAP request message ID '0 Get soilmoisture' from fdc9:fdb2:9fe8:1:42e5:3794:5da0:ceaa
Currently assigned addresses
fdc9:fdb2:9fe8:1:216d:ee29:ed28:896e
fdde:ad00:beef::ff:fe00:440e
fdde:ad00:beef:0:e2b9:970c:4c1d:3cf0
fe80::b0c1:23f0:28c5:7708

Role: Child, Eui [
    0x60,
    0x55,
    0xF9,
    0xF7,
    0x8,
    0x20,
]
Handshake complete
WARN - timer interrupt triggered at 23548
WARN - timer interrupt triggered at 23552
INFO - trigger_tx_done
INFO - EventAckRxDone
Moisture: 345, temp: 87.76756
Role: Child
WARN - timer interrupt triggered at 25343
INFO - trigger_tx_done
Moisture: 347, temp: 88.1336
Role: Child
WARN - timer interrupt triggered at 50342
...
```

And you should also see on the RPI the received data.



## Error handling / Recovery
Right now, if the node experiences an unrecoverable error it will trigger a reset which will knock the node offline temporarily, but it will come back online and rejoin. When it comes up it will join the thread network as a fully new node. The broker logic running on the RPI will pick it up as a new node & will re-register to receive sensor data without any human intervention. 

Future optimizations will involve better recovery and logic to enable nodes to store data in NVS so they can come back up with the same info / retain the same RLOC addresses etc. after restarting, if possible (depends on how quickly it can come up the parent may not give it the same child ID ). 

There are also many places in both this code and in the branch of esp-openthread I am using where there are unwraps which need to be improved so that we dont panic anywhere. If there is a panic the reset logic will not trigger and the node will remain offline. So this needs some attention