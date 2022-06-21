mod app;
mod config;
mod error;
mod registry;

pub use error::Error;
pub use registry::RootRegistry;

pub type Result<T> = std::result::Result<T, error::Error>;

pub fn release() -> bool {
    cfg!(not(debug_assertions))
}

pub fn debug() -> bool {
    cfg!(debug_assertions)
}

pub fn test() -> bool {
    cfg!(test)
}
