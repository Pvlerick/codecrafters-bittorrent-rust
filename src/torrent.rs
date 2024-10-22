use anyhow::Context;
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};

use crate::{hashes::Hashes, sha1};

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

impl Torrent {
    pub fn info_hash(&self) -> anyhow::Result<Vec<u8>> {
        let bytes = serde_bencode::to_bytes(&self.info)?;
        Ok(sha1::hash(&bytes).to_vec())
    }

    pub fn total_len(&self) -> usize {
        match &self.info.keys {
            Keys::SingleFile { length } => *length,
            Keys::MultiFile { files } => files.iter().map(|i| i.length).sum(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_base64(content: &str) -> anyhow::Result<Torrent> {
        Ok(
            serde_bencode::from_bytes(&general_purpose::STANDARD.decode(content)?)
                .context("parse torrent file")?,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn from_bytes(content: &[u8]) -> anyhow::Result<Torrent> {
        Ok(serde_bencode::from_bytes(&content).context("parse torrent file")?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: u32,
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

#[cfg(test)]
mod test {
    use crate::torrent::Torrent;

    #[test]
    fn torrent_with_hash_and_pieces_1() -> anyhow::Result<()> {
        let torrent = Torrent::from_base64("ZDg6YW5ub3VuY2U1NTpodHRwOi8vYml0dG9ycmVudC10ZXN0LXRyYWNrZXIuY29kZWNyYWZ0ZXJzLmlvL2Fubm91bmNlMTA6Y3JlYXRlZCBieTEzOm1rdG9ycmVudCAxLjE0OmluZm9kNjpsZW5ndGhpODIwODkyZTQ6bmFtZTE5OmNvbmdyYXR1bGF0aW9ucy5naWYxMjpwaWVjZSBsZW5ndGhpMjYyMTQ0ZTY6cGllY2VzODA6PUKiDtsc+EDNNSjTqekh22M4pGNp+IWzmIpS/7A1kZhUArbVKFlAq3aGnmycHxAflPOd4VPkaL5qY49Pve1o0C3gEaK2h/dbWDP0bM6OPpxlZQ==")?;

        assert_eq!(
            "http://bittorrent-test-tracker.codecrafters.io/announce",
            torrent.announce
        );
        assert_eq!(820892, torrent.total_len());
        assert_eq!(
            "1cad4a486798d952614c394eb15e75bec587fd08",
            hex::encode(&torrent.info_hash()?)
        );
        assert_eq!(262144, torrent.info.piece_length);
        assert_eq!(
            vec![
                "3d42a20edb1cf840cd3528d3a9e921db6338a463",
                "69f885b3988a52ffb03591985402b6d5285940ab",
                "76869e6c9c1f101f94f39de153e468be6a638f4f",
                "bded68d02de011a2b687f75b5833f46cce8e3e9c"
            ],
            torrent
                .info
                .pieces
                .0
                .iter()
                .map(|i| hex::encode(i))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn torrent_with_hash_and_pieces_2() -> anyhow::Result<()> {
        let torrent = Torrent::from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

        assert_eq!("http://127.0.0.1:44381/announce", torrent.announce);
        assert_eq!(2097152, torrent.total_len());
        assert_eq!(
            "a18a79fa44e045b1e13879166d35823e848419f8",
            hex::encode(&torrent.info_hash()?)
        );
        assert_eq!(262144, torrent.info.piece_length);
        assert_eq!(
            vec![
                "ddf33172599fda84f0a209a3034f79f0b8aa5e22",
                "795a618a1ee5275e952843b01a56ae4e142752ef",
                "cdae2ef532d611a46b2cf7b64d578c09b3ac0b6e",
                "098dadc0c19436f1927ea27b90eb18b1a2820a23",
                "8fa5355419886d9ec56e86cd7791343e9379de18",
                "1caeaceb15fd1134b1b4b21fad04125b227b4dcf",
                "fa586e20d579a4de76090e12bd0a3d9b1c539f3e",
                "aec2d7eb1db539c2a9d24d023fb916b79234b769"
            ],
            torrent
                .info
                .pieces
                .0
                .iter()
                .map(|i| hex::encode(i))
                .collect::<Vec<_>>()
        );
        Ok(())
    }
}
