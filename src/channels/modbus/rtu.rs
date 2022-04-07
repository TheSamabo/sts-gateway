use std::thread::JoinHandle;

use crate::channels::Channel;

use super::ModbusClientRtuConfig;

pub struct ModbusRtuChannel {
    config: ModbusClientRtuConfig
}

impl Channel for ModbusRtuChannel {
    fn run(self) -> JoinHandle<()>{
        todo!()
    }

    fn status(&self) ->  crate::channels::ChannelStatus {
        todo!()
    }
}