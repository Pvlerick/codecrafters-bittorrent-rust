use std::{
    fmt::{Debug, Display},
    io::Read,
};

use anyhow::anyhow;
use bytes::BufMut;

#[derive(Debug)]
pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self { info_hash, peer_id }
    }

    pub fn to_bytes(&self) -> [u8; 68] {
        let mut buf = Vec::new();
        buf.push(19u8);
        buf.extend_from_slice(b"BitTorrent protocol");
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
        }
    }

    fn usize_to_u32_be_bytes(input: usize) -> anyhow::Result<[u8; 4]> {
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
            x => Err(anyhow!(
                "unrecognized message id: {x} or invalid message length"
            )),
        }
    }

    pub fn read_from<T: Read>(input: &mut T) -> anyhow::Result<Message> {
        let mut mark = [0u8; 5];
        input.read_exact(&mut mark)?;
        let len = u32::from_be_bytes(mark[0..4].try_into().expect("cannot fail")) as usize;
        match mark[4] {
            0..=2 => Message::from_bytes(&mark),
            5..=7 => {
                let mut message = vec![0u8; 4 + len];
                message[..5].copy_from_slice(&mark);
                input.read_exact(&mut message[5..len + 4])?;
                Message::from_bytes(&message)
            }
            x => Err(anyhow!("unrecognized message id: {x}")),
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
        }
    }
}

#[cfg(test)]
mod test {
    use crate::peer_messages::Message;

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
}
