use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;
use std::thread;
use std::time::Duration;
use crate::definitions::{TransportAction, MainConfig};
use crate::storage::SqliteStorageAction;

const TB_DEVICE_ATTRIBUTES_TOPIC: &str = "v1/gateway/attributes";
const TB_DEVICE_TELEMETRI_TOPIC: &str = "v1/gateway/telemetry";
// const TB_DEVICE_CONNECT_TOPIC: &str = "v1/gateway/connect";

use paho_mqtt as mqtt;

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
            0 => 0_i32,
            1 => 1_i32,
            _ => 2_i32,
        };
        
        let client_options = mqtt::CreateOptionsBuilder::new()
            .client_id(self.config.name)
            .server_uri(format!("tcp://{}:{}", self.config.mqtt.host, self.config.mqtt.port))
            .max_buffered_messages(10000)
            .finalize();
        let mut connection_options = mqtt::ConnectOptionsBuilder::new();
        let _ = &connection_options.clean_session(true)
            .automatic_reconnect(Duration::from_secs(2), Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(3600))
            .keep_alive_interval(Duration::from_secs(15))
            .max_inflight(10);
            
        // Tb token auth
        if let Some(token) = self.config.mqtt.tb_token {
            connection_options.user_name(token);
        }
        let connection_options = connection_options.finalize();

        let handle = thread::spawn(move || {
            let client = mqtt::AsyncClient::new(client_options).unwrap_or_else(|e| {
                log::error!("Error creating MQTT Client: {:?}", e);
                panic!("Error creating MQTT Client: {:?}", e)
            });
            // TODO: Implement other topics to use eg: RPC request topics
            // let mut client_clone = client.clone();
            log::info!("Ready to accept TransportActions!");
            // if let Err(ct) = client.connect(connection_options).wait() {
            //     log::error!("Could not connect to mqtt broker: {:?}", ct);
            // }
            while let Err(ct) = client.connect(connection_options.clone()).wait() {
                log::error!("Could not connect to mqtt broker: {:?}", ct);
                thread::sleep(Duration::from_secs(10));
            }
            loop {
                // if !client.is_connected() {
                //     client.reconnect_with_callbacks(
                //         |_,_| {
                //         log::info!("Reconnected successfuly!");
                //     }, 
                //     |_c, _rc, e| {
                //         log::error!("Could not reconnect to broker... {:?}", mqtt::error_message(e));
                //     });
                // }
                match self.transport_rx.recv() {
                    Err(e) => log::error!("Receiver error: {:?}", e),
                    Ok(action) => match action {
                        TransportAction::SendTimeseries(telemetry) => {
                            let msg = mqtt::Message::new(TB_DEVICE_TELEMETRI_TOPIC, telemetry.as_bytes(), qos);
                            match client.publish(msg).wait() {
                                Ok(_) => log::debug!("Successfuly sent message!"),
                                Err(e) => log::error!("Error sending message: {:?}", e)
                            }

                        },
                        TransportAction::SendAttributes(attributes) => {
                            let msg = mqtt::Message::new(TB_DEVICE_ATTRIBUTES_TOPIC, attributes.as_bytes(), qos);
                            match client.publish(msg).wait() {
                                Ok(_) => log::debug!("Successfuly sent message!"),
                                Err(e) => log::error!("Error sending message: {:?}", e)
                            }
                        },
                    }
                }
            }
        });
            




        handle
    }
}