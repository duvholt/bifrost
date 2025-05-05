use thiserror::Error;

use crate::api::RType;

#[derive(Error, Debug)]
pub enum HueError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    FromHexError(#[from] hex::FromHexError),

    #[error(transparent)]
    PackedStructError(#[from] packed_struct::PackingError),

    #[error(transparent)]
    UuidError(#[from] uuid::Error),

    #[error("Bad header in hue entertainment stream")]
    HueEntertainmentBadHeader,

    #[error("Failed to decode Hue Zigbee Update")]
    HueZigbeeDecodeError,

    #[error("Failed to encode Hue Zigbee Update")]
    HueZigbeeEncodeError,

    #[error("Failed to decode Hue Zigbee Update: Unknown flags {0:04x}")]
    HueZigbeeUnknownFlags(u16),

    #[error("Resource {0} not found")]
    NotFound(uuid::Uuid),

    #[error("Resource {0} not found")]
    V1NotFound(u32),

    #[error("Cannot allocate any more {0:?}")]
    Full(RType),

    #[error("Resource type wrong: expected {0:?} but found {1:?}")]
    WrongType(RType, RType),

    #[error("Cannot generate json difference between non-map objects")]
    Undiffable,
}

pub type HueResult<T> = Result<T, HueError>;
