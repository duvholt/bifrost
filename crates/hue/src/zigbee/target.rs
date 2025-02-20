use crate::error::HueResult;

/// A target for Zigbee requests
///
/// Receives commands, and processes them in a target-specific manner.
pub trait ZigbeeTarget {
    /// The result type when sending Zigbee commands. This could be a data
    /// structure, a handle, `()`, or whatever makes sense for the impl.
    type Result;

    fn send(&mut self, cluster: u16, command: u8, data: &[u8]) -> HueResult<Self::Result>;
}
