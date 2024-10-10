use std::{
    collections::HashMap,
    env,
    error::Error,
    fmt::Display,
    fs::{self, read},
};

use base64::Engine;
use sha1::{Digest, Sha1};

const NUMBER_HEADER: u8 = b'i';
const NUMBER_TRAILER: u8 = b'e';
const LIST_HEADER: u8 = b'l';
const LIST_TRAILER: u8 = b'e';
const DICT_HEADER: u8 = b'd';
const DICT_TRAILER: u8 = b'e';

#[allow(dead_code)]
struct ItemIterator<'a> {
    content: &'a [u8],
    working_data: &'a [u8],
}

struct Field<'a, T> {
    raw: &'a [u8],
    payload: T,
}

impl<'a, T> Field<'a, T> {
    fn new(raw: &'a [u8], payload: T) -> Self {
        Self { raw, payload }
    }
}

enum Item<'a> {
    Bytes(Field<'a, &'a [u8]>),
    Number(Field<'a, &'a [u8]>),
    List(Field<'a, Vec<Item<'a>>>),
    Dict(Field<'a, HashMap<String, Item<'a>>>),
}

impl<'a> Item<'a> {
    fn raw_length(&self) -> usize {
        match self {
            Item::Bytes(Field { raw, .. }) => raw.len(),
            Item::Number(Field { raw, .. }) => raw.len(),
            Item::List(Field { raw, .. }) => raw.len(),
            Item::Dict(Field { raw, .. }) => raw.len(),
        }
    }
}

impl<'a> Into<String> for Item<'a> {
    fn into(self) -> String {
        format!("{}", self)
    }
}

impl<'a> Display for Item<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Bytes(Field { payload, .. }) => match std::str::from_utf8(payload) {
                Ok(value) => write!(f, "\"{}\"", value),
                Err(_) => write!(
                    f,
                    "{}",
                    payload
                        .iter()
                        .map(|i| format!("{}", i))
                        .collect::<Vec<_>>()
                        .join("")
                ),
            },
            Item::Number(Field { payload, .. }) => write!(
                f,
                "{}",
                std::str::from_utf8(payload).expect("can't make out number out of bytes")
            ),
            Item::List(Field { payload, .. }) => write!(
                f,
                "[{}]",
                payload
                    .iter()
                    .map(|i| format!("{}", i))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Item::Dict(Field { payload, .. }) => {
                let mut map = payload
                    .iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, v))
                    .collect::<Vec<_>>();
                map.sort();
                write!(f, "{{{}}}", map.join(","))
            }
        }
    }
}

impl<'a> ItemIterator<'a> {
    pub fn new(content: &'a [u8]) -> Self {
        Self {
            content,
            working_data: content,
        }
    }

    fn decode_bytes(&mut self) -> Result<Item<'a>, DecodingError> {
        //TODO use an accumulator
        let number_len = self
            .working_data
            .iter()
            .take_while(|i| i.is_ascii_digit())
            .count();
        let len = std::str::from_utf8(&self.working_data[..number_len])
            .expect("can't parse string from bytes")
            .parse::<usize>()
            .expect("can't parse field length");
        let ret = Item::Bytes(Field::new(
            &self.working_data[..number_len + 1 + len],
            &self.working_data[number_len + 1..number_len + 1 + len],
        ));
        self.working_data = &self.working_data[number_len + 1 + len..];
        Ok(ret)
    }

    fn decode_number(&mut self) -> Result<Item<'a>, DecodingError> {
        let payload_len = self.working_data[1..]
            .iter()
            .take_while(|i| i != &&NUMBER_TRAILER)
            .count();
        let ret = Item::Number(Field::new(
            &self.working_data[..payload_len + 2],
            &self.working_data[1..payload_len + 1],
        ));
        self.working_data = &self.working_data[1 + payload_len + 1..];
        Ok(ret)
    }

    fn decode_list(&mut self) -> Result<Item<'a>, DecodingError> {
        let raw = self.working_data;
        let mut end = 2;
        self.working_data = &self.working_data[1..];
        let mut items = Vec::new();
        while self.working_data[0] != LIST_TRAILER {
            let item = self.decode_next()?;
            end += item.raw_length();
            items.push(item);
        }
        self.working_data = &self.working_data[1..];
        Ok(Item::List(Field::new(&raw[..end], items)))
    }

    fn decode_dict(&mut self) -> Result<Item<'a>, DecodingError> {
        let raw = self.working_data;
        let mut end = 2;
        self.working_data = &self.working_data[1..];
        let mut items = HashMap::new();
        while self.working_data[0] != DICT_TRAILER {
            let key = match self.decode_next()? {
                Item::Bytes(Field { raw, payload }) => {
                    end += raw.len();
                    std::str::from_utf8(payload)
                        .expect("can't decode utf8 str from bytes")
                        .to_owned()
                }
                _ => return Err(DecodingError::new("can't decode key for dict")),
            };
            let value = self.decode_next()?;
            end += value.raw_length();
            items.insert(key, value);
        }
        self.working_data = &self.working_data[1..];
        Ok(Item::Dict(Field::new(&raw[..end], items)))
    }

    fn decode_next(&mut self) -> Result<Item<'a>, DecodingError> {
        match self.working_data[0] {
            i if i.is_ascii_digit() => self.decode_bytes(),
            NUMBER_HEADER => self.decode_number(),
            LIST_HEADER => self.decode_list(),
            DICT_HEADER => self.decode_dict(),
            i => Err(DecodingError::new(format!(
                "unknown field header '{}'",
                i as char
            ))),
        }
    }
}

