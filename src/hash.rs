use serde::Serialize;
use xxhash_rust::xxh3::{xxh3_64, Xxh3};

pub fn compute_visitor_id<T1: Serialize, T2: Serialize>(a: &T1, b: &T2) -> String {
    let mut hasher = Xxh3::new();
    if let Ok(s) = serde_json::to_string(a) {
        hasher.update(s.as_bytes());
    }
    hasher.update(b"::");
    if let Ok(s) = serde_json::to_string(b) {
        hasher.update(s.as_bytes());
    }
    format!("{:016x}", hasher.digest())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", xxh3_64(bytes))
}
