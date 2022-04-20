use std::{thread::JoinHandle, collections::HashMap};

use serde_yaml::Error;
use tokio_serial::DataBits;
use chrono::{Utc};

use crate::definitions::OneTelemetry;

pub mod modbus;


#[derive(Debug)]
pub enum ChannelStatus {
    Running,
    Stopped,
    Error,
}

pub trait ChannelConfig {
    fn serialize(config_string: String) -> Result<Self, Error> where Self: Sized;
}

// Todo:
// Implement some way to terminate/restart channels from
// something like a "ChannelController". This controller will
// hold status and restart specified channel if its configuration
// was changed at runtime
pub trait Channel {
    // Each channel has to serialize it into its special 
    fn run(self) -> JoinHandle<()>;
    fn status(&self) ->  ChannelStatus;
}

#[derive(Debug, Clone)]
pub struct DataPoint {
    pub key: String,
    pub value: String,
    pub ts: Option<i64>
}

impl From<Vec<DataPoint>> for OneTelemetry {
    /// Returns an OneTelemetry struct with ts that is
    /// taken when calling .into()
    fn from(input: Vec<DataPoint>) -> Self {
        let ts: i64 = Utc::now().timestamp_millis();

        let mut values: HashMap<String,String> = HashMap::new();
        for point in input {
            values.insert(point.key, point.value);
        }
        Self {
            ts,
            values
        }
    }
}