impl<'a> Iterator for ItemIterator<'a> {
    type Item = Result<Item<'a>, DecodingError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.decode_next())
    }
}

#[derive(Debug)]
struct DecodingError {
    message: String,
}

impl DecodingError {
    fn new<T: ToString>(message: T) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl Display for DecodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

struct Info {
    tracker: String,
    len: usize,
    hash: Vec<u8>,
    piece_len: usize,
    pieces_hashes: Vec<Vec<u8>>,
}

impl Info {
    fn parse(content: &[u8]) -> Result<Info, Box<dyn Error>> {
        let mut iter = ItemIterator::new(content);
        match iter.next() {
            Some(Ok(Item::Dict(Field { payload, .. }))) => {
                match (payload.get("announce"), payload.get("info")) {
                    (
                        Some(Item::Bytes(Field {
                            payload: tracker, ..
                        })),
                        Some(Item::Dict(Field { raw, payload: info })),
                    ) => match info.get("length") {
                        Some(Item::Number(Field { payload: bytes, .. })) => {
                            let mut hasher = Sha1::new();
                            hasher.update(raw);
                            Ok(Info {
                                tracker: std::str::from_utf8(tracker).unwrap().to_owned(),
                                len: std::str::from_utf8(bytes)
                                    .to_owned()
                                    .unwrap()
                                    .parse()
                                    .unwrap(),
                                hash: hasher.finalize().into_iter().collect::<Vec<_>>(),
                                piece_len: if let Some(Item::Number(Field { payload, .. })) =
                                    info.get("piece length")
                                {
                                    std::str::from_utf8(payload)
                                        .to_owned()
                                        .unwrap()
                                        .parse()
                                        .unwrap()
                                } else {
                                    0
                                },
                                pieces_hashes: if let Some(Item::Bytes(Field { payload, .. })) =
                                    info.get("pieces")
                                {
                                    payload.chunks(20).map(|i| i.to_vec()).collect()
                                } else {
                                    Vec::new()
                                },
                            })
                        }
                        _ => Err("bar".into()),
                    },
                    _ => Err("foo".into()),
                }
            }
            _ => Err("bah".into()),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let mut encoded_value = ItemIterator::new(&args[2].as_bytes());
        println!("{}", encoded_value.next().unwrap().unwrap());
    } else if command == "info" {
        let info_content = Info::parse(&fs::read(&args[2]).unwrap()).unwrap();
        println!("Tracker URL: {}", info_content.tracker);
        println!("Length: {}", info_content.len);
        println!("Info Hash: {}", hex::encode(info_content.hash));
        println!("Piece Length: {}", info_content.piece_len);
        println!("Piece Hashes:");
        for hash in info_content.pieces_hashes {
            println!("{}", hex::encode(hash));
        }
    } else if command == "peers" {
        base64::engine::general_purpose::STANDARD.encode(fs::read(&args[2]).unwrap());
    } else {
        println!("unknown command: {}", args[1])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use base64::{engine::general_purpose, Engine};

    fn item_from_content<'a>(content: &'a [u8]) -> Item<'a> {
        ItemIterator::new(content).next().unwrap().unwrap()
    }

