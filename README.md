# STS-Gateway

Single binary program to read from industrial protocols and other *IoT*  protocols.
Low memory usage, thanks to rust's ownership model.
Fast...  very fast... 

Currently it does not support any type of writing or passing messages to other via any implemented protocols.
Read first. Right now the goal is to have *correct* and **fast** implementations of reading for industrial protocols. 
## This should not be used in production.
Its still very experimental and most of the core features are subjects to change.

## Why?
I needed **faster** implementation of modbus TCP/RTU and wanted some nice to have features. When dealing with a *IOT Edge* linux based devices. My goal is to support other industrial protocols like: M-Bus, IEC62056-21...


## Key Features:
- Modbus Client TCP/RTU
- Sqlite Storage
- Mqtt data export

## Features Planned: 
- [x] Full modbus implementation over TCP and Serial
- [] Better read period configuration and scheduling
- [] File Storage (Append only log) 
- [] Other types of data exporting/sending
- [x] Auto create desired data folder
- [] Documentation

## Dependencies
If you staticaly compile this won't experiece any `libc` version problems
You will need following dependencies:
- Zst 
---
## How to use
First of, project is not yet very configurable. In the ``dist`` folder are some basic configuration files. The Root config file is `sts_gateway.yml` from which sts-gateway looks for **Channel Configs**

Lets introduce a conpect of **Register Maps**. Which are essentialy a predefined data points to read on a specific device. And thus should be use on type of protocol/devices. 
Example:

You have 10 devices, electricity meters. You don't want to write the same register read configuration for each device if only a single paramter is changing eg: Modbus slave id.
So you define a single config map and apply it to every device in channel config.

Now you will have to create any neccesary directories that you set to root config. It does not create them autoticaly

### Logging
For logging we use Log4rs crate
You should probably set `logging_config`  in root config(`sts_gateway.tml`) to `<path_to...>/logs.yml` 

### Modbus
TCP and RTU are both supported
Keep in mind only function 3 or read holding registers is supported.
In `modbus_rtu.yml` and `modbus_tcp.yml` you can find basic configuration of multiple slaves and its corresponding register map


## Acknowledgments
Big inspiration [Thingsboard Gateway](https://github.com/thingsboard/thingsboard-gateway)
First contributor [silen_z](https://github.com/silen-z/)
