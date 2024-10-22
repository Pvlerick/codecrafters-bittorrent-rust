use std::{
    collections::HashSet,
    fmt::Debug,
    io::{Read, Write},
    iter,
    net::{SocketAddrV4, TcpStream},
};

use anyhow::{anyhow, Context};
use reqwest::Url;

use crate::{
    peer_messages::{Handshake, Message},
    torrent::Torrent,
    tracker,
};

pub trait HttpClient {
    fn get(&self, url: Url) -> anyhow::Result<Vec<u8>>;
}

impl HttpClient for reqwest::blocking::Client {
    fn get(&self, url: Url) -> anyhow::Result<Vec<u8>> {
        match self.get(url).send() {
            Ok(mut response) => {
                let mut buf = Vec::new();
                response.copy_to(&mut buf)?;
                Ok(buf)
            }
            Err(err) => return Err(err.into()),
        }
    }
}

const PEER_ID: &str = "alice_is_1_feet_tall";

pub struct BtClient<T: HttpClient> {
    client: T,
    block_size: u32,
}

impl BtClient<reqwest::blocking::Client> {
    pub fn new() -> Self {
        BtClient::<reqwest::blocking::Client>::with_client(reqwest::blocking::Client::new())
    }

    #[allow(dead_code)]
    pub(crate) fn with_block_size(block_size: u32) -> Self {
        BtClient::<reqwest::blocking::Client>::with_client_and_block_size(
            reqwest::blocking::Client::new(),
            block_size,
        )
    }
}

impl<T: HttpClient> BtClient<T> {
    pub fn with_client(client: T) -> Self {
        Self {
            client,
            block_size: 16 * 1024,
        }
    }

    fn with_client_and_block_size(client: T, block_size: u32) -> Self {
        Self { client, block_size }
    }

    pub fn get_peers(&self, torrent: &Torrent) -> anyhow::Result<Vec<SocketAddrV4>> {
        let info_hash = hex::encode(torrent.info_hash()?)
            .chars()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|i| format!("%{}{}", i[0], i[1]))
            .collect::<Vec<_>>()
            .concat();

        let tracker_url = Url::parse_with_params(
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
        .context("creating tracker url")?;

        let res = self.client.get(tracker_url)?;

        let res: tracker::Response =
            serde_bencode::from_bytes(&res).context("parse tracker get response")?;

        Ok(res.peers.0)
    }

    pub fn handshake(&self, torrent: &Torrent, peer: SocketAddrV4) -> anyhow::Result<[u8; 20]> {
        let mut tcp_stream = TcpStream::connect(peer).context("opening socket to peer")?;

        let res = self.shake_hands(&mut tcp_stream, torrent)?;

        Ok(Handshake::from(res).peer_id)
    }

    fn shake_hands<S: Read + Write + Debug>(
        &self,
        stream: &mut S,
        torrent: &Torrent,
    ) -> anyhow::Result<[u8; 68]> {
        let message = Handshake::new(
            torrent
                .info_hash()?
                .as_slice()
                .try_into()
                .context("invalid info hash length")?,
            PEER_ID.as_bytes().try_into().context("invalid peer id")?,
        );

        stream.write_all(&message.to_bytes())?;
        let _ = stream.flush();
        let mut buf = [0u8; 68];
        stream.read_exact(&mut buf)?;

        Ok(buf)
    }

    pub fn download_piece(
        &self,
        torrent: Torrent,
        peer: SocketAddrV4,
        index: u32,
    ) -> anyhow::Result<Vec<u8>> {
        let mut tcp_stream = TcpStream::connect(peer).context("opening socket to peer")?;
        self.handshake(&torrent, peer)
            .context("shaking hands with peer")?;
        self.piece_download(&mut tcp_stream, torrent, index)
    }

