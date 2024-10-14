use reqwest::Url;
use reqwest_mock::Client;

use crate::{
    bedecode::{Field, Item, ItemIterator},
    torrent::Torrent,
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

    pub fn handshake(&self, torrent: Torrent, peer: String) -> anyhow::Result<String> {
        todo!()
    }
}

#[cfg(test)]
mod test {
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
}
