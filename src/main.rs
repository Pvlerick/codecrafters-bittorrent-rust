use std::io::{stdout, Write};

use anyhow::Context;
use bittorrent_starter_rust::{
    bedecode::ItemIterator,
    bt_client::BtClient,
    cli::{Args, Command},
    magnet_links::MagnetLink,
    torrent::Torrent,
};
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode { value } => {
            let mut encoded_value = ItemIterator::new(value.as_bytes());
            println!("{}", encoded_value.next().unwrap()?);
            Ok(())
        }
        Command::Info { torrent } => {
            let torrent = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent).context("parse torrent file")?;
            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.total_len());
            println!("Info Hash: {}", hex::encode(torrent.info_hash()?));
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            for hash in torrent.info.pieces.0 {
                println!("{}", hex::encode(hash));
            }
            Ok(())
        }
        Command::Peers { torrent } => {
            let torrent = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent).context("parse torrent file")?;
            let client = BtClient::new();
            for peer in client.get_peers(&torrent)? {
                println!("{peer}");
            }
            Ok(())
        }
        Command::Handshake { torrent, peer } => {
            let torrent = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent).context("parse torrent file")?;
            let client = BtClient::new();
            let peer_id = client.handshake(&torrent, peer)?;
            println!("Peer ID: {}", hex::encode(peer_id));
            Ok(())
        }
        Command::DownloadPiece {
            output,
            torrent,
            start,
        } => {
            let torrent = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent).context("parse torrent file")?;
            let client = BtClient::new();
            let peers = client.get_peers(&torrent)?;
            let peer = peers.first().expect("no peer after contacting tracker");
            let content = client.download_piece(&torrent, *peer, start)?;
            match output {
                Some(file) => std::fs::write(file, &content)?,
                None => stdout().write_all(&content)?,
            }
            Ok(())
        }
        Command::Download { output, torrent } => {
            let torrent = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent).context("parse torrent file")?;
            let client = BtClient::new();
            let peers = client.get_peers(&torrent)?;
            let peer = peers.first().expect("no peer after contacting tracker");
            let content = client.download(&torrent, *peer)?;
            match output {
                Some(file) => std::fs::write(file, &content)?,
                None => stdout().write_all(&content)?,
            }
            Ok(())
        }
        Command::MagnetParse { link } => {
            let magnet_link = MagnetLink::parse(link).context("parsing magnet link")?;
            println!("Tracker URL: {}", magnet_link.announce);
            println!("Info Hash: {}", hex::encode(magnet_link.info_hash));
            Ok(())
        }
    }
}
