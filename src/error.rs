use std::io;

use http_util::multipart;
use hyper::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Io: {0}")]
    Io(#[from] io::Error),
    #[error("Multipart: {0}")]
    Multipart(#[from] multipart::Error),
    #[error("Refract: {0}")]
    Refract(#[from] refract_core::RefractError),
    #[error("Reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Unauthenticated")]
    Unauthenticated,

    #[error("UnknownStatusCode: {0}")]
    UnknownStatusCode(StatusCode, String),
}
