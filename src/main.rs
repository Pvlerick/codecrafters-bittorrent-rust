use serde_json::{Map, Value};
use std::env;

#[allow(dead_code)]
fn display(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s.to_string()),
        Value::Number(n) => n.as_i64().expect("cannot unwrap i64").to_string(),
        Value::Array(values) => {
            format!(
                "[{}]",
                values
                    .iter()
                    .map(|i| display(i))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Object(values) => {
            let mut values = values
                .iter()
                .map(|(k, v)| format!("{}: {}", k, display(v)))
                .collect::<Vec<_>>();
            values.sort_by_key(|i| i.to_owned());
            format!("{{{}}}", values.join(","))
        }
        _ => todo!(),
    }
}

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> Value {
    match decode(encoded_value) {
        Some((value, _)) => value,
        None => panic!("can't decode value"),
    }
}

const STRING_SEPARATOR: char = ':';
const STRING_SEPARATOR_LEN: usize = STRING_SEPARATOR.len_utf8();
const NUMBER_HEADER: char = 'i';
const NUMBER_HEADER_LEN: usize = NUMBER_HEADER.len_utf8();
const NUMBER_TRAILER: char = 'e';
const NUMBER_TRAILER_LEN: usize = NUMBER_TRAILER.len_utf8();
const LIST_HEADER: char = 'l';
const LIST_HEADER_LEN: usize = LIST_HEADER.len_utf8();
const LIST_TRAILER: char = 'e';
const LIST_TRAILER_LEN: usize = LIST_TRAILER.len_utf8();
const DICT_HEADER: char = 'd';
const DICT_HEADER_LEN: usize = DICT_HEADER.len_utf8();
const DICT_TRAILER: char = 'e';
const DICT_TRAILER_LEN: usize = DICT_TRAILER.len_utf8();

fn decode(encoded: &str) -> Option<(Value, &str)> {
    let mut chars = encoded.chars().peekable();
    match chars.peek() {
        Some(c) if c.is_digit(10) => decode_string(encoded),
        Some(&NUMBER_HEADER) => decode_int(encoded),
        Some(&LIST_HEADER) => decode_list(encoded),
        Some(&DICT_HEADER) => decode_dict(encoded),
        Some(_) | None => todo!(),
    }
}

fn decode_string(start: &str) -> Option<(Value, &str)> {
    let header_len = start
        .chars()
        .take_while(|c| c.is_digit(10))
        .map(|i| i.len_utf8())
        .sum();
    let len = start[0..header_len]
        .parse::<usize>()
        .expect("can't parse string length");
    Some((
        Value::String(
            start[header_len + STRING_SEPARATOR_LEN..header_len + STRING_SEPARATOR_LEN + len]
                .to_owned(),
        ),
        &start[header_len + 1 + len..],
    ))
}

fn decode_int(start: &str) -> Option<(Value, &str)> {
    let payload_len: usize = start
        .chars()
        .skip(1)
        .take_while(|i| i != &NUMBER_TRAILER)
        .map(|i| i.len_utf8())
        .sum();
    let val = start[NUMBER_HEADER_LEN..NUMBER_HEADER_LEN + payload_len]
        .parse::<i64>()
        .expect("can't parse int");
    Some((
        Value::Number(val.into()),
        &start[NUMBER_HEADER_LEN + payload_len + NUMBER_TRAILER_LEN..],
    ))
}

fn decode_list(start: &str) -> Option<(Value, &str)> {
    let mut start = &start[LIST_HEADER_LEN..];
    let mut values = Vec::new();
    while let Some(c) = start.chars().next() {
        if c == LIST_TRAILER {
            break;
        }
        if let Some((value, rest)) = decode(start) {
            values.push(value);
            start = rest;
        }
    }
    Some((values.into(), &start[LIST_TRAILER_LEN..]))
}

fn decode_dict(start: &str) -> Option<(Value, &str)> {
    let mut start = &start[DICT_HEADER_LEN..];
    let mut values = Map::new();
    while let Some(c) = start.chars().next() {
        if c == DICT_TRAILER {
            break;
        }
        if let Some((key, rest)) = decode_string(start) {
            if let Some((value, rest)) = decode(rest) {
                values.insert(key.to_string(), value);
                start = rest;
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    Some((Value::Object(values), &start[DICT_TRAILER_LEN..]))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", display(&decoded_value));
    } else {
        println!("unknown command: {}", args[1])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decode_string() {
        let (val, rest) = decode("5:hello").unwrap();
        assert_eq!("hello", val);
        assert_eq!("", rest);
    }

    #[test]
    fn decode_long_string() {
        let (val, rest) = decode("123:Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.").unwrap();
        assert_eq!("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.", val);
        assert_eq!("", rest);
    }

    #[test]
    fn decode_string_with_overflow() {
        let (val, rest) = decode("3:foobar").unwrap();
        assert_eq!("foo", val);
        assert_eq!("bar", rest);
    }

    #[test]
    fn decode_positive_int() {
        let (val, rest) = decode("i52e").unwrap();
        assert_eq!("52", val.to_string());
        assert_eq!("", rest);
    }

    #[test]
    fn decode_negative_int() {
        let (val, rest) = decode("i-42e").unwrap();
        assert_eq!("-42", val.to_string());
        assert_eq!("", rest);
    }

    #[test]
    fn decode_positive_int_with_overflow() {
        let (val, rest) = decode("i52ebar").unwrap();
        assert_eq!("52", val.to_string());
        assert_eq!("bar", rest);
    }

    #[test]
    fn decode_list() {
        let (val, rest) = decode("l5:helloi52ee").unwrap();
        assert_eq!("[\"hello\",52]", display(&val));
        assert_eq!("", rest);
    }

    #[test]
    fn decode_list_with_overflow() {
        let (val, rest) = decode("l5:helloi52eebaz").unwrap();
        assert_eq!("[\"hello\",52]", display(&val));
        assert_eq!("baz", rest);
    }

    #[test]
    fn decode_dict() {
        let (val, rest) = decode("d3:foo3:bar5:helloi52ee").unwrap();
        assert_eq!("{\"foo\": \"bar\",\"hello\": 52}", display(&val));
        assert_eq!("", rest);
    }

    #[test]
    fn decode_dict_with_overflow() {
        let (val, rest) = decode("d3:fooi42eebaz").unwrap();
        assert_eq!("{\"foo\": 42}", display(&val));
        assert_eq!("baz", rest);
    }
}
