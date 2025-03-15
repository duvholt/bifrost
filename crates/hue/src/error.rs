use thiserror::Error;

#[derive(Error, Debug)]
pub enum HueError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    PackedStructError(#[from] packed_struct::PackingError),

    #[error("Failed to decode Hue Zigbee Update")]
    HueZigbeeDecodeError,

    #[error("Failed to decode Hue Zigbee Update: Unknown flags {0:04x}")]
    HueZigbeeUnknownFlags(u16),
}

pub type HueResult<T> = Result<T, HueError>;
