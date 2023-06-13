use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use sha2::{Sha256, Digest};

pub fn sync_minecraft() {
    
}

fn hash_file(path: PathBuf) -> String {
    let mut file = File::open(path).unwrap();
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).unwrap();
    let hash = hasher.finalize();
    let mut buf = [0u8; 64];
    let hash = base16ct::lower::encode_str(&hash, &mut buf).unwrap();
    hash.to_string()
}
