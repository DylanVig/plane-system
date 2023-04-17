# configuration file

The plane system accepts configuration from a JSON file. We have lots of preset
configurations in the `config` folder.

You specify the configuration using the `--config` command line argument:

```sh
./plane-system --config config/camera-only.json
cargo run -- --config config/camera-only.json
```

# configuration options

## `pixhawk`

This property controls how the plane system interacts with a Pixhawk. Set this to `null` disable communication with a Pixhawk, or provide an object with the following properties:

- `pixhawk.mavlink`: required, accepts an object of the form `{ "type": "V1" }` where `type` is `V1` or `V2`, depending on the Mavlink protocol version that the Pixhawk sends
- `pixhawk.address`: required, accepts a socket address (host and port) where the plane system will listen for UDP packets

## `ground_server`

This property controls the plane system's interface with the ground server over HTTP. Set this to `null` to disable communication with the ground server, or provide an object with the following properties:

- `ground_server.address`: required, accepts an HTTP address where the ground server's API is available (do not include the path, just the scheme and hostname)

## `main_camera`

This property controls the plane system's interface with the [Sony
R10C](https://www.notion.so/Sony-R10C-a3b2547c751147a38a154f326d40f312) camera.
Set this to `null` or omit it to disable the camera, or provide an object with
the following properties:

- `main_camera.download` (object): 
  - `main_camera.download.save_path` (string): The path where images captured by
    the R10C will be saved once they are downloaded. The plane system will
    automatically create a folder named after the current time inside of this
    path and save videos here. 
- `main_camera.live` (object, optional):
  - `main_camera.live.framerate` (float): The framerate at which the camera's
    live preview should be requested. Must be greater than zero and less than or
    equal to 30. 
    
    Note that you also need to set `livestream.preview` in order to see data
    from the camera's live preview.

## `livestream`

This property controls the plane system's interface with GStreamer, which can be
used to save video to files and livestream video to the ground. Set this to `null` or omit it to disable this, or provide an object with the following properties:

- `livestream.preview` (object, optional):
  - **Note:** this property must be used with `main_camera.live`.
  - `livestream.preview.save_path` (string): Path where videos from the camera
    preview should be saved. The plane system will automatically create a folder
    named after the current time inside of this path and save videos here.
  - `livestream.preview.bin` (array of string): Describes a GStreamer [bin](https://gstreamer.freedesktop.org/documentation/application-development/basics/bins.html) which will received JPEG-encoded frames from the R10C via an `appsrc`.

    Strings are joined together with newlines. Can use `{save_path}` as a placeholder for the timestamped save path.
- `livestream.custom` (object, optional):
  - `livestream.custom.save_path` (string): Path where videos from the custom
    pipelines should be saved. The plane system will automatically create a
    folder named after the current time inside of this path and save videos
    here.
  - `livestream.custom.pipelines` (map from string to array of string): Each key
    is the name of a pipeline that can be started at runtime, and each value is
    a GStreamer [pipeline
    description](https://gstreamer.freedesktop.org/documentation/tools/gst-launch.html?gi-language=c#pipeline-description).

    Example:
    ```
    "livestream": {
      "custom": {
        "save_path": "./videos/",
        "pipelines": {
          "keem": [
            "v4l2src device=\"/dev/video0\" ! videoconvert ! x264enc ! mp4mux ! filesink={save_path}/out.mp4"
          ]
        }
      }
    }
    ```

    At runtime, you can enter into the plane system:
    ```
    ps> livestream start keem
    ps> livestream stop keem
    ```
