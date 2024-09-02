# Tests

Bianry for testing broker layer with Thread mesh, via the `broker-mesh-test` binary.

TODO: more hardware-in-the-loop tests!

## Build 
To build the current `broker-mesh-test` test bin for rpi5, use the following:
```
cargo build --target=aarch64-unknown-linux-gnu --bin broker-mesh-test --release
```

#### Tip: 
If you are not familiar with building code for remote targets and/or run into (& dont want to deal with) issues with missing/incompatible library versions, you can clone this repo and build it on target (on the pi). That way you wont need to worry about specifying the target or linking against the correct version of glibc etc. Otherwise if you dont want to do this make sure you have all the needed toolchains and targets installed and that your gcc toolchain is at the same path specified in this crate's `.cargo/config.toml` file

## Its Working:
Deploy & run the binary on target, set your desired log level however you prefer 
```
RUST_LOG=trace ./target/release/broker-mesh-test
```
(Depending on which `ot-br-posix` build you deployed on the pi you may need to run as `sudo`)

When running, on startup, you will see logs about registration of discovered nodes and eventual sensor data: 
```
noid@raspberrypi:~/plant-minder $ RUST_LOG=trace ./target/release/broker-mesh-test
[2024-07-25T23:05:49Z INFO  broker_mesh_test] Initializing broker & starting task loops
[2024-07-25T23:05:49Z DEBUG pmindb::broker] Starting event and monitor loop tasks...
[2024-07-25T23:05:49Z INFO  pmindb::broker] Setting up node / network monitor task to check every 25s seconds
[2024-07-25T23:05:49Z DEBUG pmindb::broker] Polling for network change
[2024-07-25T23:05:49Z DEBUG pmindb::broker] Polling for new nodes
[2024-07-25T23:05:49Z INFO  pmindb::broker] Starting CoAP Registration for fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f on port 1254
[2024-07-25T23:05:49Z DEBUG pmindb::broker] Got a response from [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212, expected [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212
[2024-07-25T23:05:49Z INFO  pmindb::monitor] Node rloc: 17475 node port 1254
[2024-07-25T23:05:49Z DEBUG pmindb::monitor] Registering node rloc 17475 : port 1254
[2024-07-25T23:05:49Z TRACE pmindb::broker] Node being added to DB [96, 85, 249, 247, 7, 120] addr fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f
[2024-07-25T23:05:49Z TRACE pmindb::broker] Processing NodeEvent receiver as a stream
[2024-07-25T23:05:49Z INFO  pmindb::db] Got a new node reg :) CreateOrModify { eui: [96, 85, 249, 247, 7, 120], ip: fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f }
[2024-07-25T23:05:49Z INFO  pmindb::broker] Starting CoAP Registration for fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958 on port 1284
[2024-07-25T23:05:49Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212 moisture 956 temp 91.14108
[2024-07-25T23:05:49Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f, SensorReading { moisture: 956, temperature: 91.14108, full_spectrum: 3592, lux: 84.92312 }))
[2024-07-25T23:05:50Z DEBUG pmindb::broker] Got a response from [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212, expected [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212
[2024-07-25T23:05:50Z INFO  pmindb::monitor] Node rloc: 17476 node port 1284
[2024-07-25T23:05:50Z DEBUG pmindb::monitor] Registering node rloc 17476 : port 1284
[2024-07-25T23:05:50Z TRACE pmindb::broker] Node being added to DB [96, 85, 249, 246, 242, 68] addr fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958
[2024-07-25T23:05:50Z TRACE pmindb::broker] Processing NodeEvent receiver as a stream
[2024-07-25T23:05:50Z INFO  pmindb::db] Got a new node reg :) CreateOrModify { eui: [96, 85, 249, 246, 242, 68], ip: fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958 }
[2024-07-25T23:05:50Z INFO  pmindb::broker] Starting CoAP Registration for fdc9:fdb2:9fe8:1:3907:598:9e77:3a97 on port 1231
[2024-07-25T23:05:50Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212 moisture 399 temp 91.24501
[2024-07-25T23:05:50Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958, SensorReading { moisture: 399, temperature: 91.24501, full_spectrum: 9345, lux: 205.4654 }))
[2024-07-25T23:05:56Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212 moisture 941 temp 91.14108
[2024-07-25T23:05:56Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f, SensorReading { moisture: 941, temperature: 91.14108, full_spectrum: 3580, lux: 84.633026 }))
[2024-07-25T23:05:59Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212 moisture 405 temp 91.43175
[2024-07-25T23:05:59Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958, SensorReading { moisture: 405, temperature: 91.43175, full_spectrum: 9490, lux: 208.87677 }))
...
[2024-07-25T23:06:50Z DEBUG pmindb::broker] Polling for lost nodes
[2024-07-25T23:07:11Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212 moisture 958 temp 91.14108
[2024-07-25T23:07:11Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f, SensorReading { moisture: 958, temperature: 91.14108, full_spectrum: 3653, lux: 85.06385 }))
[2024-07-25T23:07:14Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212 moisture 403 temp 90.684814
[2024-07-25T23:07:14Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958, SensorReading { moisture: 403, temperature: 90.684814, full_spectrum: 10592, lux: 233.5217 }))
[2024-07-25T23:07:15Z DEBUG pmindb::broker] Polling for network change
[2024-07-25T23:07:15Z DEBUG pmindb::broker] Polling for new nodes
[2024-07-25T23:07:15Z INFO  pmindb::broker] Starting CoAP Registration for fdc9:fdb2:9fe8:1:3907:598:9e77:3a97 on port 1231
[2024-07-25T23:07:36Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f]:1212 moisture 943 temp 91.32147
[2024-07-25T23:07:36Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:766d:d75b:52f7:c71f, SensorReading { moisture: 943, temperature: 91.32147, full_spectrum: 3514, lux: 82.03662 }))
[2024-07-25T23:07:39Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958]:1212 moisture 399 temp 91.24501
[2024-07-25T23:07:39Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:50f3:df65:c47a:d958, SensorReading { moisture: 399, temperature: 91.24501, full_spectrum: 8887, lux: 197.08366 }))
[2024-07-25T23:07:45Z WARN  pmindb::broker] Registration failed, need to retry
[2024-07-25T23:07:45Z INFO  pmindb::broker] Starting CoAP Registration for fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d on port 1231
[2024-07-25T23:07:45Z DEBUG pmindb::broker] Got a response from [fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d]:1212, expected [fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d]:1212
[2024-07-25T23:07:45Z INFO  pmindb::monitor] Node rloc: 17478 node port 1231
[2024-07-25T23:07:45Z DEBUG pmindb::monitor] Registering node rloc 17478 : port 1231
[2024-07-25T23:07:45Z DEBUG pmindb::broker] Polling for lost nodes
[2024-07-25T23:07:45Z TRACE pmindb::broker] Node being added to DB [96, 85, 249, 247, 8, 32] addr fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d
[2024-07-25T23:07:45Z TRACE pmindb::broker] Processing NodeEvent receiver as a stream
[2024-07-25T23:07:45Z INFO  pmindb::db] Got a new node reg :) CreateOrModify { eui: [96, 85, 249, 247, 8, 32], ip: fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d }
[2024-07-25T23:07:45Z DEBUG pmindb::broker] Reading! from [fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d]:1212 moisture 636 temp 88.520615
[2024-07-25T23:07:45Z INFO  pmindb::db] Got a sensor reading :) NodeSensorReading((fdc9:fdb2:9fe8:1:c7bd:1e76:2eaa:f89d, SensorReading { moisture: 636, temperature: 88.520615, full_spectrum: 15705, lux: 359.24698 }))

```