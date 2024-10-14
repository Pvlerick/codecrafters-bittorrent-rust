use std::path::PathBuf;

use anyhow::Context;
use bittorrent_starter_rust::{
    bedecode::{Field, Item, ItemIterator},
    torrent::Torrent,
};
use clap::{Parser, Subcommand};
use reqwest::Url;
use reqwest_mock::{Client, DirectClient};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about= None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Decode { value: String },
    Info { torrent: PathBuf },
    Peers { torrent: PathBuf },
}

const PEER_ID: &str = "alice_is_1_feet_tall";

struct BtClient<T: Client> {
    client: T,
}

impl<T: Client> BtClient<T> {
    fn new(client: T) -> Self {
        Self { client }
    }

    fn get_peers(&self, torrent: Torrent) -> anyhow::Result<Vec<String>> {
        let info_hash = hex::encode(torrent.info_hash()?)
            .chars()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|i| format!("%{}{}", i[0], i[1]))
            .collect::<Vec<_>>()
            .concat();
        let tracker = Url::parse_with_params(
            format!("{}?info_hash={}", torrent.announce, info_hash).as_str(),
            &[
                ("peer_id", PEER_ID),
                ("port", "6881"),
                ("uploaded", "0"),
                ("downloaded", "0"),
                ("left", format!("{}", torrent.total_len()).as_str()),
                ("compact", "1"),
            ],
        )
        .unwrap();
        let res = self.client.get(tracker).send().unwrap();
        let mut iter = ItemIterator::new(&res.body);
        if let Ok(Item::Dict(Field { payload, .. })) = iter.next().unwrap() {
            if let Some(Item::Bytes(Field { payload: peers, .. })) = payload.get("peers") {
                Ok(peers
                    .iter()
                    .collect::<Vec<_>>()
                    .chunks(6)
                    .into_iter()
                    .map(|i| {
                        let address: [u8; 4] = i[0..4]
                            .iter()
                            .map(|j| **j)
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap();
                        let port = u16::from_be_bytes(
                            i[4..6]
                                .iter()
                                .map(|j| **j)
                                .collect::<Vec<_>>()
                                .try_into()
                                .unwrap(),
                        );
                        format!("{}:{}", std::net::IpAddr::from(address).to_string(), port)
                    })
                    .collect::<Vec<_>>())
            } else {
                panic!("can't find peers")
            }
        } else {
            panic!("can't find response dict")
        }
    }
}

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
            let client = BtClient::new(DirectClient::new());
            for peer in client.get_peers(torrent)? {
                println!("{}", peer);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use reqwest::{Method, Url};
    use reqwest_mock::{StubClient, StubDefault, StubSettings, StubStrictness};

    #[test]
    fn info_request_peers() -> anyhow::Result<()> {
        let torrent = Torrent::from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

        let mut client = StubClient::new(StubSettings {
            default: StubDefault::Error,
            strictness: StubStrictness::MethodUrl,
        });

        let response = b"d8:completei2e10:downloadedi1e10:incompletei1e8:intervali1921e12:min intervali960e5:peers18:tttt09eeee18xxxx27e";
        let _ = client
            .stub(
                Url::parse("http://127.0.0.1:44381/announce?info_hash=%a1%8a%79%fa%44%e0%45%b1%e1%38%79%16%6d%35%82%3e%84%84%19%f8&peer_id=alice_is_1_feet_tall&port=6881&uploaded=0&downloaded=0&left=2097152&compact=1")
                .unwrap(),
            )
            .method(Method::GET)
            .response()
            .body(response.to_vec())
            .mock();

        let bt_client = BtClient::new(client);

        assert_eq!(
            vec![
                "116.116.116.116:12345",
                "101.101.101.101:12600",
                "120.120.120.120:12855"
            ],
            bt_client.get_peers(torrent)?
        );

        Ok(())
    }

    // #[test]
    // fn sandbox() {
    //     // let x: serde_json::Value = serde_bencode::from_str("d8:completei2e10:downloadedi1e10:incompletei1e8:intervali1921e12:min intervali960e5:peers18:tttt09eeee18xxxx27e").unwrap();
    //     // let x: serde_json::Value = serde_bencode::from_str("d8:completei2ee").unwrap();
    //     let x: serde_json::Value = serde_bencode::from_bytes(b"3:foo").unwrap();
    //     println!("{}", x);
    //     assert!(false);
    // }
}
