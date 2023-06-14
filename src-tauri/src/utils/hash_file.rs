use std::fs::File;
use std::io;
use std::path::Path;
use sha1::{Sha1, Digest};

pub fn hash_file(path: &Path) -> String {
    let mut file = File::open(path).unwrap();
    let mut hasher = Sha1::new();
    io::copy(&mut file, &mut hasher).unwrap();
    let hash = hasher.finalize();
    let mut buf = [0u8; 40];
    let hash = base16ct::lower::encode_str(&hash, &mut buf).unwrap();
    hash.to_string()
}
