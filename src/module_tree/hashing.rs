use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn hash_file(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|data| {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    })
}
