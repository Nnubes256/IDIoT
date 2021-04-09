use anyhow::{bail, Context, Result};
use futures::stream::FuturesUnordered;
use libp2p_request_response::ResponseChannel;
use mpsc::TryRecvError;

use pin_project::pin_project;
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    sync::{mpsc, Arc},
    thread::JoinHandle as SyncJoinHandle,
    time::Duration,
};

use diot_core::device::{
    ActuationRequest, ActuationRequestData, ActuationResult, ActuatorResponseChannel,
    ActuatorValue, HardwareDevice, HardwareDeviceType, Measurement, SystemBridge,
};
use tokio::{
    select,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    task::JoinHandle,
};

use crate::{store::LocalPeerDevice, swarm::RemoteActuationResponse, system::LocalPeerData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSensorData {
    pub(crate) device: String,
    pub(crate) sensor_name: String,
    pub(crate) value: Measurement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullActuatorData {
    pub(crate) device: String,
    pub(crate) actuator_name: String,
    pub(crate) data: ActuatorValue,
}

impl FullActuatorData {
    pub fn into_local_data(self) -> ActuationRequestData {
        ActuationRequestData::new(self.actuator_name, self.data)
    }
}

pub enum SupervisorOutEvent {
    SensorData(FullSensorData),
}

#[pin_project]
pub struct HardwareSupervisor {
    hw_threads: HashMap<String, Arc<HardwareThread>>,
    device_meta: HashMap<String, LocalPeerDevice>,
    pub(crate) device_inbox: UnboundedReceiver<SupervisorOutEvent>,
    device_inbox_sender: UnboundedSender<SupervisorOutEvent>,
    pub(crate) inflight_requests: FuturesUnordered<JoinHandle<()>>,
}

impl HardwareSupervisor {
    pub fn from_peer_data(peer_data: LocalPeerData) -> Self {
        let num_devices = peer_data.devices.len();
        let mut device_meta = HashMap::with_capacity(num_devices);
        for (name, device) in peer_data.devices {
            info!(
                "Registering peripheral \"{:?}\" with name \"{}\"",
                device.device_type, name
            );
            device_meta.insert(name, device);
        }
        let (device_inbox_sender, device_inbox) = unbounded_channel();

        Self {
            hw_threads: HashMap::with_capacity(num_devices),
            device_meta,
            device_inbox,
            device_inbox_sender,
            inflight_requests: FuturesUnordered::new(),
        }
    }

    pub async fn start_devices(&mut self) -> Result<()> {
        for (name, device) in &self.device_meta {
            info!("Starting peripheral thread for device \"{}\"", name);
            let hw_thread = HardwareThread::new(
                name.clone(),
                device.device_type,
                device.config.clone(),
                self.device_inbox_sender.clone(),
            )
            .await
            .context("Failed to start device")?;
            self.hw_threads.insert(name.clone(), Arc::new(hw_thread));
        }

        info!("All devices started");

        Ok(())
    }

    pub fn actuate_device_local(
        &self,
        actuation_data: FullActuatorData,
        chan: oneshot::Sender<Result<ActuationResult>>,
    ) -> Option<()> {
        let device = self.device(&actuation_data.device)?;

        let task = tokio::spawn(async move {
            let result = device
                .try_actuate_device(actuation_data.into_local_data())
                .await
                .context("Actuation request failed on in-flight actuation task");

            if chan.send(result).is_err() {
                error!(
                    "Local response channel was closed while trying to send a response through it"
                )
            };
        });

        self.inflight_requests.push(task);

        Some(())
    }

    pub fn actuate_device_remote(
        &self,
        actuation_data: FullActuatorData,
        chan: ResponseChannel<RemoteActuationResponse>,
    ) -> Option<()> {
        let device = self.device(&actuation_data.device)?;

        let task = tokio::spawn(async move {
            let result = device
                .try_actuate_device(actuation_data.into_local_data())
                .await;

            let result = match result {
                Ok(act_res) => act_res,
                Err(err) => ActuationResult::ActuatorError {
                    error_code: -500,
                    error_description: err.to_string(),
                },
            };

            if chan.send_response(result.into()).is_err() {
                error!(
                    "Remote response channel was closed while trying to send a response through it"
                );
            }
        });

        self.inflight_requests.push(task);

        Some(())
    }

    pub fn device(&self, device_name: &str) -> Option<Arc<HardwareThread>> {
        self.hw_threads.get(device_name).cloned()
    }
}

enum SystemMessage {
    ActuationRequest(ActuationRequest<LocalResponseChannel>),
}

impl SystemMessage {
    pub fn actuation_request(
        actuation_data: ActuationRequestData,
    ) -> (Self, oneshot::Receiver<ActuationResult>) {
        let (response_sender, receiver) = LocalResponseChannel::new();
        (
            Self::ActuationRequest(ActuationRequest::new(actuation_data, response_sender)),
            receiver,
        )
    }
}

enum HardwareMessage {
    SensorData { name: String, value: Measurement },
}

pub struct HardwareThread {
    _task: JoinHandle<Result<()>>,
    outbox: UnboundedSender<SystemMessage>,
    _device_type: HardwareDeviceType,
    _config: serde_json::Value,
}

impl Debug for HardwareThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HardwareThread")
    }
}

