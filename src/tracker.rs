use anyhow::Result;
use core::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use serde::{de::Visitor, Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct Response {
    pub interval: usize,
    pub peers: Peers,
}

#[derive(Debug)]
pub struct Peers(pub Vec<SocketAddrV4>);

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
                    SocketAddrV4::new(
                        Ipv4Addr::new(i[0], i[1], i[2], i[3]),
                        u16::from_be_bytes(i[4..6].try_into().expect("should not happen")),
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
