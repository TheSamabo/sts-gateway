

use chrono_tz::Europe::Bratislava;
use clap::Parser;
use definitions::MainConfig;
use serde_yaml;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::thread;
use std::sync::{mpsc};
use log4rs::{self};
use chrono::{self, Utc};
use chrono_tz;


use crate::aggregator::Aggregator;
use crate::channels::modbus::rtu::ModbusRtuChannel;
use crate::channels::{ChannelConfig, Channel};
use crate::channels::modbus::{ModbusClientTcpConfig, ModbusRegisterMap, ModbusSlave, ModbusClientRtuConfig};
use crate::channels::modbus::tcp::ModbusTcpChannel;
use crate::definitions::{TransportAction, AggregatorAction, ChannelType, Storage};
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


#[tokio::main]
async fn main() {

    // Read arguments if no arguments panic 
    let args = definitions::MainArguments::parse();

    let mut state = MainState::new();
    
    // This should panic if the configuration is wrong...
    let config_path = state.read_file(args.root_config.clone()).unwrap();

    // This should panic if the configuration is wrong...
    let config: definitions::MainConfig = serde_yaml::from_str(&config_path).unwrap();
    let backup_config = config.clone();

    // This should panic if the configuration is wrong...
    log4rs::init_file(config.log_config.clone(), Default::default()).unwrap();

    log::info!("Starting...");
    log::info!("Arguments: {:?}", args);
    log::debug!("Config: {:#?}", config.clone()); 


    let (storage_tx, storage_rx) = mpsc::channel::<storage::SqliteStorageAction>();
    let config_clone = config.clone();

    // TODO: Move this somewhere more appropriete
    let storage_handle = thread::spawn(move || {
        log::info!("Starting storage thread...");
        let config = config_clone;
        match config.storage {
            Storage::Sqlite { data_folder, size_management, backup_management } => {
                match  storage::SqliteStorage::new(data_folder, storage_rx) {
                    Ok(mut storage) => {

                        loop {
                            storage.process();
                        }
                    },
                    Err(e) => {
                        log::error!("Error creating sqlite storage process... {:?}", e);
                        panic!("Could not start storage process...");
                    }
                }

            }

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

    let mut channel_handles: Vec<JoinHandle<()>> = vec![];

    // Initialize found channel definitions
    for channel_definition in config.channels {

        // try to read file at specified location in definition
        let raw = match state.read_file(channel_definition.file.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not read channel config file for channel config: {:?} , error: {:?}", channel_definition, e);
                continue;
            }
        };

        match channel_definition._type {
            ChannelType::ModbusTcp => {
                let modbus_config = match ModbusClientTcpConfig::serialize(raw) {
                    Ok(conf) => conf,
                    Err(e) => {
                        log::error!("Error serializing ModbusClientTcpConfig: {:?}", e);
                        continue;
                    }
                };
                let mut register_maps: HashMap<ModbusSlave, ModbusRegisterMap> = HashMap::new();
                
                // We cannot have a channel that could have a 1 correct register for a device and 1 that is wrong
                // Either all the register maps are correct or none of them are.
                // By correct i mean a correct yaml format
                let mut skip_slave: bool = false;
                for slave in modbus_config.slaves.clone() {
                   match state.read_file(slave.register_map.clone()) {
                       Ok(register_map_raw) => {
            
                        let register_map = match serde_yaml::from_str::<ModbusRegisterMap>(&register_map_raw) {
                            Ok(rm) => rm,
                            Err(e) => {
                                log::error!("Error in register maps: {:?}", e);
                                skip_slave = true;
                                break;
                            }
                        };
                            register_maps.insert(slave, register_map);
            
                       },
                       Err(e) => {
                           log::error!("Error reading file path: {:?}, {:?}", slave.register_map, e );
                       }
            
                   }
                }
                if skip_slave { continue };
                let modbus_channel = ModbusTcpChannel::new(modbus_config, register_maps, aggregation_tx.clone());
                channel_handles.push(modbus_channel.run());


            },
            // TODO: Deduplicate code...
            ChannelType::ModbusRtu => {
                let modbus_config = match ModbusClientRtuConfig::serialize(raw) {
                    Ok(conf) => conf,
                    Err(e) => {
                        log::error!("Error serializing ModbusClientRtuConfig: {:?}", e);
                        continue;
                    }
                };
                let mut register_maps: HashMap<ModbusSlave, ModbusRegisterMap> = HashMap::new();
                
                // We cannot have a channel that could have a 1 correct register for a device and 1 that is wrong
                // Either all the register maps are correct or none of them are.
                // By correct i mean a correct yaml format
                let mut skip_slave: bool = false;
                for slave in modbus_config.slaves.clone() {

                   match state.read_file(slave.register_map.clone()) {
                       Ok(register_map_raw) => {
            
                        let register_map = match serde_yaml::from_str::<ModbusRegisterMap>(&register_map_raw) {
                            Ok(rm) => rm,
                            Err(e) => {
                                log::error!("Error in register maps: {:?}", e);
                                skip_slave = true;
                                break;
                            }
                        };
                            register_maps.insert(slave, register_map);
            
                       },
                       Err(e) => {
                           log::error!("Error reading file path: {:?}, {:?}", slave.register_map, e );
                       }
            
                   }
                }
                
                if skip_slave { continue };
                let modbus_channel = ModbusRtuChannel::new(modbus_config, register_maps, aggregation_tx.clone());
                channel_handles.push(modbus_channel.run());
            }
        }
    }

    log::debug!("Loaded Configs with their hashes: {:?}", state.get_configured_hashes());
    // For testing purposes...
    // let modbus_raw = state.read_file("./dist/modbus.yml".to_string()).unwrap();
    // let modbus_config = ModbusClientTcpConfig::serialize(modbus_raw).unwrap();

    // l

    // let mqtt_config = config.mqtt.clone();


    // Start MPSC channel that we can pass wto storage thread

    // Start new storage thread
    // This thread schould also schedule compactions
    // Compaction is a process where we delete old data stored in local database
    // it is essential to work properly. becouse a faulty compaction could couse that
    // the system drive where this is running to be full. 
    backup_db_scheduler(storage_tx.clone(), backup_config).join().unwrap();
    
    aggregator_handle.join().unwrap();
    transport_handle.join().unwrap();
    storage_handle.join().unwrap();
    // modbus_handle.join().unwrap();
}


// TODO: 
fn backup_db_scheduler(storage_tx: Sender<storage::SqliteStorageAction>, config: MainConfig) -> JoinHandle<()> {
    thread::spawn(move || {

        let mut scheduler = job_scheduler::JobScheduler::new();

        let job = job_scheduler::Job::new(
            job_scheduler::Schedule::from_str("1 1/10 * * * *").unwrap(), || {

            let datetime = Utc::now();
            let datetime = datetime.with_timezone(&Bratislava);

            let date_string = datetime.format("_%F_%X.db").to_string();
            let mut backup_name = config.name.replace(" ", "_").to_lowercase();
            backup_name.push_str(&date_string);
            log::trace!("Picked a backup name: {}", backup_name);

            match storage_tx.send(storage::SqliteStorageAction::BackupDB(backup_name)) {
                Ok(_) => log::trace!("Sent backup command to SqliteStorage"),
                Err(e) => log::error!("Could not send backup command to SqliteStorage, {:?}", e)
            };
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