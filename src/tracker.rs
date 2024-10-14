use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Response {
    pub interval: usize,
    pub peers: Vec<String>,
}
