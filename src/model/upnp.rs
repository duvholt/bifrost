use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

const XML_DOCTYPE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

const XMLNS: &str = "urn:schemas-upnp-org:device-1-0";
const SCHEMA_DEVICE_BASIC: &str = "urn:schemas-upnp-org:device:Basic:1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "root")]
pub struct Root {
    #[serde(rename = "@xmlns")]
    xmlns: String,

    #[serde(rename = "specVersion")]
    pub spec_version: SpecVersion,

    #[serde(rename = "URLBase")]
    pub url_base: Url,

    pub device: Device,
}

impl Root {
    #[must_use]
    pub fn new(url_base: Url, device: Device) -> Self {
        Self {
            xmlns: XMLNS.to_string(),
            spec_version: SpecVersion::VERSION_1,
            url_base,
            device,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecVersion {
    pub major: u32,
    pub minor: u32,
}

impl SpecVersion {
    pub const VERSION_1: Self = Self { major: 1, minor: 0 };
}

impl Default for SpecVersion {
    fn default() -> Self {
        Self::VERSION_1
    }
}

mod prefixed_uuid {
    use serde::{Deserialize, Deserializer, Serializer};
    use uuid::Uuid;
    const PREFIX: &str = "uuid:";

    pub fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{PREFIX}{value}");
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let s: &str = Deserialize::deserialize(deserializer)?;
        let uuid = s
            .strip_prefix(PREFIX)
            .ok_or_else(|| D::Error::custom("Value does not start with 'uuid:' prefix"))?;

        Uuid::parse_str(uuid).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub device_type: String,

    pub friendly_name: String,

    pub manufacturer: String,

    #[serde(rename = "manufacturerURL", skip_serializing_if = "Option::is_none")]
    pub manufacturer_url: Option<Url>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_description: Option<String>,

    pub model_name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_number: Option<String>,

    #[serde(rename = "modelURL")]
    pub model_url: Option<Url>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,

    #[serde(rename = "UDN", with = "prefixed_uuid")]
    pub udn: Uuid,

    #[serde(rename = "UPC", skip_serializing_if = "Option::is_none")]
    pub upc: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub icon_list: Vec<Icon>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub service_list: Vec<Service>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub device_list: Vec<Device>,

    #[serde(rename = "presentationURL", skip_serializing_if = "Option::is_none")]
    pub presentation_url: Option<String>,
}

impl Device {
    pub fn new(
        friendly_name: impl AsRef<str>,
        manufacturer: impl AsRef<str>,
        model_name: impl AsRef<str>,
        udn: Uuid,
    ) -> Self {
        Self {
            device_type: SCHEMA_DEVICE_BASIC.to_string(),
            friendly_name: friendly_name.as_ref().into(),
            manufacturer: manufacturer.as_ref().into(),
            model_name: model_name.as_ref().into(),
            manufacturer_url: None,
            model_description: None,
            model_number: None,
            model_url: None,
            serial_number: None,
            udn,
            upc: None,
            icon_list: vec![],
            service_list: vec![],
            device_list: vec![],
            presentation_url: None,
        }
    }

    #[must_use]
    pub fn with_manufacturer_url(self, value: Url) -> Self {
        Self {
            manufacturer_url: Some(value),
            ..self
        }
    }

    #[must_use]
    pub fn with_model_description(self, value: impl Into<String>) -> Self {
        Self {
            model_description: Some(value.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_model_number(self, value: impl Into<String>) -> Self {
        Self {
            model_number: Some(value.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_model_url(self, value: Url) -> Self {
        Self {
            model_url: Some(value),
            ..self
        }
    }

    #[must_use]
    pub fn with_serial_number(self, value: impl Into<String>) -> Self {
        Self {
            serial_number: Some(value.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_upc(self, value: String) -> Self {
        Self {
            upc: Some(value),
            ..self
        }
    }

    #[must_use]
    pub fn with_presentation_url(self, value: impl Into<String>) -> Self {
        Self {
            presentation_url: Some(value.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_device(mut self, value: Self) -> Self {
        self.add_device(value);
        self
    }

    pub fn add_device(&mut self, value: Self) {
        self.device_list.push(value);
    }
}

pub fn to_xml(value: impl Serialize) -> Result<String, quick_xml::se::SeError> {
    let mut res = XML_DOCTYPE.to_string() + "\n";

    // set up a serializer with indentation that appends to `res`
    let mut ser = quick_xml::se::Serializer::new(&mut res);
    ser.indent(' ', 2);

    // serialize value, with final newline
    value.serialize(ser)?;
    res.push('\n');

    Ok(res)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Icon {
    mimetype: String,
    width: u32,
    height: u32,
    depth: u32,
    url: Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    #[serde(rename = "serviceType")]
    service_type: Url,

    #[serde(rename = "serviceId")]
    service_id: Url,

    #[serde(rename = "SCPDURL")]
    scpd_url: Url,

    #[serde(rename = "controlURL")]
    control_url: Url,

    #[serde(rename = "eventSubURL")]
    event_sub_url: Url,
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use url::Url;
    use uuid::{Uuid, uuid};

    const UUID: Uuid = uuid!("01234567-89ab-cdef-0123-456789abcdef");

    use crate::model::upnp::{
        Device, Icon, Root, SCHEMA_DEVICE_BASIC, Service, XML_DOCTYPE, XMLNS, to_xml,
    };

    // convert using `to_xml()`, but trim lines to avoid having to indent test results
    fn make_xml(obj: impl Serialize) -> String {
        to_xml(&obj).unwrap().lines().map(str::trim).collect()
    }

    #[test]
    fn uuid_prefix() {
        #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
        struct Prefixed {
            #[serde(with = "super::prefixed_uuid")]
            uuid: Uuid,
        }

        let orig = Prefixed { uuid: UUID };
        let json = serde_json::to_string(&orig).unwrap();

        let expected = format!(r#"{{"uuid":"uuid:{UUID}"}}"#);

        assert_eq!(json, expected);

        let parsed: Prefixed = serde_json::from_str(&json).unwrap();

        assert_eq!(orig, parsed);
    }

    #[test]
    fn serialize_service() {
        let svc = Service {
            service_type: Url::parse("http://service_type/").unwrap(),
            service_id: Url::parse("http://service_id/").unwrap(),
            scpd_url: Url::parse("http://scpd_url/").unwrap(),
            control_url: Url::parse("http://control_url/").unwrap(),
            event_sub_url: Url::parse("http://event_sub_url/").unwrap(),
        };

        let a = make_xml(&svc);
        let b = [
            XML_DOCTYPE,
            "<Service>",
            "<serviceType>http://service_type/</serviceType>",
            "<serviceId>http://service_id/</serviceId>",
            "<SCPDURL>http://scpd_url/</SCPDURL>",
            "<controlURL>http://control_url/</controlURL>",
            "<eventSubURL>http://event_sub_url/</eventSubURL>",
            "</Service>",
        ]
        .concat();

        assert_eq!(a, b);
    }

    #[test]
    fn serialize_icon() {
        let icon = Icon {
            mimetype: "mime/type".into(),
            width: 42,
            height: 32,
            depth: 17,
            url: Url::parse("http://example.org/icon.png").unwrap(),
        };

        let a = make_xml(&icon);
        let b = [
            XML_DOCTYPE,
            "<Icon>",
            "<mimetype>mime/type</mimetype>",
            "<width>42</width>",
            "<height>32</height>",
            "<depth>17</depth>",
            "<url>http://example.org/icon.png</url>",
            "</Icon>",
        ]
        .concat();

        assert_eq!(a, b);
    }

    #[test]
    fn serialize_device() {
        let friendly_name = "Plumbus";
        let manufacturer = "Plumbubo Prime 51b";
        let model_name = "Plumbus 9000";
        let udn = UUID;
        let dev = Device::new(friendly_name, manufacturer, model_name, udn);

        let a = make_xml(&dev);
        let b = [
            XML_DOCTYPE,
            "<Device>",
            "<deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>",
            "<friendlyName>Plumbus</friendlyName>",
            "<manufacturer>Plumbubo Prime 51b</manufacturer>",
            "<modelName>Plumbus 9000</modelName>",
            "<modelURL/>",
            "<UDN>uuid:01234567-89ab-cdef-0123-456789abcdef</UDN>",
            "</Device>",
        ]
        .concat();

        assert_eq!(a, b);
    }

    #[test]
    fn serialize_device_with_subdevice() {
        let friendly_name = "Plumbus";
        let manufacturer = "Plumbubo Prime 51b";
        let model_name = "Plumbus 9000";
        let udn = UUID;
        let mut dev = Device::new(friendly_name, manufacturer, model_name, udn);

        dev.device_list.push(dev.clone());

        let device_body = [
            "<deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>",
            "<friendlyName>Plumbus</friendlyName>",
            "<manufacturer>Plumbubo Prime 51b</manufacturer>",
            "<modelName>Plumbus 9000</modelName>",
            "<modelURL/>",
            "<UDN>uuid:01234567-89ab-cdef-0123-456789abcdef</UDN>",
        ]
        .concat();

        let a = make_xml(&dev);
        let b = [
            XML_DOCTYPE,
            "<Device>",
            &device_body,
            "<deviceList>",
            &device_body,
            "</deviceList>",
            "</Device>",
        ]
        .concat();

        assert_eq!(a, b);
    }

    #[test]
    fn serialize_root() {
        let friendly_name = "Plumbus";
        let manufacturer = "Plumbubo Prime 51b";
        let model_name = "Plumbus 9000";
        let presentation_url = "index.html";
        let model_description = "Special Fleep Edition";
        let model_url = "portal://51b.prime.plumbubo/plumbus9000";
        let manufacturer_url = "portal:://51b.prime.plumbubo";
        let serial_number = "C137";
        let model_number = "PB9000";
        let base_url = "http://example.org/base";
        let udn = UUID;
        let base_url = Url::parse(base_url).unwrap();
        let dev = Device::new(friendly_name, manufacturer, model_name, udn)
            .with_manufacturer_url(Url::parse(manufacturer_url).unwrap())
            .with_presentation_url(presentation_url)
            .with_model_description(model_description)
            .with_model_url(Url::parse(model_url).unwrap())
            .with_model_number(model_number)
            .with_serial_number(serial_number);
        let root = Root::new(base_url.clone(), dev);

        let a = make_xml(&root);
        let b = [
            XML_DOCTYPE,
            &format!("<root xmlns=\"{XMLNS}\">"),
            "<specVersion>",
            "<major>1</major>",
            "<minor>0</minor>",
            "</specVersion>",
            &format!("<URLBase>{base_url}</URLBase>"),
            "<device>",
            &format!("<deviceType>{SCHEMA_DEVICE_BASIC}</deviceType>"),
            &format!("<friendlyName>{friendly_name}</friendlyName>"),
            &format!("<manufacturer>{manufacturer}</manufacturer>"),
            &format!("<manufacturerURL>{manufacturer_url}</manufacturerURL>"),
            &format!("<modelDescription>{model_description}</modelDescription>"),
            &format!("<modelName>{model_name}</modelName>"),
            &format!("<modelNumber>{model_number}</modelNumber>"),
            &format!("<modelURL>{model_url}</modelURL>"),
            &format!("<serialNumber>{serial_number}</serialNumber>"),
            &format!("<UDN>uuid:{UUID}</UDN>"),
            &format!("<presentationURL>{presentation_url}</presentationURL>"),
            "</device>",
            "</root>",
        ]
        .concat();

        assert_eq!(a, b);
    }
}
