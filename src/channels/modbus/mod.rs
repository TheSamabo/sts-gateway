use std::cmp::min;
use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;
// use tokio_serial::{DataBits, Parity, StopBits, FlowControl};
// use tokio_modbus::slave::Slave;

use serde_yaml::Error;
use serde_yaml::from_str;


// use safe_transmute;
use bytemuck;
use bytebuffer::ByteBuffer;

pub mod tcp;
pub mod rtu;
// pub mod rtu;

use super::{ DataPoint,ChannelConfig};

pub enum ModbusClientConfig {
    Rtu,
    Tcp
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ModbusRegisterMapType {
    Attributes(Vec<ModbusRegisterGroup>),
    Timeseries(Vec<ModbusRegisterGroup>)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModbusRegisterMap {
    pub attributes: Vec<ModbusRegisterGroup>,
    pub timeseries: Vec<ModbusRegisterGroup>,
}

// This struct could be use in server implementaion later
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModbusRegisterGroup {
    pub starting_address: u16, // Starting read address
    pub elements_count: u16,   // How many registers to read from starting_address
    pub data_points: Vec<ModbusDataPointReader>,
    pub data: Option<Vec<u16>> // Data that was read from modbus
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ModbusDataType {
    Double, // f64
    Float,  // f32
    Int32,  // i32
    Int16,  // i16
    UInt32, // u32
    UInt16, // u16
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModbusDataPointReader {
    pub data_offset: usize,
    pub register_count: usize,
    pub data_type: ModbusDataType,
    pub key_name: String,
}

impl ModbusDataPointReader {
    pub fn  parse(&self, data: Vec<u16>) -> DataPoint{
        // let (first, second, third) = dbg!(data.align_to::<u8>());

        log::debug!("Incommint Vec<u16>: {:?}", data);
        log::trace!("Data offset: {}", self.data_offset);
        let mut buff = ByteBuffer::new();
        for word in data {
            buff.write_u16(word);
        }
        // let transmuted = safce_transmute::transmute_to_bytes(data.as_slice());
        // let transmuted = bytemuck::cast_slice(data.as_slice());
        

        match self.data_type {
            ModbusDataType::Float => {
                // Float =  32bits /  2 registers

                buff.set_rpos(self.data_offset);
                let value = buff.read_f32();

                log::debug!("Value prased: {}", value);
                DataPoint {
                    key: self.key_name.clone(),
                    value: value.to_string(),
                    ts: None
                }
                    

            },
            ModbusDataType::Double => {
                buff.set_rpos(self.data_offset);
                let value = buff.read_f64();

                log::debug!("Value prased: {}", value);
                DataPoint {
                    key: self.key_name.clone(),
                    value: value.to_string(),
                    ts: None
                }
            },
            ModbusDataType::Int32 => {
                buff.set_rpos(self.data_offset);
                let value = buff.read_i32();

                log::debug!("Value prased: {}", value);
                DataPoint {
                    key: self.key_name.clone(),
                    value: value.to_string(),
                    ts: None
                }

            },
            ModbusDataType::UInt32 => {
                buff.set_rpos(self.data_offset);
                let value = buff.read_u32();

                log::debug!("Value prased: {}", value);
                DataPoint {
                    key: self.key_name.clone(),
                    value: value.to_string(),
                    ts: None
                }

            },
            ModbusDataType::UInt16 => {
                buff.set_rpos(min(self.data_offset, buff.len()));
                let value = buff.read_u16();

                log::debug!("Value prased: {}", value);
                DataPoint {
                    key: self.key_name.clone(),
                    value: value.to_string(),
                    ts: None
                }
            }
            _ => {
                todo!()

            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Hash, Eq, Clone)]
pub struct ModbusSlave {
    pub device_name: String,
    pub device_type: Option<String>,
    pub modbus_id: ModbusSlaveId,
    pub register_map: String,
    pub file_descriptor: Option<i32>

}

pub type ModbusSlaveId = u8;


#[derive(Serialize, Deserialize ,Debug)]
pub struct ModbusClientTcpConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub slaves: Vec<ModbusSlave>

}


// impl <'de> Deserialize <'de> for ModbusClientTcpConfig {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de> {
//         todo!()
//     }
// }

impl ChannelConfig for ModbusClientTcpConfig {
    fn serialize(config_string: String) -> Result<Self, Error> {
        from_str::<ModbusClientTcpConfig>(&config_string)

    }
}

// TODO: IMplemet Into trait for tokio_serial::SerialPortBuilder
#[derive(Debug, Deserialize, Clone)]
pub struct ModbusClientRtuConfig {
    pub name: String,
    pub port: String,
    pub baudrate: u32,
    pub parity: char,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub slaves: Vec<ModbusSlave>
}

impl ChannelConfig for ModbusClientRtuConfig {
    fn serialize(config_string: String) -> Result<Self, Error> where Self: Sized {
        from_str::<ModbusClientRtuConfig>(&config_string)
    }
}

// impl<'de> Deserialize<'de> for ModbusClientRtuConfig {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de> {
//             let s = String::deserialize(deserializer)?;
//             log::debug!("Deserializer ModbusClientRtuConfig: {:?}", s);
//             todo!();
//     }
// }

#[cfg(test)]
mod tests {
    use crate::channels::{ChannelConfig, modbus::ModbusSlave};

    use super::{ModbusClientTcpConfig, ModbusRegisterMap, ModbusDataPointReader};
    use std::fs;

    #[test]
    fn construct_modbus_tcp_channel() {
        let data = r#"
            name: Modbus Channel 1
            host: 192.168.1.1
            port: 502
            slaves: 
              - device_name: Elektromer1
                device_type: DEVICE_TYPE
                modbus_id: 1
                register_map: "./register_map/feafef.yml"  
        "#;

        let res = ModbusClientTcpConfig::serialize(data.to_string());

        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.host, "192.168.1.1");
        assert_eq!(res.port, 502);
        assert_eq!(res.slaves, vec![ModbusSlave{
            device_name: "Elektromer1".to_string(),
            device_type: Some("DEVICE_TYPE".to_string()),
            modbus_id: 1,
            register_map: "./register_map/feafef.yml".to_string()
        }]);

        
    }
    #[test]
    fn construct_register_map() {
        let data = r#"
            name: Modbus Channel 1
            host: 192.168.1.1
            port: 502
            slaves: 
              - device_name: Elektromer1
                device_type: DEVICE_TYPE
                modbus_id: 1
                register_map: "dist/register_maps/F&F_LE-03MW-CT.yml"  
        "#;
        let res = ModbusClientTcpConfig::serialize(data.to_string())
            .unwrap();
        
        let register_map_path = res.slaves[0].register_map.clone();
        let register_map_raw = fs::read_to_string(&register_map_path).unwrap();
        let register_map = serde_yaml::from_str::<ModbusRegisterMap>(&register_map_raw);
        println!("{:?}",register_map);
        assert!(register_map.is_ok());



    }
    #[test]
    fn test_datapoint_alignment() {
        let data: &[u16] = &[0x1000,0x2000,0x3000,0xFFFF];

        unsafe {
            let (first, second, third) = data.align_to::<u8>();
            println!("First: {:?}, Second: {:?}, Third: {:?}", first,second,third);
        }

    }
    #[test]
    fn construct_float_datapoint() {
        // let data: &[u16] = &[0x9654,0x4000];
        // let data: Vec<u16> = vec![0x4121, 0x999A];
        let data: Vec<u16> = vec![0x999A,0x4121];

        let show: f32 = 10.1;
        println!("f32 to bits: {:x}", show.to_bits());

        let structure = ModbusDataPointReader {
            data_offset: 0usize,
            register_count: 4usize,
            data_type: super::ModbusDataType::Float,
            key_name: String::from("TestingTimeseries")
        };

        // unsafe {
            let ret = structure.parse(data.to_vec());

            println!("Return: {:?}", ret);
        // }


    }
}