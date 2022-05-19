use std::io::{ErrorKind, Write, Read};
use std::time::Duration;
use std::{thread::JoinHandle, collections::HashMap};
use std::thread;
use std::sync::mpsc;

use crate::channels::DataPoint;
use crate::definitions::{TimeseriesMessage, AttributeMessage, OneTelemetry};
use crate::{channels::{Channel, ChannelStatus}, definitions::AggregatorAction};

use super::{ModbusClientRtuConfig, ModbusSlave, ModbusRegisterMap};
use rmodbus::{client::ModbusRequest, guess_response_frame_len, ModbusProto};
use serialport::{self, SerialPort};
// use libmodbus::{ModbusClient, Modbus,  ModbusRTU, ErrorRecoveryMode, Timeout};
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
    fn run(self) -> JoinHandle<()>{
        log::trace!("RTU CONFIG: {:?}", self);

        let builder = thread::Builder::new()
            .name(self.config.name)
            .spawn(move || {

            let aggregator = self.aggregator_tx.clone();

                let parity = match self.config.parity {
                    'E' => serialport::Parity::Even,
                    'N' => serialport::Parity::None,
                    'O' => serialport::Parity::Odd,
                    _ => {
                        log::warn!("Could not set parity: {:?}, defaulting to Parity::None", self.config.parity);
                        serialport::Parity::None
                    }
                };
                let databits = match self.config.data_bits {
                    8 => serialport::DataBits::Eight,
                    7 => serialport::DataBits::Seven,
                    6 => serialport::DataBits::Six,
                    5 => serialport::DataBits::Five,
                    _ => {
                        log::warn!("Could not set data_bits: {:?}, defaulting to DataBits::Eight", self.config.data_bits);
                        serialport::DataBits::Eight
                    }
                };
            // modbus.set_debug(true);
            // modbus.rtu_set_serial_mode(SerialMode::RtuRS485).unwrap();
            // modbus.rtu_set_rts(RequestToSendMode::RtuRtsUp).unwrap();
                let mut reg_maps  = self.register_maps.clone();
            
                log::debug!("Opening serial port: {:?}", self.config.port);
                let builder = serialport::new(self.config.port.clone(), self.config.baudrate);
                let builder = builder.timeout(Duration::from_secs(1));
                let builder = builder.data_bits(databits);
                let builder = builder.parity(parity);
                let mut port = builder.open_native().expect(&format!("Could not open serial port: {:?}", self.config.port.clone()));
                // log::debug!("Creating RTU Context");
                
                // let mut modbus = Modbus::new_rtu(
                //         &self.config.port,
                //         self.config.baudrate.try_into().unwrap(),
                //         self.config.parity,
                //         self.config.data_bits.into(),
                //         self.config.stop_bits.into()).unwrap();
                // modbus.set_byte_timeout(Timeout::new(0,50000)).unwrap();
                // modbus.set_response_timeout(Timeout::new(1,50000)).unwrap();
                // modbus.set_error_recovery(Some(&[ErrorRecoveryMode::Protocol, ErrorRecoveryMode::Link])).unwrap();


                
                // File descriptor slave
                // key: modbus_id
                // value: descriptor id
                let mut fds: HashMap<u8, i32> = HashMap::new();
                // if reg_maps.len() < 1000 {
                //     let mut i = 1;
                //     for (slave,reg_map) in &reg_maps {
                //         fds.insert(slave.modbus_id.clone(), i  );
                //         i += 1;
                //     };
                // }
                log::debug!("FDS: {:?}", fds);
                // while let Err(e) = modbus.connect() {
                //     log::error!("Error connecting to serial line: {:?}", self.config.port);
                //     thread::sleep(Duration::from_millis(1000));
                // };
                loop {

                    for (slave, reg_map) in &reg_maps {



                        // Set correct ModbusID to call on
                        // ctx.set_slave(Slave(slave.modbus_id));
                        // log::info!("Connecting to slave with id: {}",slave.modbus_id );
                        // log::debug!("Filedescriptor Slaves Hashmap: {:?}", fds);
                        // match fds.get(&slave.modbus_id) {
                            // Some(f) => {
                                // log::debug!("Found existing file descriptor for slave {} ", slave.modbus_id);
                        // modbus.set_socket(slave.modbus_id.into()).unwrap();
                        // modbus.set_slave(slave.modbus_id).unwrap();
                        // match  modbus.get_socket() {
                        //     Ok(fd) => {
                        //         log::info!("Connected  to slave with id : {:?}, on file descriptor: {:?}",
                        //         slave.modbus_id, fd);
                        //     },
                        //     Err(e) => log::error!("Error getting file descriptor for slave with id: {:?}", slave.modbus_id)
                        // }

                        let mut attributes_message: AttributeMessage = (slave.device_name.clone(), HashMap::new());
                        let mut timeseries_message: TimeseriesMessage = (slave.device_name.clone(), vec![]);
                        // Read Attributes 
                        for reg_group in &reg_map.attributes {
                            
                            let mut mreq = ModbusRequest::new(slave.modbus_id, ModbusProto::Rtu);
                            let mut read_buffer =  Vec::new();
                            let mut request = Vec::new();

                            let mut data_point_vec: Vec<DataPoint> = vec![];
                            match port.clear(serialport::ClearBuffer::All) {
                                Err(e) => log::error!("Error clearing buffers! {:?}", e),
                                _ => ()
                            }

                            // Call group 
                            log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                            
                            match mreq.generate_get_holdings(
                                reg_group.starting_address, reg_group.elements_count, &mut request) {

                                Err(e) => {
                                    log::error!("Error creating read holding request: {:?}", e);
                                    continue;
                                },
                                Ok(_) => log::debug!("Created read holding registers request...")
                            }
                            match port.write(&request) {
                                Ok(_) => {
                                    log::debug!("Send request to read holding registers...");
                                    if port.flush().is_err() {log::warn!("Error flush write buffer...")};
                                },
                                Err(e) => {
                                    log::error!("Could not send request for holding registers: {:?}", e);
                                    continue;
                                }
                            }
                            // Possible timeout window
                            let mut buf = [0,0,0,0,0,0,0,0,0,0];
                            match port.read_exact(&mut buf) {
                                Ok(_) => log::debug!("read first response correctly"),
                                Err(e) => {
                                    match port.clear(serialport::ClearBuffer::Input) {
                                        Err(e) => log::error!("Error clearing input buffer! {:?}", e),
                                        _ => ()
                                    }
                                    match e.kind() {
                                        ErrorKind::TimedOut => {
                                            log::warn!("Operation to read first response timedout");
                                            continue;
                                        },
                                        _ => {
                                            log::error!("Error reading first response: {:?}",e );
                                            continue;
                                        }
                                    }
                                }
                            }
                            match port.clear(serialport::ClearBuffer::Input) {
                                Err(e) => log::error!("Error clearing input buffer! {:?}", e),
                                _ => ()
                            }
                            log::trace!("From serial read this buffer: {:?}", buf);
                            let len = guess_response_frame_len(&buf, ModbusProto::Rtu);
                            let mut response = Vec::new();
                            response.extend_from_slice(&buf);
                            match len {
                                Err(e) => {
                                    log::error!("Error guessing response frame len: {:?}", e);
                                    match port.clear(serialport::ClearBuffer::Input) {
                                        Err(e) => log::error!("Error clearing input buffer! {:?}", e),
                                        _ => ()
                                    }
                                    continue;
                                },
                                Ok(i) => {
                                    if i > 10{
                                        let mut rest = vec![0u8; (i - 10) as usize];
                                        match port.read_exact(&mut rest) {
                                            Ok(_) => {
                                                log::debug!("read first response correctly");
                                                match port.clear(serialport::ClearBuffer::Input) {
                                                    Err(e) => log::error!("Error clearing input buffer! {:?}", e),
                                                    _ => ()
                                                }

                                            },
                                            Err(e) => 
                                            {
                                                match port.clear(serialport::ClearBuffer::Input) {
                                                    Err(e) => log::error!("Error clearing input buffer! {:?}", e),
                                                    _ => ()
                                                }
                                                match e.kind() {
                                                    ErrorKind::TimedOut => {
                                                        log::warn!("Operation to read first response timedout");
                                                        continue;
                                                    }
                                                    _ => {
                                                        log::error!("Error reading first response: {:?}",e );
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                        response.extend(rest);
                                    }
                                }
                            }
                            log::trace!("Checking response: {:?}", response);
                            // let mut read_buffer = Vec::new();
                            match mreq.parse_ok(&response) {
                                Ok(_) => {
                                    log::debug!("Modbus response is OK");
                                    match mreq.parse_u16(&response, &mut read_buffer) {
                                        Ok(_) => log::info!("successfully extracted data from response!"),
                                        Err(e) => {
                                            log::error!("Error extracting data from response: {:?}", e);
                                            continue;
                                        }
                                    };
                                },
                                Err(e) => {
                                    log::error!("Error in modbus response: {:?}", e);
                                    continue;
                                }
                            };
                            log::trace!("Read Raw data: {:?}", read_buffer);



                            // let reg_response = modbus.read_registers(
                            //         reg_group.starting_address,
                            //         reg_group.elements_count,
                            //         &mut read_buffer);

                            // match modbus.flush() {
                            //     Ok(_) => log::debug!("Flushed untransmited data..."),
                            //     Err(e) => log::error!("Error flushing untransmited data: {:?}", e)
                            // }

                            // match reg_response {
                            //     Ok(res) => log::debug!("Read {:?} holding registers...", res ),
                            //     Err(e) => {
                            //         log::error!("Error reading holding registers: {:?}", e);
                            //         continue
                            //     }
                            // };

                            for data_point in &reg_group.data_points {
                                let point = data_point.parse(read_buffer.clone());
                                data_point_vec.push(point.clone());
                                // parse into message
                                attributes_message.1.insert(point.key, point.value);
                            }

                            log::info!("Read and parsed register group with data {} points", data_point_vec.len());
                            log::trace!("Datapoints in reg group: {:?}", data_point_vec);
                            log::trace!("Attribute message: {:?}", attributes_message)

                        }
                        // Read Timeseries
                        // thread::sleep(Duration::from_millis(100));
                        for reg_group in &reg_map.timeseries {

                            let mut mreq = ModbusRequest::new(slave.modbus_id, ModbusProto::Rtu);
                            let mut read_buffer =  Vec::new();
                            let mut request = Vec::new();

                            let mut data_point_vec: Vec<DataPoint> = vec![];

                            // Call group 
                            log::info!("Reading starting address: {:}, and register count: {:} of slave with id: {:?}", reg_group.starting_address, reg_group.elements_count, slave.modbus_id);
                            match port.clear(serialport::ClearBuffer::All) {
                                Err(e) => log::error!("Error clearing output buffer! {:?}", e),
                                _ => ()
                            }
                            
                            match mreq.generate_get_holdings(
                                reg_group.starting_address, reg_group.elements_count, &mut request) {

                                Err(e) => log::error!("Error creating read holding request: {:?}", e),
                                Ok(_) => log::debug!("Created read holding registers request...")
                            }
                            match port.write(&request) {
                                Ok(_) => {
                                    log::debug!("Send request to read holding registers...");
                                    if port.flush().is_err() {log::warn!("Error flush write buffer...")};
                                },
                                Err(e) => {
                                    log::error!("Could not send request for holding registers: {:?}", e);
                                    continue;
                                }
                            }
                            // Possible timeout window
                            let mut buf = [0,0,0,0,0,0,0,0,0,0];
                            match port.read_exact(&mut buf) {
                                Ok(_) => log::debug!("read first response correctly"),
                                Err(e) => match e.kind() {
                                    ErrorKind::TimedOut => {
                                        log::warn!("Operation to read first response timedout");
                                        continue;
                                    }
                                    _ => {
                                        log::error!("Error reading first response: {:?}",e );
                                        continue;
                                    }
                                }
                            }
                            log::trace!("From serial read this buffer: {:?}", buf);
                            let len = guess_response_frame_len(&buf, ModbusProto::Rtu);
                            let mut response = Vec::new();
                            response.extend_from_slice(&buf);
                            match len {
                                Err(e) => {
                                    log::error!("Error guessing response frame len: {:?}", e);
                                    continue;
                                },
                                Ok(i) => {
                                    if i > 10{
                                        let mut rest = vec![0u8; (i - 10) as usize];
                                        match port.read_exact(&mut rest) {
                                            Ok(_) => log::debug!("read first response correctly"),
                                            Err(e) => match e.kind() {
                                                ErrorKind::TimedOut => {
                                                    log::warn!("Operation to read first response timedout");
                                                    continue;
                                                }
                                                _ => {
                                                    log::error!("Error reading first response: {:?}",e );
                                                    continue;
                                                }
                                            }
                                        }
                                        response.extend(rest);
                                    }
                                }
                            }
                            log::trace!("Checking response: {:?}", response);
                            // let mut read_buffer = Vec::new();
                            match mreq.parse_ok(&response) {
                                Ok(_) => {
                                    log::debug!("Modbus response is OK");
                                    match mreq.parse_u16(&response, &mut read_buffer) {
                                        Ok(_) => log::info!("successfully extracted data from response!"),
                                        Err(e) => {
                                            log::error!("Error extracting data from response: {:?}", e);
                                            continue;
                                        }
                                    };
                                },
                                Err(e) => {
                                    log::error!("Error in modbus response: {:?}", e);
                                    continue;
                                }
                          
                            };
                            log::trace!("Read Raw data: {:?}", read_buffer);



                            // OLD
                            
                            // let mut mreq = ModbusRequest::new(slave.modbus_id, ModbusProto::Rtu);
                            // let mut read_buffer = Vec::new();
                            // let mut request = Vec::new();

                            // let mut data_point_vec: Vec<DataPoint> = vec![];

                            // // Call group 
                            
                            // match mreq.generate_get_holdings(
                            //     reg_group.starting_address, reg_group.elements_count, &mut request) {

                            //     Err(e) => log::error!("Error creating read holding request: {:?}", e),
                            //     Ok(_) => log::debug!("Created read holding registers request...")
                            // }
                            // port.clear(serialport::ClearBuffer::All).unwrap();
                            // port.write(&request).unwrap();
                            // // Possible timeout window
                            // let mut buf = [0,0,0,0,0,0,0,0,0,0];
                            // port.read(&mut buf).unwrap();
                            // log::trace!("From serial read this buffer: {:?}", buf);
                            

                            // let len = guess_response_frame_len(&buf, ModbusProto::Rtu);
                            // let mut response = Vec::new();
                            // response.extend_from_slice(&buf);
                            // match len {
                            //     Err(e) => log::error!("Error guessing response frame len: {:?}", e),
                            //     Ok(i) => {
                            //         if i > 10{
                            //             let mut rest = vec![0u8; (i - 10) as usize];
                            //             port.read_exact(&mut rest).unwrap();
                            //             response.extend(rest);
                            //         }
                            //     }
                            // }
                            // // let mut read_buffer = Vec::new();
                            // match mreq.parse_ok(&response) {
                            //     Ok(_) => {
                            //         log::debug!("Modbus response is OK");
                            //         match mreq.parse_u16(&response, &mut read_buffer) {
                            //             Ok(_) => log::info!("successfully extracted data from response!"),
                            //             Err(e) => log::error!("Error extracting data from response: {:?}", e)
                            //         };
                            //     },
                            //     Err(e) => log::error!("Error in modbus response: {:?}", e)
                            // };
                            // log::trace!("Read Raw data: {:?}", read_buffer);

                            // log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                            // let reg_response = modbus.read_registers(
                            //         reg_group.starting_address,
                            //         reg_group.elements_count,
                            //         &mut read_buffer);
                            
                            // match modbus.flush() {
                            //     Ok(_) => log::debug!("Flushed untransmited data..."),
                            //     Err(e) => log::error!("Error flushing untransmited data: {:?}", e)
                            // }
                            // match reg_response {
                            //     Ok(res) => log::debug!("Read {:?} holding registers...", res ),
                            //     Err(e) => {
                            //         log::error!("Error reading holding registers: {:?}", e);
                            //         continue
                            //     }
                            // };

                            for data_point in &reg_group.data_points {
                                let point = data_point.parse(read_buffer.clone());
                                data_point_vec.push(point);
                            }
                            

                            log::info!("Read and parsed register group with data {} points", data_point_vec.len());
                            log::trace!("Datapoints in reg group: {:?}", data_point_vec);
                            timeseries_message.1.push(OneTelemetry::from(data_point_vec));
                        }

                        match aggregator.send(AggregatorAction::SendBoth(attributes_message, timeseries_message)) {
                            Err(e) => log::error!("Error sending data to aggregation thread! Did it panic? : {:#?}", e),
                            _ => {}
                        }
                    }

                    
                    // modbus.close();
                    thread::sleep(Duration::from_millis(10000));
                }
                
        }).unwrap();
        builder

    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }
}