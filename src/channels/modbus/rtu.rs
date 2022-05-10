use std::time::Duration;
use std::{thread::JoinHandle, collections::HashMap};
use std::thread;
use std::sync::mpsc;

use crate::channels::DataPoint;
use crate::definitions::{TimeseriesMessage, AttributeMessage, OneTelemetry};
use crate::{channels::{Channel, ChannelStatus}, definitions::AggregatorAction};

use super::{ModbusClientRtuConfig, ModbusSlave, ModbusRegisterMap};

use libmodbus_rs::{ModbusClient, Modbus,  ModbusRTU, ErrorRecoveryMode, Timeout};
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

        // let parity = match self.config.parity {
        //     'E' => {
        //         Parity::Even
        //     },
        //     'N' => {
        //         Parity::None
        //     },
        //     'O' => {
        //         Parity::Odd
        //     },
        //     _ => { 
        //         log::error!("Unexpected parity character... defaulting to parity N");
        //         Parity::None
        //     }
        // };

        // let stop_bits = match self.config.stop_bits {
        //     1 => StopBits::One,
        //     2 => StopBits::Two,
        //     _ => {
        //         log::error!("Unexpected number of stopbits... defaulting to Stopbit::One");
        //         StopBits::One
        //     }
        // };
        // let data_bits = match self.config.data_bits {
        //     5 => DataBits::Five,
        //     6 => DataBits::Six,
        //     7 => DataBits::Seven,
        //     8 => DataBits::Eight,
        //     _ => {
        //         log::error!("Unexpected number of databits... defaulting to 8 data bits");
        //         DataBits::Eight
        //     }
        // };
        let builder = thread::Builder::new()
            .name(self.config.name)
            .spawn(move || {
            // let serial_stream = SerialStream::open(&serial_builder).unwrap();
            // let rt = tokio::runtime::Runtime::new().unwrap();
            // let tokio_handle = rt.handle();
            // let _guard = tokio_handle.enter();
            
            let aggregator = self.aggregator_tx.clone();
            let reg_maps  = self.register_maps.clone();
            // let serial_builder = tokio_serial::new(self.config.port.clone(), self.config.baudrate).clone();
            // let serial_builder =  serial_builder.parity(parity.clone());
            // let serial_builder = serial_builder.stop_bits(stop_bits.clone());
            // let serial_builder = serial_builder.data_bits(data_bits);
            // let serial_builder = serial_builder.timeout(Duration::from_secs(10)); 

            let mut modbus = Modbus::new_rtu(
                    &self.config.port,
                    self.config.baudrate.try_into().unwrap(),
                    self.config.parity,
                    self.config.data_bits.into(),
                    self.config.stop_bits.into()).unwrap();

            // modbus.set_debug(true);
            // modbus.rtu_set_serial_mode(SerialMode::RtuRS485).unwrap();
            // modbus.rtu_set_rts(RequestToSendMode::RtuRtsUp).unwrap();
            modbus.set_byte_timeout(Timeout::new(0,500000)).unwrap();
            modbus.set_response_timeout(Timeout::new(1,500000)).unwrap();
            modbus.set_error_recovery(Some(&[ErrorRecoveryMode::Protocol, ErrorRecoveryMode::Link])).unwrap();

            loop {
                for (slave, reg_map) in &reg_maps {
                    // Set correct ModbusID to call on
                    // ctx.set_slave(Slave(slave.modbus_id));
                    log::info!("Connecting to slave with id: {}",slave.modbus_id );
                    modbus.set_slave(slave.modbus_id).unwrap();
                    modbus.connect().unwrap();

                    let mut attributes_message: AttributeMessage = (slave.device_name.clone(), HashMap::new());
                    let mut timeseries_message: TimeseriesMessage = (slave.device_name.clone(), vec![]);
                    // Read Attributes 
                    for reg_group in &reg_map.attributes {
                        
                        let mut read_buffer =  vec![0u16; 100];

                        let mut data_point_vec: Vec<DataPoint> = vec![];

                        // Call group 
                        log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                        let reg_response = modbus.read_registers(
                                reg_group.starting_address,
                                reg_group.elements_count,
                                &mut read_buffer);

                        match modbus.flush() {
                            Ok(_) => log::debug!("Flushed untransmited data..."),
                            Err(e) => log::error!("Error flushing untransmited data: {:?}", e)
                        }

                        match reg_response {
                            Ok(res) => log::debug!("Read {:?} holding registers...", res ),
                            Err(e) => {
                                log::error!("Error reading holding registers: {:?}", e);
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
                        
                        let mut read_buffer = vec![0u16; 100];

                        let mut data_point_vec: Vec<DataPoint> = vec![];

                        // Call group 
                        log::trace!("Reading starting address: {:}, and register count: {:}", reg_group.starting_address, reg_group.elements_count);
                        let reg_response = modbus.read_registers(
                                reg_group.starting_address,
                                reg_group.elements_count,
                                &mut read_buffer);
                        
                        match modbus.flush() {
                            Ok(_) => log::debug!("Flushed untransmited data..."),
                            Err(e) => log::error!("Error flushing untransmited data: {:?}", e)
                        }
                        match reg_response {
                            Ok(res) => log::debug!("Read {:?} holding registers...", res ),
                            Err(e) => {
                                log::error!("Error reading holding registers: {:?}", e);
                                continue
                            }
                        };

                        for data_point in &reg_group.data_points {
                            let point = data_point.parse(read_buffer.clone());
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
                thread::sleep(Duration::from_millis(30000))
            }
                
        }).unwrap();
        builder

    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }
}