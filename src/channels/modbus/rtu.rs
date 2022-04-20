use std::time::Duration;
use std::{thread::JoinHandle, collections::HashMap};
use std::thread;
use std::sync::{mpsc, Arc};

use crate::channels::DataPoint;
use crate::definitions::{TimeseriesMessage, AttributeMessage, OneTelemetry};
use crate::{channels::{Channel, ChannelStatus}, definitions::AggregatorAction};

use super::{ModbusClientRtuConfig, ModbusSlave, ModbusRegisterMap};

use tokio::sync::Mutex;
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, Parity, StopBits, SerialStream};
use tokio;
#[derive(Debug)]
pub struct ModbusRtuChannel {
    config: ModbusClientRtuConfig,
    status: ChannelStatus,
    register_maps: HashMap<ModbusSlave, ModbusRegisterMap>,
    aggregator_tx: mpsc::Sender<AggregatorAction>
}

impl ModbusRtuChannel {
    pub fn new(
        config: ModbusClientRtuConfig,
        register_maps: HashMap<ModbusSlave, ModbusRegisterMap>,
        aggregator_tx: mpsc::Sender<AggregatorAction>
    ) -> Self {
        Self {
            config,
            register_maps,
            aggregator_tx,
            status: ChannelStatus::Stopped
        }
    }
}


impl Channel for ModbusRtuChannel {
    fn run(mut self) -> JoinHandle<()>{
        log::warn!("RTU CONFIG: {:?}", self);

        let parity = match self.config.parity {
            'E' => {
                Parity::Even
            },
            'N' => {
                Parity::None
            },
            'O' => {
                Parity::Odd
            },
            _ => { 
                log::error!("Unexpected parity character... defaulting to parity N");
                Parity::None
            }
        };

        let stop_bits = match self.config.stop_bits {
            1 => StopBits::One,
            2 => StopBits::Two,
            _ => {
                log::error!("Unexpected number of stopbits... defaulting to Stopbit::One");
                StopBits::One
            }
        };
        let data_bits = match self.config.data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            8 => DataBits::Eight,
            _ => {
                log::error!("Unexpected number of databits... defaulting to 8 data bits");
                DataBits::Eight
            }
        };
        let handle = thread::spawn(move || {
            
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _guard = rt.enter();
            // let tokio_handle = rt.handle();

            let aggregator = self.aggregator_tx.clone();
            loop {
                    let aggregator = aggregator.clone();
                let reg_maps  = self.register_maps.clone();
                let serial_builder = tokio_serial::new(self.config.port.clone(), self.config.baudrate).clone();
                let serial_builder =  serial_builder.parity(parity.clone());
                let serial_builder = serial_builder.stop_bits(stop_bits.clone());
                let serial_builder = serial_builder.data_bits(data_bits);
                let serial_stream = SerialStream::open(&serial_builder).unwrap();
                
                rt.block_on(async move {


                    let aggregator = aggregator.clone();
                    match tokio_modbus::client::rtu::connect(serial_stream).await {
                        Err(e) => {
                            log::error!("Could not get modbus context: {:?}", e);
                        },
                        Ok(mut ctx) => {
                            for (slave, reg_map) in reg_maps {
                                // Set correct ModbusID to call on
                                ctx.set_slave(Slave(slave.modbus_id));

                                let mut attributes_message: AttributeMessage = (slave.device_name.clone(), HashMap::new());
                                let mut timeseries_message: TimeseriesMessage = (slave.device_name.clone(), vec![]);
                                // Read Attributes 
                                for reg_group in &reg_map.attributes {
                                    
                                    let mut data_point_vec: Vec<DataPoint> = vec![];

                                    // Call group 
                                    log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                                    let reg_response = ctx.read_holding_registers(
                                            reg_group.starting_address,
                                            reg_group.elements_count).await;

                                    let reg_response = match reg_response {
                                        Ok(res) => res,
                                        Err(e) => {
                                            log::error!("Error reading holding registers: {:?}", e);
                                            continue
                                        }
                                    };

                                    for data_point in &reg_group.data_points {
                                        let point = data_point.parse(reg_response.clone());
                                        data_point_vec.push(point.clone());
                                        // parse into message
                                        attributes_message.1.insert(point.key, point.value);
                                    }
                                    log::info!("Read and parsed register group with data {} points", data_point_vec.len());
                                    log::debug!("Datapoints in reg group: {:?}", data_point_vec);
                                    log::debug!("Attribute message: {:?}", attributes_message)

                                }
                                // Read Timeseries
                                for reg_group in &reg_map.timeseries {
                                    
                                    let mut data_point_vec: Vec<DataPoint> = vec![];

                                    // Call group 
                                    log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                                    let reg_response = ctx.read_holding_registers(
                                            reg_group.starting_address,
                                            reg_group.elements_count).await;
                                    
                                    let reg_response = match reg_response {
                                        Ok(res) => res,
                                        Err(e) => {
                                            log::error!("Error reading holding registers: {:?}", e);
                                            continue
                                        }
                                    };

                                    for data_point in &reg_group.data_points {
                                        let point = data_point.parse(reg_response.clone());
                                        data_point_vec.push(point);
                                    }
                                    log::info!("Read and parsed register group with data {} points", data_point_vec.len());
                                    log::debug!("Datapoints in reg group: {:?}", data_point_vec);
                                    timeseries_message.1.push(OneTelemetry::from(data_point_vec))
                                }
                                match aggregator.send(AggregatorAction::SendBoth(attributes_message, timeseries_message)) {
                                    Err(e) => log::error!("Error sending data to aggregation thread! Did it panic? : {:#?}", e),
                                    _ => {}
                                }
                            }
                            
                            // Disconnect
                            match ctx.call(Request::Disconnect).await {
                                Ok(r) => log::info!("Disconnect response: {:?}", r),
                                Err(e) => log::error!("Error disconnecting: {:?}", e)
                            }
                            // thread::sleep(Duration::from_millis(10000))
                        }
                    };

                });
                // rt.(task).unwrap();
            }
        });
        handle

    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }
}