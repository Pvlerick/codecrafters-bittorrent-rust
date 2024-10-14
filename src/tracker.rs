use core::fmt;
use std::net::Ipv4Addr;

use serde::{de::Visitor, Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct Response {
    pub interval: usize,
    pub peers: Peers,
}

#[derive(Debug)]
pub struct Peers(pub Vec<Peer>);

#[derive(Debug)]
pub struct Peer {
    pub address: Ipv4Addr,
    pub port: u16,
}

impl Peer {
    fn new(address: [u8; 4], port: [u8; 2]) -> Self {
        Self {
            address: Ipv4Addr::new(address[0], address[1], address[2], address[3]),
            port: u16::from_be_bytes(port),
        }
    }
}

struct PeersVisitor;

impl<'de> Visitor<'de> for PeersVisitor {
    type Value = Peers;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between -2^31 and 2^31")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.len() % 6 != 0 {
            return Err(E::custom(format!(
                "length {} is not a multiple of 6",
                v.len()
            )));
        }

        Ok(Peers(
            v.chunks_exact(6)
                .map(|i| {
                    Peer::new(
                        i[0..4].try_into().expect("should never happen"),
                        i[4..6].try_into().expect("should never happen"),
                    )
                })
                .collect::<Vec<_>>(),
        ))
    }
}

impl<'de> Deserialize<'de> for Peers {
    fn deserialize<D>(deserializer: D) -> Result<Peers, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PeersVisitor)
    }
}
