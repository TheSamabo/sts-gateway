

use chrono_tz::Europe::Bratislava;
use clap::Parser;
use definitions::{MainConfig, StorageBackupManagement};
use job_scheduler::JobScheduler;
use serde_yaml;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use std::fs;
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
use crate::definitions::{TransportAction, AggregatorAction, ChannelType, Storage, StorageSizeManagement};
use crate::storage::SqliteStorageTruncate;
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
    let storage_tx_clone = storage_tx.clone();

    // TODO: Move this somewhere more appropriete
    let storage_handle = thread::spawn(move || {
        log::info!("Starting storage thread...");
        let config = config_clone.clone();
        match config.storage {
            Storage::Sqlite { data_folder, size_management, backup_management } => {
                

                let _backup_join = match backup_management {
                    StorageBackupManagement::Local { backup_folder, backup_interval, backup_ttl } => {
                        match size_management {
                            StorageSizeManagement::FixedWindow { messages_ttl_check, messages_ttl } => {
                                backup_local_fixed_window_scheduler(
                                    storage_tx_clone.clone(), config_clone.clone(), backup_interval, backup_ttl, backup_folder, messages_ttl_check, messages_ttl)
                            }
                        }
                    }
                };


                match  storage::SqliteStorage::new(data_folder, storage_rx) {
                    Ok(mut storage) => {

                        loop {
                            storage.process();
                        }
                        _backup_join.join().unwrap();

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
    
    storage_handle.join().unwrap();
    aggregator_handle.join().unwrap();
    transport_handle.join().unwrap();
    // modbus_handle.join().unwrap();
}
fn truncate_fixed_window(storage_tx: Sender<storage::SqliteStorageAction>, config: MainConfig, messages_ttl_check: String, messeges_ttl: i32) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut scheduler = job_scheduler::JobScheduler::new();
        log::debug!("Registering truncate_fixed_window job on interval: {}", messages_ttl_check);
    })
}

// TODO: 
fn backup_local_fixed_window_scheduler(storage_tx: Sender<storage::SqliteStorageAction>,
        config: MainConfig,
        backup_interval: String,
        backup_ttl: i32,
        backup_folder: String,
        messages_ttl_check: String,
        messages_ttl: i32) -> JoinHandle<()> {

    let backup_storage_tx = storage_tx.clone();
    let backup_messages_ttl_clone = messages_ttl.clone();

    thread::spawn(move || {
        let mut scheduler = job_scheduler::JobScheduler::new();
        log::debug!("Registering backup_local_scheduler on interval: {}", backup_interval);
        let backup_job = job_scheduler::Job::new(
            job_scheduler::Schedule::from_str(&backup_interval).unwrap(), move || {

            let datetime = Utc::now();
            let datetime = datetime.with_timezone(&Bratislava);
            let backup_path = Path::new(&backup_folder);

            if !backup_path.exists() {
                match fs::create_dir_all(backup_path) {
                    Ok(_) => log::info!("Created backup directories..."),
                    Err(e) => log::error!("Failed to create backup directories: {:?}", e)
                }
            }

            let date_string = datetime.format(":%+.db").to_string();
            let mut backup_name = config.name.replace(" ", "_").to_lowercase();
            backup_name.push_str(&date_string);
            log::trace!("Picked a backup name: {}", backup_name);

            let backup_path = backup_path.join(backup_name);
            log::trace!("Full backup path: {:?}", backup_path);
            log::info!("Sending truncation command");
            match backup_storage_tx.send(storage::SqliteStorageAction::Truncate(
                storage::SqliteStorageTruncate::FixedWindow(chrono::Duration::hours(backup_messages_ttl_clone.into()))))  {
                Ok(_) => log::trace!("Sent truncation command successfuly"),
                Err(e) => log::error!("Could not send truncation command error: {:?}", e)
            };

            match backup_storage_tx.send(storage::SqliteStorageAction::BackupDB(backup_path.display().to_string())) {
                Ok(_) => log::trace!("Sent backup command to SqliteStorage"),
                Err(e) => log::error!("Could not send backup command to SqliteStorage, {:?}", e)
            };

            // Delete old backups
            match fs::read_dir(backup_folder.clone()) {
                Ok(dir) => {
                    let v = 3600u64 * backup_ttl as u64;
                    let delta = Duration::from_secs(v);
                    let now = SystemTime::now();
                    let older_than = now - delta;
                    
                    for entry_result in dir {

                        if let Ok(entry) = entry_result {
                            let entry_metadata = entry.metadata().unwrap();
                            if entry_metadata.is_file() {
                                if let Ok(created) = entry_metadata.created() {
                                    if created  < older_than {
                                        match fs::remove_file(entry.path()) {
                                            Ok(_) => log::info!("Deleted old backup: {}", entry.path().display()),
                                            Err(e) => log::error!("Error deleting expired backups: {:?}", e)
                                        }
                                    }
                                }
                            }
                        }
                        
                    }
                },
                Err(e) => log::error!("Could not read backup directory {} : {:?}", backup_folder, e)
            }
        });
        let truncate_job = job_scheduler::Job::new(
            job_scheduler::Schedule::from_str(&messages_ttl_check).unwrap(), move || {
                match storage_tx.send(storage::SqliteStorageAction::Truncate(SqliteStorageTruncate::FixedWindow(
                    chrono::Duration::hours(messages_ttl.into())))) {
                        Ok(_) => log::debug!("Truncate JOB: Sent truncate command"),
                        Err(e) => log::error!("Error sending truncate command")
                    }
            }
        );

        scheduler.add(backup_job);
        scheduler.add(truncate_job);

        loop {
            scheduler.tick();
            thread::sleep(Duration::from_millis(500));
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