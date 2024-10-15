use std::{
    fmt::Debug,
    io::{Read, Write},
    net::{IpAddr, TcpStream},
};

use anyhow::Context;
use bytes::BufMut;
use reqwest::Url;
use reqwest_mock::Client;

use crate::{
    torrent::Torrent,
    tracker::{self, Peer},
};

const PEER_ID: &str = "alice_is_1_feet_tall";

pub struct BtClient<T: Client> {
    client: T,
}

impl<T: Client> BtClient<T> {
    pub fn new(client: T) -> Self {
        Self { client }
    }

    pub fn get_peers(&self, torrent: Torrent) -> anyhow::Result<Vec<String>> {
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

        let res = self.client.get(tracker_url).send().unwrap();
        //TODO: .context("get request to tracker")?;

        let mut res: tracker::Response =
            serde_bencode::from_bytes(&res.body).context("parse tracker get response")?;

        res = dbg!(res);

        Ok(res
            .peers
            .0
            .iter()
            .map(|i| format!("{}:{}", i.addr, i.port))
            .collect::<Vec<_>>())
    }

    pub fn handshake(&self, torrent: Torrent, peer: String) -> anyhow::Result<String> {
        let peer = Peer::try_from(peer).context("parsing peer address and port")?;
        let mut tcp_stream = TcpStream::connect(Into::<(IpAddr, u16)>::into(peer))
            .context("opening socket to peer")?;

        let res = self.shake_hands(&mut tcp_stream, torrent)?;

        Ok(hex::encode(Handshake::from(res).to_bytes()))
    }

    fn shake_hands<S: Read + Write + Debug>(
        &self,
        stream: &mut S,
        torrent: Torrent,
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
}

#[derive(Debug)]
struct Handshake {
    info_hash: [u8; 20],
    peer_id: [u8; 20],
}

impl Handshake {
    fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self { info_hash, peer_id }
    }

    fn to_bytes(&self) -> [u8; 68] {
        let mut buf = Vec::new();
        buf.push(19u8);
        buf.put_slice(b"BitTorrent protocol");
        buf.put_bytes(0u8, 8);
        buf.put(&self.info_hash[..]);
        buf.put(&self.peer_id[..]);
        buf.try_into().expect("should always work")
    }
}

impl From<[u8; 68]> for Handshake {
    fn from(value: [u8; 68]) -> Self {
        Self::new(
            value[28..48].try_into().expect("should never fail"),
            value[48..68].try_into().expect("should never fail"),
        )
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
    };

    use reqwest::{Method, Url};
    use reqwest_mock::{StubClient, StubDefault, StubSettings, StubStrictness};

    use crate::{bt_client::BtClient, torrent::Torrent};

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

    #[test]
    fn shake_hands() -> anyhow::Result<()> {
        let torrent = Torrent::from_base64("ZDg6YW5ub3VuY2UzMTpodHRwOi8vMTI3LjAuMC4xOjQ0MzgxL2Fubm91bmNlNDppbmZvZDY6bGVuZ3RoaTIwOTcxNTJlNDpuYW1lMTU6ZmFrZXRvcnJlbnQuaXNvMTI6cGllY2UgbGVuZ3RoaTI2MjE0NGU2OnBpZWNlczE2MDrd8zFyWZ/ahPCiCaMDT3nwuKpeInlaYYoe5SdelShDsBpWrk4UJ1Lvza4u9TLWEaRrLPe2TVeMCbOsC24Jja3AwZQ28ZJ+onuQ6xixooIKI4+lNVQZiG2exW6GzXeRND6Ted4YHK6s6xX9ETSxtLIfrQQSWyJ7Tc/6WG4g1Xmk3nYJDhK9Cj2bHFOfPq7C1+sdtTnCqdJNAj+5FreSNLdpZWU=")?;

        let client = StubClient::new(StubSettings {
            default: StubDefault::Error,
            strictness: StubStrictness::MethodUrl,
        });

        let bt_client = BtClient::new(client);

        let mut mock_stream = VecDeque::new();
        // Message that will be read by the client
        let response_from_peer = [
            19u8, 66, 105, 116, 84, 111, 114, 114, 101, 110, 116, 32, 112, 114, 111, 116, 111, 99,
            111, 108, 0, 0, 0, 0, 0, 0, 0, 0, 161, 138, 121, 250, 68, 224, 69, 177, 225, 56, 121,
            22, 109, 53, 130, 62, 132, 132, 25, 248, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48,
        ];
        mock_stream.write_all(&response_from_peer)?;

        let res = bt_client.shake_hands(&mut mock_stream, torrent)?;
        assert_eq!(response_from_peer, res); // What is returned is what was initialy written in
                                             // the "stream"
        let mut buf = [0u8; 68];
        mock_stream.read_exact(&mut buf)?;
        assert_eq!(b"00000000000000000000", &res[48..68]);

        Ok(())
    }
}
