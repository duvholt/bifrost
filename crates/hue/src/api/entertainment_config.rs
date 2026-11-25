use serde::{Deserialize, Serialize};

use crate::api::ResourceLink;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfiguration {
    pub name: String,
    pub configuration_type: EntertainmentConfigurationType,
    pub metadata: EntertainmentConfigurationMetadata,
    pub status: EntertainmentConfigurationStatus,
    pub stream_proxy: EntertainmentConfigurationStreamProxy,
    pub locations: EntertainmentConfigurationLocations,
    pub light_services: Vec<ResourceLink>,
    pub channels: Vec<EntertainmentConfigurationChannels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_streamer: Option<ResourceLink>,
}

impl EntertainmentConfiguration {
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.active_streamer.is_some() || self.status != EntertainmentConfigurationStatus::Inactive
    }

    pub const fn stop_streaming(&mut self) {
        self.active_streamer = None;
        self.status = EntertainmentConfigurationStatus::Inactive;
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationStatus {
    Active,
    Inactive,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationMetadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamProxy {
    pub mode: EntertainmentConfigurationStreamProxyMode,
    pub node: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocations {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocations>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocations {
    pub equalization_factor: f64,
    pub position: Position,
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationChannels {
    pub channel_id: u32,
    pub position: Position,
    pub members: Vec<EntertainmentConfigurationStreamMembers>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position {
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// `count` positions, evenly spaced on the line from `start` to `end`
    /// (both points included)
    ///
    /// A single position is placed at `start`.
    #[must_use]
    pub fn spread(start: &Self, end: &Self, count: usize) -> Vec<Self> {
        let last = count.saturating_sub(1);

        (0..count)
            .map(|index| {
                if last == 0 {
                    return start.clone();
                }

                let t = f64::from(u32::try_from(index).unwrap_or(u32::MAX))
                    / f64::from(u32::try_from(last).unwrap_or(u32::MAX));

                Self {
                    x: (end.x - start.x).mul_add(t, start.x),
                    y: (end.y - start.y).mul_add(t, start.y),
                    z: (end.z - start.z).mul_add(t, start.z),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    fn assert_close(a: &Position, b: &Position) {
        assert!((a.x - b.x).abs() < 1e-9, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < 1e-9, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < 1e-9, "z: {} != {}", a.z, b.z);
    }

    #[test]
    fn spread_includes_both_end_points() {
        let start = Position::new(1.0, 1.0, -0.77);
        let end = Position::new(1.0, 1.0, 0.77);

        let res = Position::spread(&start, &end, 7);

        assert_eq!(res.len(), 7);
        assert_close(&res[0], &start);
        assert_close(&res[6], &end);
    }

    #[test]
    fn spread_is_evenly_spaced() {
        let start = Position::new(0.0, 0.0, -1.0);
        let end = Position::new(0.0, 0.0, 1.0);

        let res = Position::spread(&start, &end, 5);

        let z: Vec<f64> = res.iter().map(|p| p.z).collect();
        for (got, want) in z.iter().zip([-1.0, -0.5, 0.0, 0.5, 1.0]) {
            assert!((got - want).abs() < 1e-9, "{got} != {want}");
        }
    }

    #[test]
    fn swapping_the_end_points_reverses_the_order() {
        let a = Position::new(-1.0, 1.0, 0.54);
        let b = Position::new(-1.0, 1.0, -1.0);

        let forward = Position::spread(&a, &b, 7);
        let flipped = Position::spread(&b, &a, 7);

        for (f, r) in forward.iter().zip(flipped.iter().rev()) {
            assert_close(f, r);
        }
    }

    #[test]
    fn spread_handles_small_counts() {
        let start = Position::new(1.0, 2.0, 3.0);
        let end = Position::new(4.0, 5.0, 6.0);

        assert!(Position::spread(&start, &end, 0).is_empty());

        let one = Position::spread(&start, &end, 1);
        assert_eq!(one.len(), 1);
        assert_close(&one[0], &start);

        let two = Position::spread(&start, &end, 2);
        assert_close(&two[0], &start);
        assert_close(&two[1], &end);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamMembers {
    pub service: ResourceLink,
    pub index: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationAction {
    Start,
    Stop,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationType {
    Screen,
    Monitor,
    Music,
    #[serde(rename = "3dspace")]
    Space3D,
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EntertainmentConfigurationUpdate {
    pub configuration_type: Option<EntertainmentConfigurationType>,
    pub metadata: Option<EntertainmentConfigurationMetadata>,
    pub action: Option<EntertainmentConfigurationAction>,
    pub stream_proxy: Option<EntertainmentConfigurationStreamProxyUpdate>,
    pub locations: Option<EntertainmentConfigurationLocationsUpdate>,
}

impl EntertainmentConfigurationUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EntertainmentConfigurationNew {
    pub configuration_type: EntertainmentConfigurationType,
    pub metadata: EntertainmentConfigurationMetadata,
    pub stream_proxy: Option<EntertainmentConfigurationStreamProxyUpdate>,
    pub locations: EntertainmentConfigurationLocationsNew,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntertainmentConfigurationStreamProxyMode {
    Auto,
    Manual,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum EntertainmentConfigurationStreamProxyUpdate {
    Auto,
    Manual { node: ResourceLink },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocationsUpdate {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocationsUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocationsNew {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocationsNew>,
}

impl From<EntertainmentConfigurationServiceLocationsNew>
    for EntertainmentConfigurationServiceLocationsUpdate
{
    fn from(value: EntertainmentConfigurationServiceLocationsNew) -> Self {
        Self {
            equalization_factor: Some(1.0),
            positions: value.positions,
            service: value.service,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocations>
    for EntertainmentConfigurationServiceLocationsUpdate
{
    fn from(value: EntertainmentConfigurationServiceLocations) -> Self {
        Self {
            equalization_factor: Some(value.equalization_factor),
            service: value.service,
            positions: value.positions,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocationsUpdate>
    for EntertainmentConfigurationServiceLocations
{
    fn from(value: EntertainmentConfigurationServiceLocationsUpdate) -> Self {
        Self {
            equalization_factor: value.equalization_factor.unwrap_or(1.0),
            service: value.service,
            position: value.positions.first().cloned().unwrap_or_default(),
            positions: value.positions,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocationsNew>
    for EntertainmentConfigurationServiceLocations
{
    fn from(value: EntertainmentConfigurationServiceLocationsNew) -> Self {
        Self {
            equalization_factor: 1.0,
            service: value.service,
            position: value.positions.first().cloned().unwrap_or_default(),
            positions: value.positions,
        }
    }
}

impl From<EntertainmentConfigurationLocationsNew> for EntertainmentConfigurationLocationsUpdate {
    fn from(value: EntertainmentConfigurationLocationsNew) -> Self {
        Self {
            service_locations: value
                .service_locations
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocationsUpdate {
    pub equalization_factor: Option<f64>,
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocationsNew {
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}
