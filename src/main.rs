use serde_json;
use std::env;

#[derive(Debug)]
struct Value(serde_json::Value);

impl ToString for Value {
    fn to_string(&self) -> String {
        match &self.0 {
            serde_json::Value::String(_) => self.0.to_string(),
            serde_json::Value::Number(n) => n.as_i64().expect("cannot unwrap i64").to_string(),
            _ => todo!(),
        }
    }
}

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> Value {
    match encoded_value.chars().next() {
        Some(c) if c.is_digit(10) => Value(serde_json::Value::String(
            serde_bencode::from_str(encoded_value).expect("cannot decode string"),
        )),
        Some('i') => Value(
            serde_bencode::from_str::<i64>(encoded_value)
                .expect("cannot decode number")
                .into(),
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
        assert_eq!(r#""hello""#, val.to_string());
    }

    #[test]
    fn decode_positive_int() {
        let val = decode_bencoded_value("i52e");
        assert_eq!("52", val.to_string());
    }

    #[test]
    fn decode_negative_int() {
        let val = decode_bencoded_value("i-42e");
        assert_eq!("-42", val.to_string());
    }
}
