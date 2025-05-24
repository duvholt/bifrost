use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::api::{DeviceArchetype, DeviceProductData};

// This file contains discovered product data from multiple sources,
// including data samples from the community, and various open source or public
// domain examples, including:
//
//  - https://github.com/niomwungeri-fabrice/hue-v2-api
//
// This file is a best-effort attempt to gather a database of product data, to
// provide more realistic API data, even when certain information is not
// available from the backend (zigbee2mqtt).

#[derive(Debug, Clone)]
pub struct SimpleProductData<'a> {
    pub manufacturer_name: &'a str,
    pub product_name: &'a str,
    pub product_archetype: DeviceArchetype,
    pub hardware_platform_type: Option<&'a str>,
}

impl<'a> SimpleProductData<'a> {
    /// helper function to construct signify devices
    #[must_use]
    pub const fn signify(
        product_name: &'a str,
        product_archetype: DeviceArchetype,
        hardware_platform_type: &'a str,
    ) -> Self {
        Self {
            manufacturer_name: DeviceProductData::SIGNIFY_MANUFACTURER_NAME,
            product_name,
            product_archetype,
            hardware_platform_type: Some(hardware_platform_type),
        }
    }
}

static PRODUCT_DATA: LazyLock<BTreeMap<&str, SimpleProductData>> = LazyLock::new(make_product_data);

#[cfg_attr(coverage_nightly, coverage(off))]
fn make_product_data() -> BTreeMap<&'static str, SimpleProductData<'static>> {
    // use shorter alias for better formatting
    #[allow(clippy::enum_glob_use)]
    use DeviceArchetype::*;
    use SimpleProductData as SPD;

    maplit::btreemap! {
        "915005987201" => SPD::signify("Signe gradient floor", HueSigne, "100b-118"),
        "929003053301_01" => SPD::signify("Hue Ensis up", PendantLong, "100b-11f"),
        "929003053301_02" => SPD::signify("Hue Ensis down", PendantLong, "100b-11f"),
        "LCA001" => SPD::signify("Hue color lamp", SultanBulb, "100b-112"),
        "LCD007" => SPD::signify("Hue color downlight", RecessedCeiling, "100b-114"),
        "LCE002" => SPD::signify("Hue color candle", CandleBulb, "100b-114"),
        "LCG002" => SPD::signify("Hue color spot", SpotBulb, "100b-114"),
        "LCT014" => SPD::signify("Hue color lamp", SultanBulb, "100b-10c"),
        "LCT015" => SPD::signify("Hue color lamp", SultanBulb, "100b-10c"),
        "LCT016" => SPD::signify("Hue color lamp", SultanBulb, "100b-10c"),
        "LCX001" => SPD::signify("Hue play gradient lightstrip", HueLightstripTv, "100b-118"),
        "LCX005" => SPD::signify("Hue play gradient lightstrip", HueLightstripPc, "100b-118"),
        "LLC020" => SPD::signify("Hue go", HueGo, "100b-108"),
        "LOM001" => SPD::signify("Hue Smart plug", Plug, "100b-115"),
        "LST002" => SPD::signify("Hue lightstrip plus", HueLightstrip, "100b-10f"),
        "LTO001" => SPD::signify("Hue filament bulb", VintageBulb, "100b-114"),
        "LTW015" => SPD::signify("Hue ambiance lamp", SultanBulb, "100b-10c"),
        "LWA003" => SPD::signify("Hue white lamp", SultanBulb, "100b-114"),
        "LWA029" => SPD::signify("Hue white lamp", SultanBulb, "100b-114"),
        "LWB014" => SPD::signify("Hue white lamp", ClassicBulb, "100b-10c"),
        "RDM002" => SPD::signify("Hue tap dial switch", UnknownArchetype, "100b-121"),
        "RWL021" => SPD::signify("Hue dimmer switch", UnknownArchetype, "100b-109"),
        "RWL022" => SPD::signify("Hue dimmer switch", UnknownArchetype, "100b-119"),
        "SML001" => SPD::signify("Hue motion sensor", UnknownArchetype, "100b-10d"),
        "SML002" => SPD::signify("Hue outdoor motion sensor", UnknownArchetype, "100b-10d"),
        "SML003" => SPD::signify("Hue motion sensor", UnknownArchetype, "100b-11b"),

        "Z3-1BRL" => SPD {
            manufacturer_name: "Lutron",
            product_name: "Lutron Aurora",
            product_archetype: UnknownArchetype,
            hardware_platform_type: Some("1144-0"),
        },
    }
}

#[must_use]
pub fn product_data(model_id: &str) -> Option<SimpleProductData<'static>> {
    PRODUCT_DATA.get(model_id).cloned()
}

#[must_use]
pub fn product_archetype(model_id: &str) -> Option<DeviceArchetype> {
    product_data(model_id).map(|pd| pd.product_archetype)
}

#[must_use]
pub fn hardware_platform_type(model_id: &str) -> Option<&'static str> {
    product_data(model_id).and_then(|pd| pd.hardware_platform_type)
}

#[cfg(test)]
mod tests {
    use crate::api::DeviceArchetype;
    use crate::devicedb::{hardware_platform_type, product_archetype, product_data};

    #[test]
    fn lookup_spf() {
        assert!(product_data("LCX001").is_some());
    }

    #[test]
    fn lookup_archetype() {
        assert_eq!(
            product_archetype("LCX001").unwrap(),
            DeviceArchetype::HueLightstripTv
        );
    }

    #[test]
    fn lookup_platform_type() {
        assert_eq!(hardware_platform_type("LCX001").unwrap(), "100b-118",);
    }
}
