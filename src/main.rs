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
    use base64::{engine::general_purpose, Engine};
    use reqwest::{Method, Url};
    use reqwest_mock::{StubClient, StubDefault, StubSettings, StubStrictness};

    fn torrent_from_base64(content: &str) -> anyhow::Result<Torrent> {
        Ok(
            serde_bencode::from_bytes(&general_purpose::STANDARD.decode(content)?)
                .context("parse torrent file")?,
        )
    }

    #[test]
    fn info_with_hash_and_pieces_1() -> anyhow::Result<()> {
        let torrent = torrent_from_base64("ZDg6YW5ub3VuY2U1NTpodHRwOi8vYml0dG9ycmVudC10ZXN0LXRyYWNrZXIuY29kZWNyYWZ0ZXJzLmlvL2Fubm91bmNlMTA6Y3JlYXRlZCBieTEzOm1rdG9ycmVudCAxLjE0OmluZm9kNjpsZW5ndGhpODIwODkyZTQ6bmFtZTE5OmNvbmdyYXR1bGF0aW9ucy5naWYxMjpwaWVjZSBsZW5ndGhpMjYyMTQ0ZTY6cGllY2VzODA6PUKiDtsc+EDNNSjTqekh22M4pGNp+IWzmIpS/7A1kZhUArbVKFlAq3aGnmycHxAflPOd4VPkaL5qY49Pve1o0C3gEaK2h/dbWDP0bM6OPpxlZQ==")?;

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
    fn info_with_hash_and_pieces_2() -> anyhow::Result<()> {
        let torrent = torrent_from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

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
    fn info_request_peers() -> anyhow::Result<()> {
        let torrent = torrent_from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

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
