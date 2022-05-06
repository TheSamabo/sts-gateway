use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;
use std::thread;
use std::time::Duration;
use crate::definitions::{MqttConfig, TransportAction, MainConfig};
use crate::storage::SqliteStorageAction;

const TB_DEVICE_ATTRIBUTES_TOPIC: &str = "v1/gateway/attributes";
const TB_DEVICE_TELEMETRI_TOPIC: &str = "v1/gateway/telemetry";
const TB_DEVICE_CONNECT_TOPIC: &str = "v1/gateway/connect";

use rumqttc::{Client, MqttOptions, QoS};

pub struct MqttTransport {
    pub config: MainConfig,
    pub storage_tx: Sender<SqliteStorageAction>,
    pub transport_rx: Receiver<TransportAction>
}


impl MqttTransport {
    pub fn new(
            config: MainConfig,
            storage_tx: Sender<SqliteStorageAction>,
            transport_rx: Receiver<TransportAction>
        ) -> Self {
        Self {
            config,
            storage_tx,
            transport_rx
        }
    }
    pub fn run(self) -> JoinHandle<()> {

        let qos = match self.config.mqtt.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::ExactlyOnce,
            _ => QoS::AtLeastOnce,
        };
        
        let mut options = MqttOptions::new(
            self.config.name,
                self.config.mqtt.host,
                self.config.mqtt.port    
        );
        // Tb token auth
        if let Some(token) = self.config.mqtt.tb_token {
            options.set_credentials(token, String::from(""));
        }

        let handle = thread::spawn(move || {
            let (mut client, mut connection) = Client::new(options, 10);
            // TODO: Implement other topics to use eg: RPC request topics
            let mut client_clone = client.clone();
            thread::spawn(move || {
                log::info!("Ready to accept TransportActions!");
                loop {
                    match self.transport_rx.recv().unwrap() {
                        TransportAction::SendTimeseries(telemetry) => {
                            match client.publish(TB_DEVICE_TELEMETRI_TOPIC, qos, false, telemetry.as_bytes()) {
                                Ok(_) => log::trace!("Message successfuly sent!"),
                                Err(e) => log::error!("Error sending message: {:?} on topic: {}, Error: {:?}",
                                    telemetry,TB_DEVICE_TELEMETRI_TOPIC, e)
                            }
                        },
                        TransportAction::SendAttributes(attributes) => {
                            match client.publish(TB_DEVICE_ATTRIBUTES_TOPIC, qos, false, attributes.as_bytes()) {
                                Ok(_) => log::trace!("Message successfuly sent!"),
                                Err(e) => log::error!("Error sending message: {:?} on topic: {}, Error: {}",
                                attributes ,TB_DEVICE_ATTRIBUTES_TOPIC, e)
                            }
                        },
                        _ => {}
                    }
                }
            });
            for (i, notification) in connection.iter().enumerate() {
                match notification {
                    Ok(e) => log::trace!("Notification = {:?}", e),
                    Err(error) => {
                        log::error!("Mqtt Error: {:?}", error);
                        match client_clone.disconnect() {
                            Ok(_) => log::info!("Send mqtt disconnect!"),
                            Err(e) => log::error!("Error sending mqtt disconnect: {:?}",e)
                        };
                        thread::sleep(Duration::from_secs(5))
                    }

                };
            }
        });

        handle
    }
}