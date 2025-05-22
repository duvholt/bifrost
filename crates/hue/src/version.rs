use std::fmt::Debug;

use crate::{HUE_BRIDGE_V2_DEFAULT_APIVERSION, HUE_BRIDGE_V2_DEFAULT_SWVERSION};

#[derive(Clone, Eq, PartialEq)]
pub struct SwVersion {
    version: u64,
    name: String,
}

impl PartialOrd for SwVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SwVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version.cmp(&other.version)
    }
}

impl Default for SwVersion {
    fn default() -> Self {
        Self {
            version: HUE_BRIDGE_V2_DEFAULT_SWVERSION,
            name: HUE_BRIDGE_V2_DEFAULT_APIVERSION.to_string(),
        }
    }
}

impl Debug for SwVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.version)
    }
}

impl SwVersion {
    #[must_use]
    pub const fn new(version: u64, name: String) -> Self {
        Self { version, name }
    }

    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn get_legacy_apiversion(&self) -> String {
        let version = format!("{:05}", self.version);
        format!("{}.{}.0", &version[0..1], &version[2..4])
    }

    #[must_use]
    pub fn get_legacy_swversion(&self) -> String {
        format!("{}", &self.version)
    }

    #[must_use]
    /// Format a version into the hue legacy format
    ///
    /// Legacy version is constructed from the version number.
    ///
    /// ```text
    /// Example:
    ///   1968096020
    ///
    ///   1_68______ (these digits used)
    ///
    ///   1.68.1968096020
    ///     ^^^^^^^^^^ append whole version number at the end
    /// ```
    pub fn get_software_version(&self) -> String {
        let version = format!("{:05}", self.version);
        format!("{}.{}.{}", &version[0..1], &version[2..4], version)
    }
}

#[cfg(test)]
mod tests {
    use crate::version::SwVersion;
    use crate::{HUE_BRIDGE_V2_DEFAULT_APIVERSION, HUE_BRIDGE_V2_DEFAULT_SWVERSION};

    #[allow(clippy::nonminimal_bool)]
    #[test]
    fn partial_ord() {
        let a = SwVersion {
            version: 10,
            name: String::new(),
        };
        let b = SwVersion {
            version: 20,
            name: String::new(),
        };

        assert!(a < b);
        assert!(!(a >= b));
    }

    #[test]
    fn default() {
        let def = SwVersion::default();

        assert_eq!(
            def,
            SwVersion {
                version: HUE_BRIDGE_V2_DEFAULT_SWVERSION,
                name: HUE_BRIDGE_V2_DEFAULT_APIVERSION.to_string(),
            }
        );
    }

    #[test]
    fn debug() {
        let version = SwVersion {
            version: 1234,
            name: "name".to_string(),
        };
        assert_eq!(format!("{version:?}"), "name (1234)");
    }

    #[test]
    fn as_u64() {
        assert_eq!(
            SwVersion::default().as_u64(),
            HUE_BRIDGE_V2_DEFAULT_SWVERSION
        );
    }

    #[test]
    fn get_legacy_swversion() {
        let version = SwVersion::new(1234, String::new());
        assert_eq!(version.get_legacy_swversion(), "1234");
    }

    #[test]
    fn get_legacy_apiversion() {
        let version = SwVersion::new(12345, String::new());
        assert_eq!(version.get_legacy_apiversion(), "1.34.0");
    }

    #[test]
    fn get_software_version() {
        let version = SwVersion::new(123_456, String::new());
        assert_eq!(version.get_software_version(), "1.34.123456");
    }
}
