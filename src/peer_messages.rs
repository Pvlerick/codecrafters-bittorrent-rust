use std::{
    fmt::{Debug, Display},
    io::Read,
};

use anyhow::{anyhow, Context};
use bytes::BufMut;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    extension: Extension,
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Handshake::with_extension(info_hash, peer_id, Extension::None)
    }

    pub fn with_extension(info_hash: [u8; 20], peer_id: [u8; 20], extension: Extension) -> Self {
        Self {
            info_hash,
            peer_id,
            extension,
        }
    }

    pub fn to_bytes(&self) -> [u8; 68] {
        let mut buf = Vec::new();
        buf.push(19u8);
        buf.extend_from_slice(b"BitTorrent protocol");
        buf.put(self.extension.to_bytes().as_slice());
        buf.put(&self.info_hash[..]);
        buf.put(&self.peer_id[..]);
        buf.try_into().expect("should always work")
    }
}

impl From<&[u8; 68]> for Handshake {
    fn from(value: &[u8; 68]) -> Self {
        Self::with_extension(
            value[28..48].try_into().expect("should never fail"),
            value[48..68].try_into().expect("should never fail"),
            Extension::from(&value[20..28].try_into().expect("should never fail")),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Extension {
    None,
    MagnetLink,
}

impl Extension {
    pub fn to_bytes(&self) -> [u8; 8] {
        match self {
            Extension::None => [0u8; 8],
            Extension::MagnetLink => [0, 0, 0, 0, 0, 16, 0, 0],
        }
    }
}

//TODO Should be try_from
impl From<&[u8; 8]> for Extension {
    fn from(value: &[u8; 8]) -> Self {
        match value {
            [0, 0, 0, 0, 0, 16, 0, 0] => Extension::MagnetLink,
            _ => Extension::None,
        }
    }
}

#[cfg(test)]
mod handshake_test {
    use bytes::BufMut;

    use crate::peer_messages::{Extension, Handshake};

    const INFO_HASH: [u8; 20] = [
        0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
    ];
    const PEER_ID: [u8; 20] = [
        40u8, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
    ];

    #[test]
    fn ser_deser_handshake_without_extension() {
        let handshake = Handshake::new(INFO_HASH, PEER_ID);

        let mut bytes = Vec::new();
        bytes.push(19u8);
        bytes.extend_from_slice(b"BitTorrent protocol");
        bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        bytes.put(&INFO_HASH[..]);
        bytes.put(&PEER_ID[..]);
        let bytes: [u8; 68] = bytes.try_into().expect("should not fail");

        assert_eq!(bytes, handshake.to_bytes());
        assert_eq!(handshake, Handshake::from(&bytes));
    }

    #[test]
    fn ser_deser_handshake_with_magnet_link_extension() {
        let handshake = Handshake::with_extension(INFO_HASH, PEER_ID, Extension::MagnetLink);

        let mut bytes = Vec::new();
        bytes.push(19u8);
        bytes.extend_from_slice(b"BitTorrent protocol");
        bytes.extend_from_slice(&[0, 0, 0, 0, 0, 16, 0, 0]);
        bytes.put(&INFO_HASH[..]);
        bytes.put(&PEER_ID[..]);
        let bytes: [u8; 68] = bytes.try_into().expect("should not fail");

        assert_eq!(bytes, handshake.to_bytes());
        assert_eq!(handshake, Handshake::from(&bytes));
    }
}

#[derive(Debug, PartialEq)]
pub enum Message {
    BitField {
        payload: Vec<u8>,
    },
    Interested,
    Choke,
    Unchoke,
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    Extension {
        extensions_info: ExtensionsInfo,
    },
}

impl Message {
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        match self {
            // choke: <len=0001><id=0>
            Message::Choke => Ok(vec![0, 0, 0, 1, 0]),
            // unchoke: <len=0001><id=1>
            Message::Unchoke => Ok(vec![0, 0, 0, 1, 1]),
            // interested: <len=0001><id=2>
            Message::Interested => Ok(vec![0, 0, 0, 1, 2]),
            // bitfield: <len=0001+X><id=5><bitfield>
            Message::BitField { payload } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(&Message::usize_to_u32_be_bytes(payload.len() + 1)?);
                buf.push(5);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
            // request: <len=0013><id=6><index><begin><length>
            Message::Request {
                index,
                begin,
                length,
            } => {
                let mut buf = vec![0u8, 0, 0, 13, 6];
                buf.extend_from_slice(&u32::to_be_bytes(*index));
                buf.extend_from_slice(&u32::to_be_bytes(*begin));
                buf.extend_from_slice(&u32::to_be_bytes(*length));
                Ok(buf)
            }
            // piece: <len=0009+X><id=7><index><begin><block>
            Message::Piece {
                index,
                begin,
                block,
            } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(&Message::usize_to_u32_be_bytes(block.len() + 9)?);
                buf.push(7);
                buf.extend_from_slice(&u32::to_be_bytes(*index));
                buf.extend_from_slice(&u32::to_be_bytes(*begin));
                buf.extend_from_slice(block);
                Ok(buf)
            }
            // extension: <len=0001+X><id=20><extensions_stuff>
            Message::Extension { extensions_info } => {
                let payload = serde_bencode::to_bytes(extensions_info)?;
                let mut buf = Vec::new();
                buf.extend_from_slice(&Message::usize_to_u32_be_bytes(payload.len() + 2)?);
                buf.push(20); // message id
                buf.push(0); // extension handshake id
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
        }
    }

    pub fn usize_to_u32_be_bytes(input: usize) -> anyhow::Result<[u8; 4]> {
        Ok(u32::to_be_bytes(input.try_into()?))
    }

    pub fn from_bytes(input: &[u8]) -> anyhow::Result<Message> {
        if input.len() < 5 {
            return Err(anyhow!("minimum message len is 5"));
        }

        match input[4] {
            0 => Ok(Message::Choke),
            1 => Ok(Message::Unchoke),
            2 => Ok(Message::Interested),
            5 => Ok(Message::BitField {
                payload: input[5..].to_vec(),
            }),
            6 if input.len() == 17 => Ok(Message::Request {
                index: u32::from_be_bytes(input[5..9].try_into().expect("cannot fail")),
                begin: u32::from_be_bytes(input[9..13].try_into().expect("cannot fail")),
                length: u32::from_be_bytes(input[13..17].try_into().expect("cannot fail")),
            }),
            7 if input.len() >= 13 => Ok(Message::Piece {
                index: u32::from_be_bytes(input[5..9].try_into().expect("cannot fail")),
                begin: u32::from_be_bytes(input[9..13].try_into().expect("cannot fail")),
                block: input[13..].to_vec(),
            }),
            20 => Ok(Message::Extension {
                extensions_info: serde_bencode::from_bytes(&input[6..])?,
            }),
            id => Err(anyhow!(
                "unrecognized message id: {id} or invalid message length"
            )),
        }
    }

    pub fn read_from<T: Read>(input: &mut T) -> anyhow::Result<Message> {
        let mut mark = [0u8; 5];
        input.read_exact(&mut mark).context("reading from input")?;
        let len: usize = u32::from_be_bytes(mark[0..4].try_into().context("cannot fail")?)
            .try_into()
            .context("converting u32 to usize")?;
        match mark[4] {
            0..=2 => Message::from_bytes(&mark),
            5..=7 | 20 => {
                dbg!(len);
                let mut message = vec![0u8; 4 + len];
                message[..5].copy_from_slice(&mark);
                dbg!(&message);
                input
                    .read_exact(&mut message[5..len + 4])
                    .context("reading exact number of bytes from the reader")?;
                dbg!(&message);
                Message::from_bytes(&message)
            }
            id => Err(anyhow!("unrecognized message id: {id}")),
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::BitField { .. } => write!(f, "BitField"),
            Message::Interested => write!(f, "Interested"),
            Message::Choke => write!(f, "Choke"),
            Message::Unchoke => write!(f, "Unchoke"),
            Message::Request { .. } => write!(f, "Request"),
            Message::Piece { .. } => write!(f, "Piece"),
            Message::Extension { .. } => write!(f, "Extensions"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExtensionsInfo {
    #[serde(rename = "m")]
    pub metdata: Metadata,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Metadata {
    pub ut_metadata: Option<u8>,
    pub ut_pex: Option<u8>,
}

impl ExtensionsInfo {
    pub fn new(ut_metadata: u8) -> Self {
        ExtensionsInfo {
            metdata: Metadata {
                ut_metadata: Some(ut_metadata),
                ut_pex: None,
            },
        }
    }
}

#[cfg(test)]
mod message_test {
    use bytes::BufMut;

    use crate::peer_messages::{ExtensionsInfo, Message};

    #[test]
    fn ser_deser_message_bitfield() -> anyhow::Result<()> {
        let msg = Message::BitField {
            payload: b"foo".to_vec(),
        };
        let bytes = vec![0, 0, 0, 4, 5, 102, 111, 111];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_bitfield_empty() -> anyhow::Result<()> {
        let msg = Message::BitField {
            payload: Vec::new(),
        };
        let bytes = vec![0, 0, 0, 1, 5];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_interested() -> anyhow::Result<()> {
        let msg = Message::Interested;
        let bytes = vec![0, 0, 0, 1, 2];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_choke() -> anyhow::Result<()> {
        let msg = Message::Choke;
        let bytes = vec![0, 0, 0, 1, 0];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_unchoke() -> anyhow::Result<()> {
        let msg = Message::Unchoke;
        let bytes = vec![0, 0, 0, 1, 1];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_request() -> anyhow::Result<()> {
        let msg = Message::Request {
            index: 1,
            begin: 3,
            length: 42,
        };
        let bytes = vec![0, 0, 0, 13, 6, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 42];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_piece() -> anyhow::Result<()> {
        let msg = Message::Piece {
            index: 4,
            begin: 12,
            block: vec![102, 111, 111],
        };
        let bytes = vec![0, 0, 0, 12, 7, 0, 0, 0, 4, 0, 0, 0, 12, 102, 111, 111];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_empty_piece() -> anyhow::Result<()> {
        let msg = Message::Piece {
            index: 4,
            begin: 12,
            block: vec![],
        };
        let bytes = vec![0, 0, 0, 9, 7, 0, 0, 0, 4, 0, 0, 0, 12];

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }

    #[test]
    fn ser_deser_message_extension() -> anyhow::Result<()> {
        let extensions_info = ExtensionsInfo::new(16);
        let msg = Message::Extension { extensions_info };

        let payload = b"d1:md11:ut_metadatai16eee";
        let mut bytes = vec![0, 0, 0];
        bytes.push(payload.len() as u8 + 2);
        bytes.push(20);
        bytes.push(0);
        bytes.put_slice(payload);

        assert_eq!(bytes, msg.to_bytes()?);
        assert_eq!(msg, Message::from_bytes(&bytes)?);

        Ok(())
    }
}
