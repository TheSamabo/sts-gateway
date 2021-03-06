use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use crate::storage::Insert;
use crate::{definitions::{AggregatorAction, TransportAction}, storage::SqliteStorageAction};
use chrono::Utc;
use serde_json::json;

// TODO:
// Collect statistics about amount of datapoints per channel
// or somethings like how fast channel reads x number of devices
// Any statistics would be nice to have.
pub struct Aggregator {
    aggregator_rx: Receiver<AggregatorAction>,
    storage_tx: Sender<SqliteStorageAction>,
    transport_tx: Sender<TransportAction>
}

impl Aggregator {
    pub fn new(
        aggregator_rx: Receiver<AggregatorAction>,
        storage_tx: Sender<SqliteStorageAction>,
        transport_tx: Sender<TransportAction>
    ) -> Self {
        Self {
            aggregator_rx,
            storage_tx,
            transport_tx
        }
    }

    pub fn run(self) -> JoinHandle<()>{
        thread::spawn(move || {
            loop {
                match self.aggregator_rx.recv() {
                    Err(e) => {
                        log::error!("Error aggregator channel: {:?}", e)
                    },
                    Ok(action) => {
                        match action {
                            AggregatorAction::SendBoth(attributes, timeseries) => {
                                // Struct 
                                // Generate TS NOW...
                                let ts: i64 = Utc::now().timestamp_millis();
                                
                                // Take device name from either of the attributes or timeseries
                                let device_name = timeseries.0;

                                let timeseries_message = json!({
                                    &device_name: timeseries.1
                                }).to_string();

                                let attributes_message = json!({
                                    &device_name: attributes.1
                                }).to_string();
                                log::trace!("JSON Attributes: {:?}", attributes_message);
                                log::trace!("JSON Timeseries: {:?}", timeseries_message);
                                
                                // This is unsafe as fuck
                                match self.storage_tx.send(SqliteStorageAction::InsertBoth(Insert{
                                    ts,
                                    device_name,
                                    timeseries_message: Some(timeseries_message.clone()),
                                    attributes_message: Some(attributes_message.clone())
                                // String
                                })) {
                                    Ok(_) => log::trace!("Sent messages: {} and {} to storage!", attributes_message, timeseries_message),
                                    Err(e) => {
                                        log::error!("Error sending messages: {} and {} to SqliteStorage!... {:?}", attributes_message, timeseries_message, e)
                                    }
                                }
                                // As are these...
                                match self.transport_tx.send(TransportAction::SendTimeseries(timeseries_message.clone())) {
                                    Ok(_) => log::debug!("SentTimeseries to transport with message: {}", timeseries_message),
                                    Err(e) => log::error!("Error while sending a message to trasport channel: {:?}",e)
                                };
                                match self.transport_tx.send(TransportAction::SendAttributes(attributes_message.clone())) {
                                    Ok(_) => log::debug!("SentAttributes to transport with message: {}", attributes_message),
                                    Err(e) => log::error!("Error while sending a message to trasport channel: {:?}",e)
                                };
                            },
                            // AggregatorAction::SendStatistics(stats) => {}
                        };
                    }
                }
            }
        })
    }
}