use std::{collections::HashMap, env, error::Error, fmt::Display, fs};

use base64::Engine;

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

enum Item<'a> {
    Bytes(&'a [u8]),
    Number(&'a [u8]),
    List(Vec<Item<'a>>),
    Dict(HashMap<String, Item<'a>>),
}

impl<'a> Into<String> for Item<'a> {
    fn into(self) -> String {
        format!("{}", self)
    }
}

impl<'a> Display for Item<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(value) => write!(f, "\"{}\"", value),
                Err(_) => write!(
                    f,
                    "{}",
                    bytes
                        .iter()
                        .map(|i| format!("{}", i))
                        .collect::<Vec<_>>()
                        .join("")
                ),
            },
            Item::Number(number) => write!(
                f,
                "{}",
                std::str::from_utf8(number).expect("can't make out number out of bytes")
            ),
            Item::List(items) => write!(
                f,
                "[{}]",
                items
                    .iter()
                    .map(|i| format!("{}", i))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Item::Dict(hashmap) => {
                let mut hashmap = hashmap
                    .iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, v))
                    .collect::<Vec<_>>();
                hashmap.sort();
                write!(f, "{{{}}}", hashmap.join(","))
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
        let ret = Item::Bytes(&self.working_data[number_len + 1..number_len + 1 + len]);
        self.working_data = &self.working_data[number_len + 1 + len..];
        Ok(ret)
    }

    fn decode_number(&mut self) -> Result<Item<'a>, DecodingError> {
        let payload_len = self.working_data[1..]
            .iter()
            .take_while(|i| i != &&NUMBER_TRAILER)
            .count();
        let ret = Item::Number(&self.working_data[1..payload_len + 1]);
        self.working_data = &self.working_data[1 + payload_len + 1..];
        Ok(ret)
    }

    fn decode_list(&mut self) -> Result<Item<'a>, DecodingError> {
        self.working_data = &self.working_data[1..];
        let mut items = Vec::new();
        while self.working_data[0] != LIST_TRAILER {
            items.push(self.decode_next()?);
        }
        self.working_data = &self.working_data[1..];
        Ok(Item::List(items))
    }

    fn decode_dict(&mut self) -> Result<Item<'a>, DecodingError> {
        self.working_data = &self.working_data[1..];
        let mut items = HashMap::new();
        while self.working_data[0] != DICT_TRAILER {
            let key = match self.decode_next()? {
                Item::Bytes(bytes) => std::str::from_utf8(bytes)
                    .expect("can't decode utf8 str from bytes")
                    .to_owned(),
                _ => return Err(DecodingError::new("can't decode key for dict")),
            };
            let value = self.decode_next()?;
            items.insert(key, value);
        }
        Ok(Item::Dict(items))
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
}

impl Info {
    fn parse(content: &[u8]) -> Result<Info, Box<dyn Error>> {
        let mut iter = ItemIterator::new(content);
        match iter.next() {
            Some(Ok(Item::Dict(map))) => match (map.get("announce"), map.get("info")) {
                (Some(Item::Bytes(tracker)), Some(Item::Dict(info))) => match info.get("length") {
                    Some(Item::Number(bytes)) => Ok(Info {
                        tracker: std::str::from_utf8(tracker).unwrap().to_owned(),
                        len: std::str::from_utf8(bytes)
                            .to_owned()
                            .unwrap()
                            .parse()
                            .unwrap(),
                    }),
                    _ => Err("bar".into()),
                },
                _ => Err("foo".into()),
            },
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
        let content = fs::read(&args[2]).unwrap();
        println!(
            "#{}#",
            base64::engine::general_purpose::STANDARD.encode(&content)
        );

        // let info_content = Info::parse(&fs::read(&args[2]).unwrap()).unwrap();
        // println!("Tracker URL: {}", info_content.tracker);
        // println!("Length: {}", info_content.len);
    } else {
        println!("unknown command: {}", args[1])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn item_from_content<'a>(content: &'a [u8]) -> Item<'a> {
        ItemIterator::new(content).next().unwrap().unwrap()
    }

    #[test]
    fn decode_simple_string() {
        let item = item_from_content(b"5:hello");
        assert_eq!("\"hello\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_long_string() {
        let item = item_from_content(b"123:Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
        assert_eq!("\"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_string_with_overflow() {
        let item = item_from_content(b"3:foobar");
        assert_eq!("\"foo\"".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_positive_int() {
        let item = item_from_content(b"i52e");
        assert_eq!("52".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_negative_int() {
        let item = item_from_content(b"i-42e");
        assert_eq!("-42".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_positive_int_with_overflow() {
        let item = item_from_content(b"i52ebar");
        assert_eq!("52".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_simple_list() {
        let item = item_from_content(b"l5:helloi52ee");
        assert_eq!("[\"hello\",52]".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_list_with_overflow() {
        let item = item_from_content(b"l3:bazi42eebaz");
        assert_eq!("[\"baz\",42]".to_owned(), format!("{}", item));
    }

    #[test]
    fn decode_dict() {
        let item = item_from_content(b"d3:foo3:bar5:helloi52ee");
        assert_eq!(
            "{\"foo\":\"bar\",\"hello\":52}".to_owned(),
            format!("{}", item)
        )
    }

    #[test]
    fn decode_dict_with_overflow() {
        let item = item_from_content(b"d3:fooi42eebaz");
        assert_eq!("{\"foo\":42}".to_owned(), format!("{}", item))
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
}