    fn piece_download<S: Read + Write + Debug>(
        &self,
        stream: &mut S,
        torrent: Torrent,
        index: u32,
    ) -> anyhow::Result<Vec<u8>> {
        use crate::peer_messages::Message::*;
        use state::State::*;
        let mut state = WaitingForBitField;
        let mut piece = vec![0u8; torrent.info.piece_length as usize];
        let mut collected_blocks = HashSet::new();
        loop {
            if collected_blocks
                .iter()
                .map(|k: &(u32, u32)| k.1 as usize)
                .sum::<usize>()
                == piece.len()
            {
                break;
            }

            let msg = Message::read_from(stream)?;

            match (&state, msg) {
                (WaitingForBitField, BitField { .. }) => {
                    stream.write_all(&Interested.to_bytes()?)?;
                    state = WaitingForUnchoke;
                }
                (WaitingForUnchoke, Unchoke) => {
                    let mut block_index = 0;
                    let mut rest = torrent.info.piece_length;
                    let blocks = iter::from_fn(move || {
                        return match rest {
                            0 => None,
                            _ => {
                                let ret = Some((
                                    block_index * self.block_size,
                                    if rest >= self.block_size {
                                        self.block_size
                                    } else {
                                        rest
                                    },
                                ));
                                block_index += 1;
                                rest = if rest >= self.block_size {
                                    rest - self.block_size
                                } else {
                                    0
                                };
                                ret
                            }
                        };
                    })
                    .collect::<Vec<_>>();

                    for (offset, block_size) in blocks {
                        stream.write_all(
                            &Request {
                                index,
                                begin: offset,
                                length: block_size,
                            }
                            .to_bytes()?,
                        )?;
                    }

                    state = WaitingForPieceBlock;
                }
                (
                    WaitingForPieceBlock,
                    Piece {
                        index: piece_index,
                        begin,
                        block,
                    },
                ) if piece_index == index => {
                    let key = (begin, block.len() as u32);
                    let begin = begin as usize;
                    piece[begin..begin + block.len()].copy_from_slice(&block);
                    collected_blocks.insert(key);
                }
                (_, msg) => return Err(anyhow!("unknown message received: '{}'", &msg)),
            }
        }

        Ok(piece)
    }
}

mod state {
    pub enum State {
        WaitingForBitField,
        WaitingForUnchoke,
        WaitingForPieceBlock,
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
    };

    use anyhow::anyhow;
    use base64::{engine::general_purpose, Engine};
    use reqwest::{Method, Url};
    use reqwest_mock::{StubClient, StubDefault, StubSettings, StubStrictness};

    use crate::{bt_client::BtClient, peer_messages::Message, sha1, torrent::Torrent};

    use super::HttpClient;

    impl HttpClient for StubClient {
        fn get(&self, url: Url) -> anyhow::Result<Vec<u8>> {
            match reqwest_mock::Client::get(self, url)
                .send()
                .map_err(|e| anyhow!("receiver is gone: {}", e.description()))
            {
                Ok(response) => Ok(response.body),
                Err(err) => Err(err),
            }
        }
    }

