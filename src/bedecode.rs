use std::{collections::HashMap, error::Error, fmt::Display};

const NUMBER_HEADER: u8 = b'i';
const NUMBER_TRAILER: u8 = b'e';
const LIST_HEADER: u8 = b'l';
const LIST_TRAILER: u8 = b'e';
const DICT_HEADER: u8 = b'd';
const DICT_TRAILER: u8 = b'e';

#[allow(dead_code)]
pub struct ItemIterator<'a> {
    content: &'a [u8],
    working_data: &'a [u8],
}

pub struct Field<'a, T> {
    raw: &'a [u8],
    pub payload: T,
}

impl<'a, T> Field<'a, T> {
    pub fn new(raw: &'a [u8], payload: T) -> Self {
        Self { raw, payload }
    }
}

pub enum Item<'a> {
    Bytes(Field<'a, &'a [u8]>),
    Number(Field<'a, &'a [u8]>),
    List(Field<'a, Vec<Item<'a>>>),
    Dict(Field<'a, HashMap<String, Item<'a>>>),
}

impl<'a> Item<'a> {
    pub fn raw_length(&self) -> usize {
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
                        .concat()
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
pub struct DecodingError {
    message: String,
}

impl Error for DecodingError {}

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

#[cfg(test)]
mod test {
    use crate::bedecode::Field;

    use super::{Item, ItemIterator};

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
}
