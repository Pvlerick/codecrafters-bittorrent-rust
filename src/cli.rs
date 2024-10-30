use std::{net::SocketAddrV4, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about= None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Command {
    Decode {
        value: String,
    },
    Info {
        torrent: PathBuf,
    },
    Peers {
        torrent: PathBuf,
    },
    Handshake {
        torrent: PathBuf,
        peer: SocketAddrV4,
    },
    #[command(name = "download_piece")]
    DownloadPiece {
        #[arg(short, long)]
        output: Option<PathBuf>,
        torrent: PathBuf,
        #[arg(default_value_t = 0)]
        start: u32,
    },
    Download {
        #[arg(short, long)]
        output: Option<PathBuf>,
        torrent: PathBuf,
    },
    #[command(name = "magnet_parse")]
    MagnetParse {
        magnet_link: String,
    },
    #[command(name = "magnet_handshake")]
    MagnetHandshake {
        magnet_link: String,
    },
    #[command(name = "magnet_info")]
    MagnetInfo {
        magnet_link: String,
    },
    #[command(name = "magnet_download_piece")]
    MagnetDownloadPiece {
        #[arg(short, long)]
        output: Option<PathBuf>,
        magnet_link: String,
        #[arg(default_value_t = 0)]
        start: u32,
    },
    MagnetDownload {
        #[arg(short, long)]
        output: Option<PathBuf>,
        magnet_link: String,
    },
}

#[cfg(test)]
mod test {
    use std::{net::SocketAddrV4, str::FromStr};

    use clap::Parser;

    use crate::cli::Command;

    use super::Args;

    #[test]
    fn parse_socket_addr_v4() -> anyhow::Result<()> {
        let args = Args::parse_from("x handshake /tmp/sample.torrent 127.0.0.1:48845".split(" "));
        assert_eq!(
            Command::Handshake {
                torrent: "/tmp/sample.torrent".into(),
                peer: SocketAddrV4::from_str("127.0.0.1:48845")?
            },
            args.command
        );
        Ok(())
    }
}
