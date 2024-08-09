use serde::{Deserialize, Serialize};

use crate::hue::api::{Metadata, RType, ResourceLink};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Device {
    pub product_data: DeviceProductData,
    pub metadata: Metadata,
    pub services: Vec<ResourceLink>,
}

impl Device {
    #[must_use]
    pub fn light(&self) -> Option<&ResourceLink> {
        self.services.iter().find(|rl| rl.rtype == RType::Light)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceProductData {
    pub model_id: String,
    pub manufacturer_name: String,
    pub product_name: String,
    pub product_archetype: String,
    pub certified: bool,
    pub software_version: String,
}

impl DeviceProductData {
    #[must_use]
    pub fn hue_color_spot() -> Self {
        Self {
            model_id: "LCG002".to_string(),
            manufacturer_name: "Signify Netherlands B.V.".to_string(),
            product_name: "Hue color spot".to_string(),
            product_archetype: "spot_bulb".to_string(),
            certified: true,
            software_version: "1.104.2".to_string(),
        }
    }

    #[must_use]
    pub fn hue_bridge_v2() -> Self {
        Self {
            certified: true,
            manufacturer_name: "Signify Netherlands B.V.".to_string(),
            model_id: "BSB002".to_string(),
            product_archetype: "bridge_v2".to_string(),
            product_name: "Hue Bridge".to_string(),
            software_version: "1.60.1960149090".to_string(),
        }
    }
}
