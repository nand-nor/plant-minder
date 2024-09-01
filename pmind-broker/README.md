# `pmind-broker`

The broker layer interfaces with client subscribers as well as sensor node servers on the Thread mesh (via the `otbr-agent`/ `openthread` stack). The broker is configured to provide the following responsibilities/functionality
- node monitoring 
  - register new nodes as they come online (done automatically)
  - detect when nodes drop off the network 
- manage socket(s) where sensor data is received 
  - route received data to appropriate queue
  - detect when socket error arises or when node times out (specific amount of time has passed since last data report), clean up as needed and report as a node event
- push sensor data and node events (registration, termination) into event queues 
- expose an API to enable clients to subscribe to events/sensor data pushed to event queues