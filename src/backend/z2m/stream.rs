use hue::zigbee::{ZigbeeMessage, ZigbeeTarget};

use crate::backend::z2m::zclcommand::hue_zclcommand;
use crate::error::{ApiError, ApiResult};
use crate::z2m::request::Z2mRequest;

pub struct Z2mTarget {
    pub device: String,
}

impl Z2mTarget {
    #[must_use]
    pub fn new(device: &str) -> Self {
        Self {
            device: device.to_string(),
        }
    }
}

impl ZigbeeTarget for Z2mTarget {
    type Error = ApiError;
    type Output = Z2mRequest<'static>;

    fn send(&mut self, msg: ZigbeeMessage) -> ApiResult<Self::Output> {
        let cluster = match msg.cluster {
            0xFC01 => "manuSpecificPhilips3",
            _ => Err(ApiError::ZigbeeMessageError)?,
        };

        let res = hue_zclcommand(cluster, msg.command, &msg.data);

        Ok(Z2mRequest::Raw(res))
    }
}
