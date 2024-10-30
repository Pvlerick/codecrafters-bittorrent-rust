use crate::{
    magnet_links::MagnetLink,
    torrent::{BlockInfo, Info, PieceInfo, Torrent},
};

pub trait TorrentInfo {
    fn info(&self) -> &Info;

    fn total_len(&self) -> usize {
        self.info().total_len()
    }

    fn info_hash(&self) -> anyhow::Result<[u8; 20]> {
        let bytes = serde_bencode::to_bytes(&self.info())?;
        Ok(crate::sha1::hash(&bytes))
    }

    fn piece_length(&self) -> usize {
        self.info()
            .piece_length
            .try_into()
            .expect("usize can't hold a u32, what kind of architecture are you running this on?")
    }

    fn pieces_count(&self) -> usize {
        self.info().pieces.0.len()
    }

    fn last_piece_size(&self) -> usize {
        match self.total_len() % self.piece_length() {
            0 => self.piece_length(),
            len => len,
        }
    }

    fn blocks_info(&self, piece_index: usize, block_size: usize) -> Option<Vec<BlockInfo>> {
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

    fn pieces_info(&self) -> Vec<PieceInfo> {
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
}

impl TorrentInfo for Torrent {
    fn info(&self) -> &Info {
        &self.info
    }
}

impl TorrentInfo for (MagnetLink, Info) {
    fn info(&self) -> &Info {
        &self.1
    }
}
