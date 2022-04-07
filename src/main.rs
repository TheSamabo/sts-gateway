

use chrono_tz::Europe::Bratislava;
use definitions::MainConfig;
use log4rs::config::{Appender, Root};
use rusqlite::{Connection, Result as SqliteResult, OpenFlags, params, backup, Error as SqliteError};
use rusqlite::NO_PARAMS;
use serde_yaml;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::{thread, fs};
use std::sync::{mpsc};
use log4rs::{self, Config};
use log::LevelFilter;
use std::path::{self, PathBuf, Path};
use chrono::{self, DateTime, Utc, TimeZone, NaiveDateTime};
use chrono_tz;

use tokio_modbus::prelude::*;

use crate::aggregator::Aggregator;
use crate::channels::{ChannelConfig, Channel};
use crate::channels::modbus::{ModbusClientTcpConfig, ModbusRegisterMap, ModbusSlave};
use crate::channels::modbus::tcp::ModbusTcpChannel;
use crate::definitions::{TransportAction, AggregatorAction};
use crate::transport::MqttTransport;

mod storage;
mod definitions;
mod transport;
mod channels;
mod utilities;
mod aggregator;

// use transport::MqttTransport;
// This will hold a hash of contents of the file, when we will periodicaly read configuration at runtime 
// we will compare the hashes and determine which part of the gateway to reload


// #[tokio::main]
fn main() {

    let mut state = MainState::new();
    
    let config_path = state.read_file("./dist/sts_gateway.yml".to_string()).unwrap();
    let config: definitions::MainConfig = serde_yaml::from_str(&config_path).unwrap();
    let backup_config = config.clone();
    log4rs::init_file(config.log_config.clone(), Default::default()).unwrap();
    log::info!("Starting...");
    log::debug!("Config: {:?}", config.clone()); 

    log::debug!("Loaded Configs with their hashes: {:?}", state.get_configured_hashes());

    let (storage_tx, storage_rx) = mpsc::channel::<storage::SqliteStorageAction>();
    let config_clone = config.clone();
    let storage_handle = thread::spawn(move || {
        log::info!("Starting storage thread...");
        let config = config_clone;

        // HARD CODED PATH  --- TODO: CHANGE
        let mut storage = storage::SqliteStorage::new(config.data_folder, storage_rx).unwrap();

        loop {
            storage.process();
        }
    });

    let (transport_tx, transport_rx) = mpsc::channel::<TransportAction>();

    let transport = MqttTransport::new(config.clone(), storage_tx.clone(), transport_rx);
    let transport_handle = transport.run();
    // Spawn a Aggregation Channel
    // brief This Sender part of the MPSC will be dispatched to every channel
    // so that it can send data to aggregation Thread 
    let (aggregation_tx, aggregation_rx) = mpsc::channel::<AggregatorAction>();
    let aggregator = Aggregator::new(aggregation_rx, storage_tx.clone(), transport_tx.clone());
    let aggregator_handle = aggregator.run();

    let modbus_raw = state.read_file("./dist/modbus.yml".to_string()).unwrap();
    let modbus_config = ModbusClientTcpConfig::serialize(modbus_raw).unwrap();

    let mut register_maps: HashMap<ModbusSlave, ModbusRegisterMap> = HashMap::new();
    for slave in modbus_config.slaves.clone() {
       let register_map_raw = state.read_file(slave.register_map.clone()).unwrap(); 
       let register_map = serde_yaml::from_str::<ModbusRegisterMap>(&register_map_raw).unwrap();
        register_maps.insert(slave, register_map);
    }
    let modbus_channel = ModbusTcpChannel::new(modbus_config, register_maps, aggregation_tx.clone());
    let modbus_handle = modbus_channel.run();

    let mqtt_config = config.mqtt.clone();


    // Start MPSC channel that we can pass wto storage thread

    // Start new storage thread
    backup_db_scheduler(storage_tx.clone(), backup_config).join().unwrap();
    
    aggregator_handle.join().unwrap();
    transport_handle.join().unwrap();
    storage_handle.join().unwrap();
    modbus_handle.join().unwrap();
}


// TODO: 
fn backup_db_scheduler(storage_tx: Sender<storage::SqliteStorageAction>, config: MainConfig) -> JoinHandle<()> {
    thread::spawn(move || {

        let mut scheduler = job_scheduler::JobScheduler::new();

        let job = job_scheduler::Job::new(
            job_scheduler::Schedule::from_str("1 10/1 * * * *").unwrap(), || {

            let datetime = Utc::now();
            let datetime = datetime.with_timezone(&Bratislava);

            let date_string = datetime.format("_%F_%X.db").to_string();
            let mut backup_name = config.name.replace(" ", "_").to_lowercase();
            backup_name.push_str(&date_string);
            log::trace!("Picked a backup name: {}", backup_name);

            storage_tx.send(storage::SqliteStorageAction::BackupDB(backup_name));
        });

        scheduler.add(job);

        loop {
            scheduler.tick();
            thread::sleep(Duration::from_millis(250))
        }
    })
    

    
}



pub struct MainState {
    configured_hashes: HashMap<String,String>
}

impl MainState {
    pub fn new() -> Self {
        Self {
            configured_hashes: HashMap::new()
        }
    }

    pub fn read_file(&mut self, path: String) -> Result<String, std::io::Error> {
        let (data, hash ) = utilities::open_and_read(path.clone())?;
        self.configured_hashes.insert(path, hash);
        Ok(data)
    }
    pub fn get_configured_hashes(&self) -> &HashMap<String,String> {
        &self.configured_hashes
    }
}
// fn read_configs() -> 