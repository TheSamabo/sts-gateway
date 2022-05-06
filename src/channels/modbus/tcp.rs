use crate::channels::{Channel, ChannelStatus, DataPoint};
use crate::definitions::{AggregatorAction, OneTelemetry};

use super::{ModbusClientTcpConfig, ModbusRegisterMap, ModbusSlave};
use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr};
use std::{sync::mpsc, thread::JoinHandle};
use std::thread;
use std::time::Duration;
// use tokio_modbus::prelude::*;
use libmodbus_rs::{Modbus, ModbusTCPPI, ModbusClient};
// use tokio;

use crate::definitions::{AttributeMessage, TimeseriesMessage};

pub struct ModbusTcpChannel {
    config: ModbusClientTcpConfig,
    status: ChannelStatus,
    register_maps: HashMap<ModbusSlave, ModbusRegisterMap>,
    aggregator_tx: mpsc::Sender<AggregatorAction>
}

impl ModbusTcpChannel {
    pub fn new(
            config: ModbusClientTcpConfig,
            register_maps: HashMap<ModbusSlave, ModbusRegisterMap>,
            aggregator_tx: mpsc::Sender<AggregatorAction>
        ) -> Self {

        Self {
            config,
            status: ChannelStatus::Stopped,
            register_maps,
            aggregator_tx
        }
    }
}

impl Channel for ModbusTcpChannel {
    fn run(mut self) -> JoinHandle<()> {
        self.status = ChannelStatus::Running;
        let handle = thread::spawn(move || {

            // Make connection to ModbusTCP server
            // let socket_full_string = format!("{}:{}", self.config.host ,self.config.port);
            // let socket_addr = socket_full_string.parse::<SocketAddr>().unwrap();

            let addr: IpAddr = self.config.host.parse().unwrap();
            let socket = SocketAddr::from((addr, self.config.port));
            let mut modbus = Modbus::new_tcp_pi(&socket.ip().to_string(), &socket.port().to_string());


            // let mut socket_addr: SocketAddr = SocketAddr::new(Ipv4Addr::from_str("127.0.0.1").unwrap().into(), 502);
            // if self.config.host.contains(":") {
            //     // let new_ip = std::net::Ipv6Addr::from_str(&self.config.host);
            //     // match new_ip {
            //     //     Ok(ip) => ip,
            //     //     Err(e) => log::error!("Error parsing IPV6 host and port: {:?}",e)
            //     // }
            //     socket_addr.set_ip(std::net::Ipv6Addr::from_str(&self.config.host).unwrap().into());
            // }
            // else {
            //     socket_addr.set_ip(std::net::Ipv4Addr::from_str(&self.config.host).unwrap().into());
            // }
            // socket_addr.set_port(self.config.port);
            // let socket_addr = socket_addr.into();
            loop { 
                // log::info!("Sleeping for 20s");
                thread::sleep(Duration::from_millis(30000));
                
                // Error Handle
                match modbus {
                    Ok(ref mut ctx) => {
                        for (slave, reg_map) in &mut self.register_maps {
                            // Set correct ModbusID to call on
                            match ctx.set_slave(slave.modbus_id) {
                                Ok(_) => log::trace!("Switched to slave with id: {}", slave.modbus_id),
                                Err(e) => log::error!("Error switching to modbus slave id: {} error: {:?}", slave.modbus_id, e)
                            };

                            let mut attributes_message: AttributeMessage = (slave.device_name.clone(), HashMap::new());
                            let mut timeseries_message: TimeseriesMessage = (slave.device_name.clone(), vec![]);
                            // Read Attributes 
                            for reg_group in &reg_map.attributes {
                                
                                let mut data_point_vec: Vec<DataPoint> = vec![];
                                let mut read_buffer: Vec<u16> = vec![];

                                // Call group 
                                log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                                match ctx.read_registers(reg_group.starting_address,reg_group.elements_count,&mut read_buffer) {
                                    Ok(c) => log::debug!("Success reading register group: {:?} return code: {}",reg_group, c),
                                    Err(e) => {
                                        log::error!("Error reading register group: {:?} return code: {}", reg_group, e);
                                        continue
                                    }
                                };

                                for data_point in &reg_group.data_points {
                                    let point = data_point.parse(read_buffer.clone());
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
                                let mut read_buffer: Vec<u16> = vec![];

                                // Call group 
                                match ctx.read_registers(reg_group.starting_address,reg_group.elements_count,&mut read_buffer) {
                                    Ok(c) => log::debug!("Success reading register group: {:?} return code: {}",reg_group, c),
                                    Err(e) => {
                                        log::error!("Error reading register group: {:?} return code: {}", reg_group, e);
                                        continue
                                    }
                                };

                                for data_point in &reg_group.data_points {
                                    let point = data_point.parse(read_buffer.clone());
                                    data_point_vec.push(point.clone());
                                }

                                log::info!("Read and parsed register group with data {} points", data_point_vec.len());
                                log::debug!("Datapoints in reg group: {:?}", data_point_vec);
                                timeseries_message.1.push(OneTelemetry::from(data_point_vec))
                            }
                            match self.aggregator_tx.send(AggregatorAction::SendBoth(attributes_message, timeseries_message)) {
                                Err(e) => log::error!("Error sending data to aggregation thread! Did it panic? : {:#?}", e),
                                _ => {}
                            }
                        }
                        
                        // Disconnect
                        ctx.close();

                    },


                    Err(ref e)  => {
                        log::error!("Error Connecting to ModbusTCP server: {:?}", e);
                    }

                }
                    
            }
        });
    

        handle
    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }

}