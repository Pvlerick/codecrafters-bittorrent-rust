use std::fmt;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Debug, Clone)]
pub struct Hashes(pub Vec<[u8; 20]>);

struct HashVisitor;

impl<'de> Visitor<'de> for HashVisitor {
    type Value = Hashes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a byte slice whose length is a multiple of 20")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if v.len() % 20 != 0 {
            return Err(E::custom(format!(
                "length {} is not a multiple of 20",
                v.len()
            )));
        }

        Ok(Hashes(
            v.chunks_exact(20)
                .map(|i| i.try_into().expect("should not happen"))
                .collect(),
        ))
    }
}

impl<'de> Deserialize<'de> for Hashes {
    fn deserialize<D>(deserializer: D) -> Result<Hashes, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(HashVisitor)
    }
}

impl Serialize for Hashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(
            self.0
                .iter()
                .flatten()
                .map(|i| *i)
                .collect::<Vec<_>>()
                .as_slice(),
        )
    }
}