    #[test]
    fn get_peers() -> anyhow::Result<()> {
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

        let bt_client = BtClient::with_client(client);

        assert_eq!(
            vec![
                "116.116.116.116:12345",
                "101.101.101.101:12600",
                "120.120.120.120:12855"
            ],
            bt_client
                .get_peers(&torrent)?
                .iter()
                .map(|i| format!("{i}"))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn shake_hands() -> anyhow::Result<()> {
        let torrent = Torrent::from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

        let bt_client = BtClient::new();

        let mut mock_stream = VecDeque::new();
        // Message that will be read by the client
        let response_from_peer = [
            19u8, 66, 105, 116, 84, 111, 114, 114, 101, 110, 116, 32, 112, 114, 111, 116, 111, 99,
            111, 108, 0, 0, 0, 0, 0, 0, 0, 0, 161, 138, 121, 250, 68, 224, 69, 177, 225, 56, 121,
            22, 109, 53, 130, 62, 132, 132, 25, 248, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48,
        ];
        mock_stream.write_all(&response_from_peer)?;

        let res = bt_client.shake_hands(&mut mock_stream, &torrent)?;
        assert_eq!(response_from_peer, res); // What is returned is what was initialy written in
                                             // the "stream"
        let mut buf = [0u8; 68];
        mock_stream.read_exact(&mut buf)?;
        assert_eq!(b"00000000000000000000", &res[48..68]);

        Ok(())
    }

    #[test]
    fn piece_download() -> anyhow::Result<()> {
        const PIECES_SIZE: usize = 100;
        const PIECE_INDEX: usize = 0;
        const BLOCK_SIZE: usize = 19;
        // Lorem ipsum ...
        let file_content = general_purpose::STANDARD.decode("TG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFV0IGVuaW0gYWQgbWluaW0gdmVuaWFtLCBxdWlzIG5vc3RydWQgZXhlcmNpdGF0aW9uIHVsbGFtY28gbGFib3JpcyBuaXNpIHV0IGFsaXF1aXAgZXggZWEgY29tbW9kbyBjb25zZXF1YXQuIER1aXMgYXV0ZSBpcnVyZSBkb2xvciBpbiByZXByZWhlbmRlcml0IGluIHZvbHVwdGF0ZSB2ZWxpdCBlc3NlIGNpbGx1bSBkb2xvcmUgZXUgZnVnaWF0IG51bGxhIHBhcmlhdHVyLiBFeGNlcHRldXIgc2ludCBvY2NhZWNhdCBjdXBpZGF0YXQgbm9uIHByb2lkZW50LCBzdW50IGluIGN1bHBhIHF1aSBvZmZpY2lhIGRlc2VydW50IG1vbGxpdCBhbmltIGlkIGVzdCBsYWJvcnVtLg==")?;
        let hashes = file_content
            .chunks(PIECES_SIZE)
            .map(|i| sha1::hash(i))
            .collect::<Vec<_>>();
        // len: 445
        // last piece size: 45
        // pieces count: 5
        let mut torrent_content = Vec::from(b"d8:announce31:http://127.0.0.1:44381/announce4:infod6:lengthi445e4:name15:faketorrent.iso12:piece lengthi100e6:pieces");
        torrent_content.extend_from_slice(
            &format!("{}:", hashes.len() * 20)
                .bytes()
                .collect::<Vec<_>>(),
        );
        for hash in hashes {
            torrent_content.extend_from_slice(&hash);
        }
        torrent_content.extend_from_slice(b"ee");

        let torrent = Torrent::from_bytes(&torrent_content)?;

        let mut mock_stream = VecDeque::new();

        mock_stream.write_all(&Message::BitField { payload: vec![] }.to_bytes()?)?;
        mock_stream.write_all(&Message::Unchoke.to_bytes()?)?;
        let piece = &file_content[PIECE_INDEX..PIECES_SIZE];
        for block in 0..(PIECES_SIZE / BLOCK_SIZE) {
            let begin = block * BLOCK_SIZE;
            mock_stream.write_all(
                &Message::Piece {
                    index: PIECE_INDEX as u32,
                    begin: begin as u32,
                    block: piece[begin..begin + BLOCK_SIZE].to_vec(),
                }
                .to_bytes()?,
            )?;
        }
        // Write final block
        let last_block_size = PIECES_SIZE % BLOCK_SIZE;
        let last_block_start = PIECES_SIZE - last_block_size;
        mock_stream.write_all(
            &Message::Piece {
                index: PIECE_INDEX as u32,
                begin: last_block_start as u32,
                block: piece[last_block_start..PIECES_SIZE].to_vec(),
            }
            .to_bytes()?,
        )?;

        let client = BtClient::with_block_size(BLOCK_SIZE as u32);
        let res = client.piece_download(&mut mock_stream, torrent, 0)?;

        assert_eq!(Message::Interested, Message::read_from(&mut mock_stream)?);
        for _ in 0..(piece.len() / 19) {
            assert!(matches!(
                Message::read_from(&mut mock_stream)?,
                Message::Request { .. }
            ));
        }
        assert_eq!(file_content[0..100], res);

        Ok(())
    }
}
