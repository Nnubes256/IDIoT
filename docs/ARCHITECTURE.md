# Architecture

![System architecture](architecture.png?raw=true "System architecture")

Each node on the network is built from a Raspberry Pi Zero WH (i.e., with wireless networking and GPIO headers pre-installed). Its small form factor allows to keep both size and cost down for the overall setup, thus further enabling the consumer’s capacity to deploy multi-node setups. The device runs Raspbian on top as its operating system, chosen because its natural compatibility with the wider Raspberry Pi ecosystem.

The central part of the system is provided on the dIoT software package developed for this project. This package has two parts:
- The `diot-core` component, encompassing a series of device drivers that allow the node to interact with its local peripherals, whether they are sensors, actuators, or both. These drivers are abstracted on top of a common, purpose-specific API, which allows to add support for new peripherals into the system in a standardized way.
- The `diotd` service, a program intended to run as a service or daemon on the node. Integrating diot-core as a statically linked library, the diotd daemon implements the bulk of the system objectives and connects the data and capabilities of the peripherals to the wider node network. It consists of the following components:
    - An implementation for a device supervisor, which allows to setup, monitor, and control an arbitrary number of peripherals (within hardware constraints) defined within a configuration file.
    - A swarm network manager, responsible of discovering and communicating with the wider node network in a secure and efficient way.
    - A control layer, capable of responding to sensor events with actuation requests through the evaluation of rules defined within a configuration file, allowing the user basic automated control of their devices.
    - An integrated web server that serves a frontend, allowing the user to access the sensor data of the whole node network from any of the nodes in real-time.
    - Finally, a system core that integrates together and coordinates all the above components and provides a local store of the data produced by the system.

Three key libraries are used:
- Tokio, an asynchronous programming library for Rust that allows to write fast, concurrent applications. It provides a runtime on which asynchronous tasks can run, optimized towards I/O-bound operations; allowing, among other things, performant network applications to be built on top of it. This component is a pre-requisite for the rest of the components.
- Warp, a web server framework for Rust, which serves the frontend and the data provided to it. It allows to achieve instant feedback of data thanks to its support for WebSockets, which allows real-time communication from the webserver to the browser.
- libp2p, a framework for developing peer-to-peer applications. This framework is the basis, among other things, of the IPFS network. It provides a series of building blocks, tied together through a common specification, that allow developers to kick-start applications that communicate in a peer-to-peer basis with other devices. This component is the basis of the decentralized capabilities of the proposed system.

Additionally, the frontend itself is built using the Vue.js JavaScript framework. This framework was chosen mainly for the author’s interest to give a proving ground for its capabilities. No further libraries were used on the frontend side, aside from HTML5 standard capabilities such as the WebSocket client.

## Device abstraction

To accommodate the usage of various kinds and models of peripherals, the system must be able to individually distinguish between them and interact with them according to their specifications. To overcome this requirement, a generic device abstraction can be defined which the different kinds of device drivers implement, and which a device supervisor can use to setup and manage a specific device. This allows the device supervisor to be unaware of the specific workings of the devices; such details are simply abstracted away. 

![Device architecture](architecture_device.png?raw=true "Device architecture")

Device drivers implement a `HardwareDevice` trait. Thus, the device driver’s instance is treated under a generic type common to all device drivers. Consequently, the common functionality on such instances can be invoked without needing to know further information about the device driver itself. This common trait implemented for all device drivers represents the device API on the figure above.

The HardwareDevice trait defines three operations: 
- **Initialization:** this allows to initialize the device driver itself, provided a configuration (e.g., GPIO pin, etc.). A sister trait, `ConfigurableHardwareDevice`, has provisions to define any custom configuration structure the device driver may want to receive, the only requirement for it being that it can be constructed through deserialization of the format being used in the node’s configuration file.
- **Sensing:** when called, the driver may request sensor data from the hardware device and publish it to the node itself. This is done by writing the read sensor data into a "collector" passed as an argument. The collector allows to differentiate between different sensors in a device by specifying the name of the device’s sensor which data is being published to. For example, a DHT11 sensor can sense both temperature and humidity; thus, both are published under different sensor names, but in the context of the same device.
- **Actuation:** this operation is invoked whether the node wants to trigger a given actuator within the device. The specific actuator to trigger is specified by name, and a "actuation value" is provided which the device driver is free to interpret according to the actuation capabilities it desires to provide. For the scope of this project, a value may be any of signed integer, double-precision floating-point number, UTF-8 string, and “signal” (an empty value that implies that the actuation request does not specify any information other than the request to actuate the given device).

