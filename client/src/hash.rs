use xxhash_rust::xxh3::xxh3_64;

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", xxh3_64(bytes))
}