impl HardwareThread {
    pub async fn new(
        name: String,
        device_type: HardwareDeviceType,
        config: serde_json::Value,
        supervisor_tx: UnboundedSender<SupervisorOutEvent>,
    ) -> Result<Self> {
        use diot_core::device::initialize_device;
        let (outbox, mut outbox_in) = unbounded_channel();
        let task = {
            let config = config.clone();
            let dev_name = name;
            tokio::spawn(async move {
                let (outbox_inner_dev, mut outbox_inner) = unbounded_channel();
                let (mut inbox_inner, inbox_inner_dev) = mpsc::channel();
                // Initialize device
                let device = initialize_device(device_type, config.clone())
                    .expect("Failed to initialize device");
                let mut task = Some(Self::spawn_inner(device, inbox_inner_dev, outbox_inner_dev));
                loop {
                    select! {
                        in_msg = outbox_in.recv() => {
                            match in_msg {
                                Some(msg) => if let Err(err) = inbox_inner.send(msg) {
                                    error!("Error while sending message to peripheral thread: {}", err);
                                }
                                None => {
                                    if let Err(err) = task.take().expect("No thread?!").join().expect("Joining on task failed!") {
                                        error!("Error on peripheral thread for device of type {:?}: {}", device_type, err);

                                        let (outbox_inner_dev, outbox_inner_n) = unbounded_channel();
                                        let (inbox_inner_n, inbox_inner_dev) = mpsc::channel();

                                        outbox_inner = outbox_inner_n;
                                        inbox_inner = inbox_inner_n;

                                        // Reinitialize device
                                        let device = initialize_device(device_type, config.clone()).expect("Failed to initialize device");
                                        task = Some(Self::spawn_inner(device, inbox_inner_dev, outbox_inner_dev));
                                    }
                                }
                            }
                        }
                        Some(msg) = outbox_inner.recv() => {
                            match msg {
                                HardwareMessage::SensorData { name, value } => {
                                    let out_ev = SupervisorOutEvent::SensorData(FullSensorData{
                                        device: dev_name.clone(),
                                        sensor_name: name.clone(),
                                        value
                                    });
                                    if let Err(err) = supervisor_tx.send(out_ev) {
                                        error!("Error while sending hardware message to supervisor: {}", err);
                                    }
                                }
                            };
                        }
                    }
                }
            })
        };
        Ok(Self {
            _task: task,
            _device_type: device_type,
            outbox,
            _config: config,
        })
    }

    pub async fn try_actuate_device(
        &self,
        actuation_data: ActuationRequestData,
    ) -> Result<ActuationResult> {
        let (message, response_chan) = SystemMessage::actuation_request(actuation_data);

        if self.outbox.send(message).is_err() {
            bail!("Failed to send message to device's supervisor thread");
        }

        let response = response_chan
            .await
            .context("Failed to receive message from device's supervisor thread")?;

        Ok(response)
    }

    fn spawn_inner(
        device: Box<dyn HardwareDevice>,
        inbox: mpsc::Receiver<SystemMessage>,
        outbox: UnboundedSender<HardwareMessage>,
    ) -> SyncJoinHandle<Result<()>> {
        std::thread::spawn(move || hardware_thread(device, inbox, outbox))
    }
}

pub struct LocalResponseChannel {
    channel: oneshot::Sender<ActuationResult>,
}

impl LocalResponseChannel {
    pub fn new() -> (Self, oneshot::Receiver<ActuationResult>) {
        let (sender, receiver) = oneshot::channel();
        (Self { channel: sender }, receiver)
    }
}

impl ActuatorResponseChannel for LocalResponseChannel {
    fn send(self, response: ActuationResult) {
        if self.channel.send(response).is_err() {
            error!("Actuator response channel was closed before trying to send a response to it");
        }
    }
}

struct DiodtSystemBridge {
    inbox: mpsc::Receiver<SystemMessage>,
    outbox: UnboundedSender<HardwareMessage>,
    in_actuation_queue: VecDeque<ActuationRequest<LocalResponseChannel>>,
}

impl DiodtSystemBridge {
    fn new(inbox: mpsc::Receiver<SystemMessage>, outbox: UnboundedSender<HardwareMessage>) -> Self {
        Self {
            inbox,
            outbox,
            in_actuation_queue: VecDeque::new(),
        }
    }

    fn collect_all_messages(&mut self) -> Result<()> {
        loop {
            match self.inbox.try_recv() {
                Ok(msg) => match msg {
                    SystemMessage::ActuationRequest(req) => {
                        self.in_actuation_queue.push_back(req);
                    }
                },
                Err(e) if e == TryRecvError::Empty => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl SystemBridge for DiodtSystemBridge {
    type ActuatorRequestOutChannel = LocalResponseChannel;

    fn write_sensor_data(&mut self, name: String, value: Measurement) {
        match self
            .outbox
            .send(HardwareMessage::SensorData { name, value })
        {
            Ok(_) => {}
            Err(err) => error!("Error while sending sensor data: {}", err),
        }
    }

    fn actuator_request_next(
        &mut self,
    ) -> Result<Option<ActuationRequest<Self::ActuatorRequestOutChannel>>> {
        Ok(self.in_actuation_queue.pop_front())
    }
}

fn hardware_thread(
    mut device: Box<dyn HardwareDevice>,
    inbox: mpsc::Receiver<SystemMessage>,
    outbox: UnboundedSender<HardwareMessage>,
) -> Result<()> {
    let mut bridge = DiodtSystemBridge::new(inbox, outbox);

    debug!("Entered hardware thread");
    loop {
        bridge
            .collect_all_messages()
            .context("Error while collecting all messages")?;

        let mut sensor_collector = bridge.sensor_collector();
        device
            .sense(&mut sensor_collector)
            .context("Device returned error while sensing")?;

        let actuator_provider = bridge.actuator_provider();

        for request in actuator_provider {
            let request = request.expect("an available request");
            let request_data = request.data();
            let response = device.actuate(request_data);
            request.send_answer(response);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
