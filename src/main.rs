use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    match encoded_value.chars().next() {
        Some(c) if c.is_digit(10) => {
            serde_json::Value::String(serde_bencode::from_str(encoded_value).unwrap())
        }
        Some('i') => serde_json::Value::String(
            serde_bencode::from_str::<i64>(encoded_value)
                .unwrap()
                .to_string(),
        ),
        _ => panic!("Unhandled encoded value: {}", encoded_value),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decode_string() {
        let val = decode_bencoded_value("5:hello");
        assert_eq!("hello", val);
    }

    #[test]
    fn decode_positive_int() {
        let val = decode_bencoded_value("i52e");
        assert_eq!("52", val);
    }

    #[test]
    fn decode_negative_int() {
        let val = decode_bencoded_value("i-42e");
        assert_eq!("-42", val);
    }
}
