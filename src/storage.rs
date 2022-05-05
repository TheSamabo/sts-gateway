
use chrono::Utc;
use rusqlite::{Connection, Result as SqliteResult, OpenFlags, params, backup, Error as SqliteError};
use rusqlite::NO_PARAMS;
use serde_json::Value;
use core::time;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::io::BufWriter;
use zstd;

use crate::definitions::{Storage, StorageSizeManagement};

// fn insert_message(message: Value, ts: Option<i64> , device_name: &str, con: &Connection) -> SqliteResult<> {

// }

#[derive(Debug, PartialEq)]
pub struct Insert {
    pub ts: i64,
    pub device_name: String,
    // In the final form, that we can store in DB
    pub timeseries_message: Option<String>,
    pub attributes_message: Option<String>
}

pub enum SqliteStorageTruncate {
    FixedWindow(chrono::Duration)
}

pub enum SqliteStorageAction {
    InsertAttributes(Insert),
    InsertTimeseries(Insert),
    InsertBoth(Insert),
    CloseDB,
    // BackupDB need a string  that is the destination of backup db
    BackupDB(String),
    Truncate(SqliteStorageTruncate), // Start 
    Timeout
}

pub struct SqliteStorage {
    connection: Connection,
    rx: mpsc::Receiver<SqliteStorageAction>,
    data_dir: PathBuf
}

impl SqliteStorage{

    /// Expects parameter: 
    /// path: String  - path to folder where it will store database and backups 
    /// rx: mpsc::Receiver<SqliteStorageAction> - Receiver so that we can send Actions to do somethings
    pub fn new(
        data_folder: String,
        rx: mpsc::Receiver<SqliteStorageAction>) -> SqliteResult<Self> {
        let mut data_path = PathBuf::new();
        let mut data_dir = PathBuf::new();
        let path = data_folder;
        match path.ends_with(".db") {
            true => {
                data_path = Path::new(&path).to_path_buf();
            },
            false => {
            let tmp_data_folder = Path::new(&path).to_path_buf();
            data_path = tmp_data_folder.join("data.db");
                log::debug!("Apeended \"data.db\" to database folder path: {}", data_path.display());
            }
        };

        // for entry in data_path.ancestors() {

            data_dir = data_path.parent().expect("Expected parent directory").to_path_buf();
        // }

        let con = Connection::open(data_path)?;
        match con.execute(r#"CREATE TABLE IF NOT EXISTS 
            messages(ts INTEGER, device_name TEXT, timeseries_message TEXT, attributes_message TEXT)"#, []) {
                Ok(modified_rows) => {
                    log::debug!("Created table \"messages\" in database! Affected rows: {}", modified_rows);
                },
                Err(e) => {
                    log::error!("Could not create table to store messages with rusqlite, Error: {:?}", e);
                }
            }
        
        Ok(Self {
            connection: con,
            rx,
            data_dir
        })
    }

    /// TABLE COLUMNS:
    ///    ts: i64 | device_name: TEXT | timeseries_message | attributes_message

    pub fn process(&mut self) {
        match self.rx.recv().unwrap() {

            SqliteStorageAction::InsertBoth(insert) => {
                
                log::debug!("Inserting message: {:?} to Database", insert);
                
                match self.connection.transaction() {
                    Ok(mut t) => {
                        t.set_drop_behavior(rusqlite::DropBehavior::Commit);
                        
                        match t.execute(r#"INSERT INTO messages
                            (ts, device_name, timeseries_message, attributes_message)
                            VALUES(?1, ?2, ?3, ?4)"#, params![
                                 insert.ts,
                                 insert.device_name,
                                 // These are safe unwraps, at least i hope so
                                 insert.timeseries_message.unwrap(),
                                 insert.attributes_message.unwrap()
                             ]) {
                                 Ok(_rows_affected) => log::debug!("Wrote {} rows to DB", _rows_affected),
                                 Err(e) => log::error!("Error executing SQL: {:?}", e)
                             };

                        match t.finish() {
                            Ok(_) => log::debug!("Data was writen to storage"),
                            Err(err) => {
                                log::error!("Rollback failed to save transaction: {:?}", err);
                                panic!("Rollback failed to save transaction");
                            }
                        }
                    },
                    Err(e) => log::error!("SqliteTransactionBegin Error: {:?}", e)
                };

            },
            SqliteStorageAction::BackupDB(path) => {
                log::info!("Starting Database backup ...");
                backup_db(&self.connection, path, self.data_dir.clone()).unwrap();
            },
            SqliteStorageAction::Truncate(trun) => {
                log::info!("Starting trucation process...");
                match trun {
                    SqliteStorageTruncate::FixedWindow(older_than) => {
                        log::info!("Selected FixedWindow truncation...");
                        match truncate_fixed_window(&self.connection, older_than) {
                            Ok(_) => log::info!("Successfuly truncated"),
                            Err(e) => log::error!("Error truncating database: {:?}", e)
                        }
                    }
                }
            }
            SqliteStorageAction::CloseDB => {
                log::info!("Closing DB...");

            },
            SqliteStorageAction::Timeout => {
                log::trace!("Recv timed out");
            },
            _ => {}
        };
    }
}

pub fn truncate_fixed_window(con: &Connection, older_than: chrono::Duration) -> SqliteResult<()> {
    let now = Utc::now();
    let old = now - older_than;
    log::info!("Truncating data older than {} UTC", old.to_string());
    let deleted = con.execute(r#"
        DELETE FROM messages WHERE ts NOT BETWEEN ?1 AND ?2
    "#, params![old.timestamp_millis(), now.timestamp_millis()])?;
    log::info!("Truncated {} rows...", deleted);
    Ok(())
}


pub fn backup_db(
    src: &Connection,
    dst_path: String,
    data_dir: PathBuf
) -> SqliteResult<()> {
    let dest_path = PathBuf::from(&dst_path);
    let mut dst = Connection::open(dst_path.clone())?;
    let backup = backup::Backup::new(src, &mut dst)?;
    let progress = |now: backup::Progress| {
        if now.pagecount > 0 {
            let percentage = (now.pagecount - now.remaining) / now.pagecount;
            log::info!("Backup progress: {}%", percentage);
        } else {
            log::info!("Backup status: STATUS = {} , REMAINIG = {}", now.pagecount, now.remaining);
        }

    } ;
    match  backup.run_to_completion(5, time::Duration::from_millis(250), Some(progress)) {
        Ok(_) => {
            log::info!("Backup Complete!");
            
            if Path::exists(Path::new(&dest_path))  {
                log::debug!("Starting Compression Thread!");
                thread::spawn(move || {
                    let dest_path = dest_path;
                    log::trace!("Compression Thread started...");
                    let database_bytes = fs::read(dest_path.clone()).expect("Could not read database file");
                    let compress = zstd::bulk::compress(database_bytes.as_slice(), 1).unwrap();
                    log::trace!("Compressed database: From {:?} To {:?}", database_bytes.len(),compress.len());
                    log::trace!("Data dir in compression thread: {}", data_dir.display());
                    let compressed_file = data_dir.join(Path::new("backup/").join(dest_path.with_extension("db.zst")));
                    log::trace!("Compressed file path: {}", compressed_file.display());
                    fs::write(compressed_file, compress).unwrap();
                    fs::remove_file(dest_path).unwrap();
                });
            }
            Ok(())
        },
        Err(e) => {
            log::error!("Error backing up database: {:?}", e);
            Err(e)
        }
    }
}