    #[test]
    fn decode_simple_string() {
        let content = b"5:hello";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Bytes(Field {raw, ..}) if raw == content));
        assert_eq!("\"hello\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_long_string() {
        let content = b"123:Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Bytes(Field {raw, ..}) if raw == content));
        assert_eq!("\"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_string_with_overflow() {
        let content = b"3:foobar";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Bytes(Field {raw, ..}) if raw == &content[0..5]));
        assert_eq!("\"foo\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_positive_int() {
        let content = b"i52e";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Number(Field {raw, ..}) if raw == content));
        assert_eq!("52".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_negative_int() {
        let content = b"i-42e";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Number(Field {raw, ..}) if raw == content));
        assert_eq!("-42".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_positive_int_with_overflow() {
        let content = b"i52ebar";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Number(Field {raw, ..}) if raw == &content[0..4]));
        assert_eq!("52".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_simple_list() {
        let content = b"l5:helloi52ee";
        let item = item_from_content(content);
        assert!(matches!(item, Item::List(Field { raw, .. }) if raw == content));
        assert_eq!("[\"hello\",52]".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_list_with_overflow() {
        let content = b"l3:bazi42eebaz";
        let item = item_from_content(content);
        assert!(matches!(item, Item::List(Field { raw, .. }) if raw == &content[0..11]));
        assert_eq!("[\"baz\",42]".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_dict() {
        let content = b"d3:foo3:bar5:helloi52ee";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Dict(Field { raw, .. }) if raw == content));
        assert_eq!(
            "{\"foo\":\"bar\",\"hello\":52}".to_owned(),
            format!("{}", item)
        );
    }

    #[test]
    fn decode_dict_with_overflow() {
        let content = b"d3:fooi42eebaz";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Dict(Field { raw, .. }) if raw == &content[0..11]));
        assert_eq!("{\"foo\":42}".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_dict_in_dict() {
        let content = b"d3:foo3:bar4:infod3:bari42eee";
        let item = item_from_content(content);
        assert!(matches!(item, Item::Dict(Field { raw, .. }) if raw == content));
        assert_eq!(
            "{\"foo\":\"bar\",\"info\":{\"bar\":42}}".to_owned(),
            format!("{}", item)
        );
    }

    #[test]
    fn info_file() {
        let info =
            Info::parse(b"d8:announce34:http://disney.com/steamboat_willie4:infod6:lengthi123eee")
                .unwrap();
        assert_eq!("http://disney.com/steamboat_willie", info.tracker);
        assert_eq!(123, info.len);
    }

    #[test]
    fn info_bad_file() {
        let info = Info::parse(b"foo");
        assert!(info.is_err());
    }

    #[test]
    fn info_with_hash_and_pieces() {
        let info = Info::parse(&general_purpose::STANDARD.decode("ZDg6YW5ub3VuY2U1NTpodHRwOi8vYml0dG9ycmVudC10ZXN0LXRyYWNrZXIuY29kZWNyYWZ0ZXJzLmlvL2Fubm91bmNlMTA6Y3JlYXRlZCBieTEzOm1rdG9ycmVudCAxLjE0OmluZm9kNjpsZW5ndGhpODIwODkyZTQ6bmFtZTE5OmNvbmdyYXR1bGF0aW9ucy5naWYxMjpwaWVjZSBsZW5ndGhpMjYyMTQ0ZTY6cGllY2VzODA6PUKiDtsc+EDNNSjTqekh22M4pGNp+IWzmIpS/7A1kZhUArbVKFlAq3aGnmycHxAflPOd4VPkaL5qY49Pve1o0C3gEaK2h/dbWDP0bM6OPpxlZQ==").unwrap()).unwrap();
        assert_eq!(
            "http://bittorrent-test-tracker.codecrafters.io/announce",
            info.tracker
        );
        assert_eq!(820892, info.len);
        assert_eq!(
            "1cad4a486798d952614c394eb15e75bec587fd08",
            hex::encode(info.hash)
        );
        assert_eq!(262144, info.piece_len);
        assert_eq!(
            vec![
                "3d42a20edb1cf840cd3528d3a9e921db6338a463",
                "69f885b3988a52ffb03591985402b6d5285940ab",
                "76869e6c9c1f101f94f39de153e468be6a638f4f",
                "bded68d02de011a2b687f75b5833f46cce8e3e9c"
            ],
            info.pieces_hashes
                .iter()
                .map(|i| hex::encode(i))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn info_with_peers() {
        assert!(false);
    }
}
