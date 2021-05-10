# Integrated Decentralized IoT

## Setup instructions

You need the following hardware:

- One or more Raspberry Pi 3/4/ZeroW devices.
- Some way to power the above.
- Any/Some of the supported sensors:
    - `dht11`: DHT11 temperature and humidity sensor.
- Any/Some of the supported actuators:
    - `buzzer`: active buzzer.
- A WiFi network which allows mDNS requests.

To build the software, you need:

- Docker installed and running.
- A working Rust toolchain (check out [https://rustup.rs/](`rustup`)).
    - The project works on the `stable` channel of Rust, so you can simply stick with defaults.
- The [https://github.com/rust-embedded/cross](`cross`) tool.
    - Once you have a Rust toolchain installed, run: `cargo install cross`.

Once this is done, clone this repo, and run the following in a terminal with access to your Docker daemon:
    
  * For Raspberry Pi 3/4:

    ```sh
    cd path/to/repo     # Replace with path to your clone

    # Build the custom `cross` Docker container
    docker build -t idiot/cross:armv7-unknown-linux-gnueabihf-0.2.1 -f scripts/Dockerfile.rpi34 scripts/

    # Cross-compile the daemon
    cargo cross build --target=armv7-unknown-linux-gnueabihf --release
    ```

  * For Raspberry Pi Zero W:

    ```sh
    cd path/to/repo     # Replace with path to your clone

    # Build the custom `cross` Docker container
    docker build -t idiot/cross:arm-unknown-linux-gnueabi-0.2.1 -f scripts/Dockerfile.rpi34 scripts/

    # Cross-compile the daemon
    cargo cross build --target=arm-unknown-linux-gnueabi --release
    ```

Once finished, the compiled binary will be located in `target/release/diotd`. Copy this binary to your device. The binary can then be run directly:

```sh
sudo ./diotd
```

## Configuration example

Put in the working directory where the application will be run as `config.json`.

**Note:** make sure to remove the comments before running!

```javascript
{
  // Peer data
  "peer": {
    // Peer display name; can be changed anytime
    "name": "test1",

    // Local devices
    // key: device display name
    // value: device config
    "devices": {
      "timer-1": {
        // device type; determines the driver to load
        // for this device
        "device_type": "timer",

        // device-specific configuration
        // for "dht11" and "buzzer", the only configuration setting is "pin",
        // which defines the GPIO pin to communicate with the device through.
        "config": {
          "tick_every_ms": 5000
        }
      },
      "logger-1": {
        "device_type": "logger",
        "config": {
          "prefix": "LOGGER AT DEVICE 1 REPORTED VALUE: ",
          "signal": "LOGGER AT DEVICE 1 REPORTED SIGNAL"
        }
      }
    }
  },
  // Web interface settings
  "web": {
    // Port where the web interface will be served on
    "port": 3030
  },

  // Automation rules
  "rules": [
    {
      // Sensor whose measurements to listen to
      "sensor": {
        // Node peer ID (only if listening to remote node; otherwise remove this field)
        // You can get this on the logs when starting the node
        "node": "12D3KooWFXaCkMq86H2pYN9kTB9qr6XqCwtumbXRTKt8YcqM8cv4",
        
        // Device name
        "device": "timer-2",

        // Name of sensor within the device to listen to
        "sensor_name": "tick"
      },

      // Condition
      "on": {
        // Condition type
        // Supported condition types:
        // - `any`: matches on any received measurement ("value" is not required in this case)
        // - `equal`: matches on measurement equal to "value"
        // - `greater_than`: matches on measurement greater than "value"
        // - `less_than`: matches on measurement greater than "value"
        // - `greater_or_equal_than`: matches on measurement equal or greater than "value"
        // - `less_or_equal_than`: matches on measurement equal or less than "value"
        // Examples of "value" are below; you can uncomment to use.
        "operation": "any"
        //"value": "signal"
        //"value": {
        //  "integer": 12
        //}
        //"value": {
        //  "double": 4.2
        //}
        //"value": {
        //  "string": "texttexttexttext"
        //}
      },

      // Actuator to actuate if the condition matches
      "then": {
        // Node peer ID (only if actuating a remote device; otherwise remove this field)
        // You can get this on the logs when starting the node
        "node": "12D3KooWFXaCkMq86H2pYN9kTB9qr6XqCwtumbXRTKt8YcqM8cv4",

        // Device name
        "device": "logger-2",

        // Name of actuator within the device to actuate
        "actuator_name": "ticker",

        // Actuation parameters. If none, just "signal"; otherwise,
        // a similar format to "value" on the condition, only that
        // instead of having "integer" you have "signed" and "unsigned".
        "data": "signal"
      }
    }
  ]
}
```

Once both nodes are configured, run the daemon once, then exit once started (`Ctrl+C`). The nodes won't connect to each other in this stage, keep that in mind.

The configuration will now contain a `secrets` section with the
`keypair` of the node (leave as-is) and a randomly-generated `psk` (pre-shared key). Take one of the `psk` values and overwrite the other with it, such that they both end up equal in both devices.

Run the software again; now the nodes should be able to discover and connect to each other. You may now also navigate to each device's web inteface through its configured web port.
