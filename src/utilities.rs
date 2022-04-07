use std::fs;
use std::io::Error;
// use hex_literal::hex;
use sha2::{Sha256, Digest};
use hex::ToHex;

pub fn open_and_read(path: String) -> Result<(String, String), Error> {
    let data = fs::read_to_string(path.clone())?;
    let mut hasher = Sha256::new();
    hasher.update(data.clone());
    let content_hash =  hasher.finalize();
    log::debug!("Hashed path: {} with hash: {:?}", path, content_hash);

    Ok((data, content_hash.encode_hex::<String>()))
}