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

    #[error("Cannot merge json difference between non-map object")]
    Unmergable,
}

/// Error types for Hue Bridge v1 API
#[derive(Error, Debug, Clone, Copy)]
pub enum HueApiV1Error {
    /// Type 1
    #[error("Unauthorized")]
    UnauthorizedUser = 1,

    /// Type 2
    #[error("Body contains invalid JSON")]
    BodyContainsInvalidJson = 2,

    /// Type 3
    #[error("Resource not found")]
    ResourceNotfound = 3,

    /// Type 4
    #[error("Method not available for resource")]
    MethodNotAvailableForResource = 4,

    /// Type 5
    #[error("Missing parameters in body")]
    MissingParametersInBody = 5,

    /// Type 6
    #[error("Parameter not available")]
    ParameterNotAvailable = 6,

    /// Type 7
    #[error("Invalid value for parameter")]
    InvalidValueForParameter = 7,

    /// Type 8
    #[error("Parameter not modifiable")]
    ParameterNotModifiable = 8,

    /// Type 11
    #[error("Too many items in list")]
    TooManyItemsInList = 11,

    /// Type 12
    #[error("Portal connection is required")]
    PortalConnectionIsRequired = 12,

    /// Type 901
    #[error("Internal bridge error")]
    BridgeInternalError = 901,
}

impl HueApiV1Error {
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[must_use]
    pub const fn error_code(&self) -> u32 {
        *self as u32
    }
}

pub type HueResult<T> = Result<T, HueError>;
