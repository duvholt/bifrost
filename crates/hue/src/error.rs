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

/// Error types for Hue Bridge v1 API
#[derive(Error, Debug)]
pub enum HueApiV1Error {
    /// Type 1
    #[error("Unauthorized")]
    UnauthorizedUser,

    /// Type 2
    #[error("Body contains invalid JSON")]
    BodyContainsInvalidJson,

    /// Type 3
    #[error("Resource not found")]
    ResourceNotfound,

    /// Type 4
    #[error("Method not available for resource")]
    MethodNotAvailableForResource,

    /// Type 5
    #[error("Missing parameters in body")]
    MissingParametersInBody,

    /// Type 6
    #[error("Parameter not available")]
    ParameterNotAvailable,

    /// Type 7
    #[error("Invalid value for parameter")]
    InvalidValueForParameter,

    /// Type 8
    #[error("Parameter not modifiable")]
    ParameterNotModifiable,

    /// Type 11
    #[error("Too many items in list")]
    TooManyItemsInList,

    /// Type 12
    #[error("Portal connection is required")]
    PortalConnectionIsRequired,

    /// Type 901
    #[error("Internal bridge error")]
    BridgeInternalError,
}

impl HueApiV1Error {
    pub fn error_code(&self) -> u32 {
        match self {
            Self::UnauthorizedUser => 1,
            Self::BodyContainsInvalidJson => 2,
            Self::ResourceNotfound => 3,
            Self::MethodNotAvailableForResource => 4,
            Self::MissingParametersInBody => 5,
            Self::ParameterNotAvailable => 6,
            Self::InvalidValueForParameter => 7,
            Self::ParameterNotModifiable => 8,
            Self::TooManyItemsInList => 11,
            Self::PortalConnectionIsRequired => 12,
            Self::BridgeInternalError => 901,
        }
    }
}

pub type HueResult<T> = Result<T, HueError>;
