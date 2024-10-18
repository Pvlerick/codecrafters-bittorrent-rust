use sha1::{Digest, Sha1};

pub fn hash(bytes: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    TryInto::<[u8; 20]>::try_into(hasher.finalize()).expect("hash is not 20 bytes")
}
