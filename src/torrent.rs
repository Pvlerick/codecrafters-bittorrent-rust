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
    pub fn info_hash(&self) -> anyhow::Result<[u8; 20]> {
        let bytes = serde_bencode::to_bytes(&self.info)?;
        Ok(sha1::hash(&bytes))
    }

    pub fn total_len(&self) -> usize {
        match &self.info.keys {
            Keys::SingleFile { length } => *length,
            Keys::MultiFile { files } => files.iter().map(|i| i.length).sum(),
        }
    }

    fn piece_length(&self) -> usize {
        self.info
            .piece_length
            .try_into()
            .expect("usize can't hold a u32, what kind of architecture are you running this on?")
    }

    pub fn pieces_count(&self) -> usize {
        self.info.pieces.0.len()
    }

    fn last_piece_size(&self) -> usize {
        match self.total_len() % self.piece_length() {
            0 => self.piece_length(),
            len => len,
        }
    }

    pub fn pieces_info(&self) -> Vec<PieceInfo> {
        let mut info = Vec::new();
        for i in 0..self.pieces_count() {
            info.push(PieceInfo {
                index: i,
                offset: i * self.piece_length(),
                length: if i == self.pieces_count() - 1 {
                    self.last_piece_size()
                } else {
                    self.piece_length()
                },
            })
        }
        info
    }

    /// A vector containing block division for the given piece in the given block size
    pub fn blocks_info(&self, piece_index: usize, block_size: usize) -> Option<Vec<BlockInfo>> {
        let pieces_info = self.pieces_info();
        let pieces_info = pieces_info.get(piece_index)?;
        let mut info = Vec::new();
        let blocks_count = (pieces_info.length + block_size - 1) / block_size;
        for i in 0..blocks_count {
            info.push(BlockInfo {
                offset: i * block_size,
                length: if i == blocks_count - 1 {
                    match pieces_info.length % block_size {
                        0 => block_size,
                        len => len,
                    }
                } else {
                    block_size
                },
            })
        }
        Some(info)
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

#[derive(Debug, PartialEq)]
pub struct PieceInfo {
    pub index: usize,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, PartialEq)]
pub struct BlockInfo {
    pub offset: usize,
    pub length: usize,
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
    use anyhow::Context;

    use crate::torrent::{BlockInfo, PieceInfo, Torrent};

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

    #[test]
    fn torrent_shorthands_1() -> anyhow::Result<()> {
        const FILE_SIZE: usize = 450;
        const PIECES_SIZE: usize = 120;
        let pieces_count: usize = (FILE_SIZE + PIECES_SIZE - 1) / PIECES_SIZE;
        let mut torrent_content = Vec::from(format!("d8:announce31:http://127.0.0.1:44381/announce4:infod6:lengthi{FILE_SIZE}e4:name15:faketorrent.iso12:piece lengthi{PIECES_SIZE}e6:pieces{}:", pieces_count * 20).as_bytes());
        torrent_content.extend_from_slice(&vec![0; pieces_count * 20]);
        torrent_content.extend_from_slice(b"ee");

        let torrent = Torrent::from_bytes(&torrent_content)?;

        assert_eq!(450, torrent.total_len());
        assert_eq!(4, torrent.pieces_count());
        assert_eq!(90, torrent.last_piece_size());
        assert_eq!(
            Some(&PieceInfo {
                index: 0,
                offset: 0,
                length: 120
            }),
            torrent.pieces_info().get(0)
        );
        assert_eq!(
            Some(&PieceInfo {
                index: 3,
                offset: 360,
                length: 90
            }),
            torrent.pieces_info().last()
        );
        assert_eq!(
            Some(&BlockInfo {
                offset: 0,
                length: 60,
            }),
            torrent
                .blocks_info(0, 60)
                .context("requested piece does not exist")?
                .get(0)
        );
        assert_eq!(
            Some(&BlockInfo {
                offset: 82,
                length: 8,
            }),
            torrent
                .blocks_info(3, 41)
                .context("requested piece does not exist")?
                .get(2)
        );

        Ok(())
    }

    #[test]
    fn torrent_shorthands_2() -> anyhow::Result<()> {
        const FILE_SIZE: usize = 300;
        const PIECES_SIZE: usize = 100;
        let pieces_count: usize = (FILE_SIZE + PIECES_SIZE - 1) / PIECES_SIZE;
        let mut torrent_content = Vec::from(format!("d8:announce31:http://127.0.0.1:44381/announce4:infod6:lengthi{FILE_SIZE}e4:name15:faketorrent.iso12:piece lengthi{PIECES_SIZE}e6:pieces{}:", pieces_count * 20).as_bytes());
        torrent_content.extend_from_slice(&vec![0; pieces_count * 20]);
        torrent_content.extend_from_slice(b"ee");

        let torrent = Torrent::from_bytes(&torrent_content)?;

        assert_eq!(300, torrent.total_len());
        assert_eq!(3, torrent.pieces_count());
        assert_eq!(100, torrent.last_piece_size());
        assert_eq!(
            Some(&PieceInfo {
                index: 1,
                offset: 100,
                length: 100
            }),
            torrent.pieces_info().get(1)
        );
        assert_eq!(
            Some(&PieceInfo {
                index: 2,
                offset: 200,
                length: 100
            }),
            torrent.pieces_info().last()
        );
        assert_eq!(
            Some(&BlockInfo {
                offset: 0,
                length: 41,
            }),
            torrent
                .blocks_info(0, 41)
                .context("requested piece does not exist")?
                .get(0)
        );
        assert_eq!(
            Some(&BlockInfo {
                offset: 82,
                length: 18,
            }),
            torrent
                .blocks_info(0, 41)
                .context("requested piece does not exist")?
                .get(2)
        );

        Ok(())
    }
}
