# pmindd

Currently just defines a bin for testing broker layer with Thread mesh, via the `plant-minder-mesh` binary

## Build
For building for the RPI 4 running raspbian kernel release `6.1.0-rpi7-rpi-2712` / kernel version `#1 SMP PREEMPT Debian 1:6.1.63-1+rpt1 (2023-11-24)`, I am using rust target `aarch64-unknown-linux-gnu`, and have separately installed the `aarch64-linux-gnu-gcc` toolchain (version 11.4.0), building on Ubuntu. 

To build the current `plant-minder-mesh` example bin, use the following:
```
cargo build --target=aarch64-unknown-linux-gnu --bin plant-minder-mesh --features="thread_mesh" --release
```

For older versions like 3b+ use the `armv7-unknown-linux-gnueabihf` target and the appropriate toolchain. 

#### Tip: 
If you are not familiar with building code for remote targets and/or run into (& dont want to deal with) issues with missing/incompatible library versions, you can clone this repo and build it on target (on the pi). That way you wont need to worry about specifying the target or linking against the correct version of glibc etc. Otherwise if you dont want to do this make sure you have all the needed toolchains and targets installed and that your gcc toolchain is at the same path specified in this crate's `.cargo/config.toml` file

## Its Working:
Deploy & run the binary on target, set your desired log level however you prefer 
```
RUST_LOG=debug ./plant-minder-mesh
```
(Depending on which `ot-br-posix` build you deployed on the pi you may need to run as `sudo`)

When running, on startup, you will see logs about registration of discovered nodes and eventual sensor data: 
```
[2024-07-12T22:05:58Z INFO  plant_minder_mesh] Initializing broker & starting task loops
[2024-07-12T22:05:58Z DEBUG pmindb::broker] Starting event and monitor loop tasks...
[2024-07-12T22:05:58Z INFO  pmindb::broker] Setting up listener on socket PollEvented { io: Some(UdpSocket { addr: [fdc9:fdb2:9fe8:1:42e5:3794:5da0:ceaa]:1212, fd: 9 }) }
[2024-07-12T22:05:58Z INFO  pmindb::broker] Setting up node / network monitor task to check every 25 seconds
[2024-07-12T22:05:58Z DEBUG pmindb::broker] Polling for network change
[2024-07-12T22:05:58Z DEBUG pmindb::broker] Polling for new nodes
[2024-07-12T22:05:58Z INFO  pmindb::broker] Registering fdc9:fdb2:9fe8:1:9189:13e7:9ec2:ad68
[2024-07-12T22:05:58Z DEBUG pmindb::monitor] Registering node 4413 : fdc9:fdb2:9fe8:1:9189:13e7:9ec2:ad68
[2024-07-12T22:05:58Z INFO  pmindb::broker] Registering fdc9:fdb2:9fe8:1:d366:bef0:62d5:2964
[2024-07-12T22:06:28Z DEBUG pmindb::monitor] Registering node 4414 : fdc9:fdb2:9fe8:1:d366:bef0:62d5:2964
[2024-07-12T22:06:28Z INFO  pmindb::broker] Registering fdc9:fdb2:9fe8:1:d062:5abf:c70a:76b5
[2024-07-12T22:06:58Z DEBUG pmindb::monitor] Registering node 4415 : fdc9:fdb2:9fe8:1:d062:5abf:c70a:76b5
[2024-07-12T22:06:58Z INFO  pmindb::broker] Registering fdc9:fdb2:9fe8:1:3715:a581:7786:efd9
[2024-07-12T22:07:28Z DEBUG pmindb::monitor] Registering node 4416 : fdc9:fdb2:9fe8:1:3715:a581:7786:efd9
[2024-07-12T22:07:28Z INFO  pmindb::broker] Registering fdc9:fdb2:9fe8:1:d8a9:a6d9:398:ffcc
[2024-07-12T22:07:58Z DEBUG pmindb::monitor] Registering node 4402 : fdc9:fdb2:9fe8:1:d8a9:a6d9:398:ffcc
...
[fdc9:fdb2:9fe8:1:d062:5abf:c70a:76b5]:1212 sent moisture: 326 temp 86.31459
[fdc9:fdb2:9fe8:1:3715:a581:7786:efd9]:1212 sent moisture: 370 temp 88.52559
[fdc9:fdb2:9fe8:1:9189:13e7:9ec2:ad68]:1212 sent moisture: 385 temp 89.96271
[fdc9:fdb2:9fe8:1:d8a9:a6d9:398:ffcc]:1212 sent moisture: 364 temp 92.37707
[fdc9:fdb2:9fe8:1:d366:bef0:62d5:2964]:1212 sent moisture: 370 temp 92.61803
[2024-07-12T22:08:23Z DEBUG pmindb::broker] Polling for network change
[2024-07-12T22:08:23Z DEBUG pmindb::broker] Polling for new nodes
[2024-07-12T22:08:23Z DEBUG pmindb::broker] Polling for lost nodes
[fdc9:fdb2:9fe8:1:d062:5abf:c70a:76b5]:1212 sent moisture: 326 temp 86.50314
[fdc9:fdb2:9fe8:1:3715:a581:7786:efd9]:1212 sent moisture: 372 temp 87.80733
[fdc9:fdb2:9fe8:1:9189:13e7:9ec2:ad68]:1212 sent moisture: 385 temp 90.1402
[fdc9:fdb2:9fe8:1:d8a9:a6d9:398:ffcc]:1212 sent moisture: 365 temp 92.19032
[fdc9:fdb2:9fe8:1:d366:bef0:62d5:2964]:1212 sent moisture: 370 temp 92.99008
[2024-07-12T22:08:48Z DEBUG pmindb::broker] Polling for network change
[2024-07-12T22:08:48Z DEBUG pmindb::broker] Polling for new nodes
[2024-07-12T22:08:48Z DEBUG pmindb::broker] Polling for lost nodes
[fdc9:fdb2:9fe8:1:d062:5abf:c70a:76b5]:1212 sent moisture: 325 temp 86.86919
[fdc9:fdb2:9fe8:1:3715:a581:7786:efd9]:1212 sent moisture: 371 temp 88.7164
[fdc9:fdb2:9fe8:1:9189:13e7:9ec2:ad68]:1212 sent moisture: 384 temp 90.68378
[fdc9:fdb2:9fe8:1:d8a9:a6d9:398:ffcc]:1212 sent moisture: 365 temp 92.56378
[fdc9:fdb2:9fe8:1:d366:bef0:62d5:2964]:1212 sent moisture: 373 temp 92.99008
[2024-07-12T22:09:13Z DEBUG pmindb::broker] Polling for network change

```