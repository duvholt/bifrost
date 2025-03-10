// Tool to discover DeviceProductData unknown in bifrost::hue::devicedb.
//
// cat samples/*.json | jq '.data? | .[]? | select(.product_data?.hardware_platform_type) | .product_data' | cargo run --example=convert-product-data
//
// Any output from the above command will be devices currently unknown in the device database.

use std::io::stdin;

use serde_json::Deserializer;

use hue::api::DeviceProductData;
use hue::devicedb::{product_data, SimpleProductData};

fn print_std(obj: DeviceProductData) {
    let spd = SimpleProductData {
        manufacturer_name: &obj.manufacturer_name,
        product_name: &obj.product_name,
        product_archetype: obj.product_archetype,
        hardware_platform_type: obj.hardware_platform_type.as_deref(),
    };
    println!(
        "{:?} => {},",
        obj.model_id,
        format!("{spd:?}").replace("SimpleProductData ", "SPD ")
    );
}

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let stream = Deserializer::from_reader(stdin()).into_iter::<DeviceProductData>();

    for obj in stream {
        let Ok(obj) = obj else {
            continue;
        };

        let pd = product_data(&obj.model_id);
        if pd.is_none() {
            if obj.manufacturer_name == DeviceProductData::SIGNIFY_MANUFACTURER_NAME {
                if let Some(hpt) = obj.hardware_platform_type {
                    println!(
                        "{:?} => SPD::signify({:?}, {:?}, {:?}),",
                        obj.model_id, obj.product_name, obj.product_archetype, hpt,
                    );
                    continue;
                }
            }

            print_std(obj);
        }
    }
}
