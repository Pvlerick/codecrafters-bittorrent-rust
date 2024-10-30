use anyhow::Context;
use reqwest::Url;

use crate::{magnet_links::MagnetLink, torrent::Torrent};

pub const PEER_ID: &str = "alice_is_1_feet_tall";

pub trait TrackerInfo {
    fn tracker_url(&self) -> anyhow::Result<Url>;
}

impl TrackerInfo for Torrent {
    fn tracker_url(&self) -> anyhow::Result<Url> {
        tracker_url(&self.announce, &self.info_hash()?, self.total_len())
    }
}

impl TrackerInfo for MagnetLink {
    fn tracker_url(&self) -> anyhow::Result<Url> {
        tracker_url(&self.announce.to_string(), &self.info_hash, 999)
    }
}

fn tracker_url(announce_url: &str, info_hash: &[u8; 20], left: usize) -> anyhow::Result<Url> {
    let info_hash = hex::encode(info_hash)
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|i| format!("%{}{}", i[0], i[1]))
        .collect::<Vec<_>>()
        .concat();

    Url::parse_with_params(
        format!("{}?info_hash={}", announce_url, info_hash).as_str(),
        &[
            ("peer_id", PEER_ID),
            ("port", "6881"),
            ("uploaded", "0"),
            ("downloaded", "0"),
            ("left", format!("{}", left.to_string()).as_str()),
            ("compact", "1"),
        ],
    )
    .context("creating tracker url")
}
