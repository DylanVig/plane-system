# configuration file

The plane system accepts configuration from `plane-system.json`.

This file is located automatically if it is in the working directory when the plane system is invoked, but if it's located somewhere else, it can be specified with the CLI option `--config=/path/to/plane-system.json`.

Note that the executable is also named `plane-system`, so if the executable and the config file are in the same directory, then you need to specify the JSON file specifically using the `--config` option. Or you can rename the executable.

# configuration options

## `pixhawk`

This property controls how the plane system interacts with a Pixhawk. Set this to `null` disable communication with a Pixhawk, or provide an object with the following properties:

- `mavlink`: required, accepts an object of the form `{ "type": "V1" }` where `type` is `V1` or `V2`, depending on the Mavlink protocol version that the Pixhawk sends
- `address`: required, accepts a socket address (host and port) where the plane system will listen for UDP packets

## `plane_server`

This property controls the plane system's HTTP API. Provide an object with the following properties.

- `address`: required, accepts a socket address (host and port) where the plane system will listen for incoming HTTP requests

## `ground_server`

This property controls the plane system's interface with the ground server over HTTP. Set this to `null` to disable communication with the ground server, or provide an object with the following properties:

- `address`: required, accepts an HTTP address where the ground server's API is available (do not include the path, just the scheme and hostname)

## `scheduler`

**The scheduler is still a work in progress. The `gps` property will be removed in future versions.**

This property controls the plane system's image capture scheduler. Set this to `null` to disable automated image capture and gimbal control, or provide an object with the following properties:

- `gps`: required, accepts an object with properties `latitude` (number) and `longitude` (number) that describe a GPS location where the gimbal should point

## `camera`

This property controls the plane system's interface with a camera. Set this to `null` to disable the camera, or provide an object with the following properties:

- `kind`: required, accepts a camera model (string)
  - the following camera models are currently defined: `R10C`
- `save_path`: optional, accepts a path where images captured by the camera will be saved when they are downloaded. if this is not specified, the plane system will save them in the present working directory.

## `gimbal`

This property controls the plane system's interface with a gimbal. Set this to `null` to disable the gimbal, or provide an object with the following properties:

- `kind`: required, accepts a gimbal type (object) that can be one of the following:
  - `{ "type": "software" }`: simulated gimbal
  - `{ "type": "hardware", "protocol": "SimpleBGC" }`: hardware gimbal that communicates over the SimpleBGC protocol
- `device_path`: optional, the path to the device file for gimbals that communicate via USB or serial connections. if this is not specified, and `kind.type` is `"hardware"` , the plane system will try to find the gimbal automatically. an error will be thrown if this process fails.
