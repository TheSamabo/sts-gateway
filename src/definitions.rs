
use std::collections::HashMap;
use serde::{Serialize, Deserialize};


use clap::Parser;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct MainArguments {
    /// Required file to startup the gateway
    pub root_config: String
}
#[derive(Serialize, Deserialize)]
pub struct OneTelemetry {
    pub ts: i64,
    // Key/Value
    pub values: HashMap<String, String>
}

pub type AttributeMessage = (String, HashMap<String, String>);

// String: device_name
// Vec: Vec<{ts: i64, value: }>
pub type TimeseriesMessage = (String, Vec<OneTelemetry>);


#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MainConfig {
    pub name: String,
    pub log_config: String,
    pub channels: Vec<ChannelDefinition>,
    pub storage: Storage,
    pub mqtt: MqttConfig
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum Storage {
    #[serde(rename = "sqlite")]
    Sqlite {data_folder: String, size_management: StorageSizeManagement, backup_management: StorageBackupManagement} 

}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum StorageSizeManagement {
    #[serde(rename = "fixed_window")]
    FixedWindow {messages_ttl_check: String, messages_ttl: i32}
}
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum StorageBackupManagement {
    #[serde(rename = "local")]
    Local {backup_folder: String, backup_interval: String, backup_ttl: i32}
}


#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
// #[serde(untagged)]
pub enum ChannelType {
    #[serde(rename = "modbus_tcp")]
    ModbusTcp,
    #[serde(rename = "modbus_rtu")]
    ModbusRtu
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
// #[serde(tag = "type")]
pub struct ChannelDefinition {
    #[serde(rename = "type")]
    pub _type: ChannelType,
    pub file: String
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MqttConfig {
    pub client_id: Option<String>,
    pub host: String,
    pub port: u16,
    pub qos: u8,
    pub tb_token: Option<String>
}

// These are actions that this gateway sends to any transport medium eg: thingsboard server
pub enum TransportAction {
    SendTimeseries(String), // Already parsed json string
    SendAttributes(String),
    // SendClientSideRPC
}

// pub struct DataCombined {
//     attribute_message: Option<AttributeMessage>,
//     timeseries_message: TimeseriesMessage
// }
pub enum AggregatorAction {
    SendBoth(AttributeMessage, TimeseriesMessage),
    // SendAttributes(AttributeMessage),
    // SendTimeseries(TimeseriesMessage),
    // SendStatistics(AttributeMessage) // will store some statistics in device attributes
}
// pub struct RootConfig {
    
// }