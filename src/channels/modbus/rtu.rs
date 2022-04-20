use std::{thread::JoinHandle, collections::HashMap};
use std::sync::mpsc;

use crate::{channels::{Channel, ChannelStatus}, definitions::AggregatorAction};

use super::{ModbusClientRtuConfig, ModbusSlave, ModbusRegisterMap};

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
        todo!()
    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }
}