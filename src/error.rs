use std::io;

use util::http::multipart;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Io: {0}")]
    Io(#[from] io::Error),
    #[error("Multipart: {0}")]
    Multipart(#[from] multipart::Error),
}
