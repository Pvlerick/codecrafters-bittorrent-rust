use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    match encoded_value.chars().next() {
        Some(c) if c.is_digit(10) => serde_json::Value::String(
            serde_bencode::from_str(encoded_value).expect("cannot decode string"),
        ),
        Some('i') => serde_bencode::from_str::<i64>(encoded_value)
            .expect("cannot decode number")
            .into(),
        _ => panic!("Unhandled encoded value: {}", encoded_value),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = match decode_bencoded_value(encoded_value) {
            serde_json::Value::String(_) => encoded_value.to_string(),
            serde_json::Value::Number(val) => {
                val.as_i64().expect("cannot unwrap as i64").to_string()
            }
            _ => panic!("unsupported value"),
        };
        println!("{}", decoded_value);
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
        assert_eq!("52", val.as_i64().unwrap().to_string());
    }

    #[test]
    fn decode_negative_int() {
        let val = decode_bencoded_value("i-42e");
        assert_eq!("-42", val.as_i64().unwrap().to_string());
    }
}
