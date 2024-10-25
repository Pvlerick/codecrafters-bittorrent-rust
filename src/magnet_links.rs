use std::collections::HashMap;

use anyhow::Context;
use reqwest::Url;

pub struct MagnetLink {
    pub announce: Url,
    pub info_hash: [u8; 20],
}

impl MagnetLink {
    pub fn parse<T: ToString>(link: T) -> anyhow::Result<MagnetLink> {
        //TODO use AsRef<u8> ?
        let link = link.to_string();
        let payload = &link[8..];
        let map = serde_urlencoded::from_bytes::<HashMap<String, String>>(payload.as_bytes())
            .context("turing magnet link to hashmap")?;

        let hash = map.get("xt").context("getting xt key")?;
        dbg!(&hash.as_bytes()[9..]);
        let hash = hex::decode(&hash.as_bytes()[9..])?;

        Ok(Self {
            announce: Url::parse(map.get("tr").context("getting tr key")?)
                .context("parsing announce url")?,
            info_hash: TryInto::<[u8; 20]>::try_into(&hash[..20]).expect("hash is not 20 bytes"),
        })
    }
}

#[cfg(test)]
mod test {
    use reqwest::Url;

    use crate::magnet_links::MagnetLink;

    #[test]
    fn parse_link() -> anyhow::Result<()> {
        let res = MagnetLink::parse("magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce")?;

        assert_eq!(
            "ad42ce8109f54c99613ce38f9b4d87e70f24a165",
            hex::encode(res.info_hash)
        );
        assert_eq!(
            Url::parse("http://bittorrent-test-tracker.codecrafters.io/announce")?,
            res.announce
        );

        Ok(())
    }
}
