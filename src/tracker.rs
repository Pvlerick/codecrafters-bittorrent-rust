use anyhow::{anyhow, Result};
use core::fmt;
use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

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
    pub addr: IpAddr,
    pub port: u16,
}

impl Peer {
    fn new(address: [u8; 4], port: [u8; 2]) -> Self {
        Self {
            addr: Ipv4Addr::new(address[0], address[1], address[2], address[3]).into(),
            port: u16::from_be_bytes(port),
        }
    }

    fn from_addr_and_port(addr: IpAddr, port: u16) -> Self {
        Self { addr, port }
    }
}

impl Into<(IpAddr, u16)> for Peer {
    fn into(self) -> (IpAddr, u16) {
        (self.addr, self.port)
    }
}

impl TryFrom<String> for Peer {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.split_once(":") {
            Some((ip, port)) => Ok(Peer::from_addr_and_port(
                Ipv4Addr::from_str(ip)?.into(),
                port.parse()?,
            )),
            None => Err(anyhow!("invalid format, got '{}'", value)),
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
