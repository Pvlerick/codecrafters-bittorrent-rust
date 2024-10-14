use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::hashes::Hashes;

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

impl Torrent {
    pub fn info_hash(&self) -> anyhow::Result<Vec<u8>> {
        let bytes = serde_bencode::to_bytes(&self.info)?;
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        Ok(hasher.finalize().into_iter().collect::<Vec<_>>())
    }

    pub fn total_len(&self) -> usize {
        match &self.info.keys {
            Keys::SingleFile { length } => *length,
            Keys::MultiFile { files } => files.iter().map(|i| i.length).sum(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: usize,
    pub pieces: Hashes,
    #[serde(flatten)]
    pub keys: Keys,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Keys {
    SingleFile { length: usize },
    MultiFile { files: Vec<File> },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct File {
    pub length: usize,
    pub path: Vec<String>,
}