## Peer-to-peer node communication

As previously described, communication among nodes within the network is handled by the libp2p library, which provides a series of building blocks to do this task. For this project, the composing of these building blocks has resulted in the network stack pictured below.

![Peer-to-peer architecture](architecture_p2p.png?raw=true "Peer-to-peer architecture")

This network stack has several key components, all of them provided by the libp2p framework itself. First off, nodes discover each other on the local network using the mDNS protocol. This allows automatic self-organization of the network without user intervention beyond first-time configuration. Once a pair of nodes discovers each other, they will be able to establish a connection between each other.

Each connection is first protected by an encryption layer governed by a pre-shared key, to be manually shared among all peers through a configuration file. This encryption layer protects the network itself from trivial access to it by unauthorized actors which do not know the pre-shared key.

The transport upgrade layer of the libp2p specification then begins its operation. This layer provides a standardized way for the connection itself to be able to further “upgrade” to the next layers in the stack.

Next, the connection is further secured using a public-key, Diffie-Hellman cryptographic system known as the Noise protocol framework. The purpose of this layer is purely to authenticate and protect the connection between two nodes; this avoids trivial man-in-the-middle attacks on the individual connections. The cryptographic keypair material used to protect the communications within this and the above layers is pre-generated on the node’s first boot and stored in a configuration file. It is thus to be noted that any given node’s identity can be safely defined and determined entirely from its public key or, more specifically for the libp2p platform, a hash of such public key; this hash is known as the peer ID [11].

Once encrypted, another layer provides stream multiplexing, allowing multiple different protocols to simultaneously use the connection to communicate with each other. This layer now gives way for more specialized protocols answering application-specific needs.

### Connection keep-alive
A “ping” protocol is used to keep connections alive. This potentially reduces the overall communication latency between nodes in the system by avoiding the latency inherent to establishing and securing a new connection.

### Pubsub-based message routing
To disseminate sensor measurements and node metadata among the nodes on the network, a publish-subscribe message routing protocol known as Gossipsub is used. Gossipsub is a smart protocol for implementing publish-subscribe systems that can spread messages across interconnected device meshes. It attempts, among other things, to balance the overall load of the entire network across its nodes equally, avoiding overloading intermediate nodes with excessive load from relaying messages.

As indicated, Gossipsub serves two functions: to broadcast sensor measurements generated by local peripherals to the wider network, indicating which device and which sensor were used along with the collected measurement; and to broadcast metadata about the participating nodes to the wider system, including a display name set by the user and the information regarding the registered local devices for each node.

### Request-response messaging

To allow nodes to ask their peers to trigger their local actuation devices remotely, a request-response protocol is provided. While the mechanics of requests and responses themselves are handled by libp2p itself, the wire protocol itself is application-specific, and thus left to the developer. For such, the data structures below are defined. `FullActuatorData` is sent as the request part of the communication, whereas `RemoteActuationResponse` is sent as the response part of the communication.

```rust
enum ActuatorValue {
    // can be one, and only one, of:
    Signal,
    Unsigned(u64),
    Signed(i64),
    Double(f64),
    String(String),
}

struct FullActuatorData {
    // is composed of:
    /// Device name
    device: String,
    /// Actuator within the specific device
    actuator_name: String,
    /// Actuation parameter
    data: ActuatorValue,
}

enum RemoteActuationResponse {
    // can be one, and only one, of:
    Success,
    Ignored,
    NoResponse,
    BadRequest {
        reason: String,
    },
    ActuatorError {
        error_code: i64,
        error_description: String,
    },
}
```

The wire protocol is then defined as the transcoding of these structures from and into binary data using the Bincode binary format and corresponding third-party software library.

### Control layer
Automation is done through the definition of rules. Such rules describe an action to take, namely, an actuation requested to a specific device on the network with specific parameters, in presence of measurements generated by a given sensor on the network that match a condition. One such rule may be, for example, `IF sensor_1.humidity > 80% THEN buzzer_1.beep(100ms)`, meaning, if a humidity measurement received from `sensor_1` exceeds a value of 80%, actuate `buzzer_1` for 100 milliseconds. Multiple of these rules may be defined.
