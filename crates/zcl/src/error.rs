use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZclError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    HueError(#[from] hue::error::HueError),

    #[error("Attribute type 0x{0:02x} not supported")]
    UnsupportedAttrType(u8),

    #[error(transparent)]
    PackedStructError(#[from] packed_struct::PackingError),
}

pub type ZclResult<T> = Result<T, ZclError>;
