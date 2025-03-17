use crate::zigbee::ZigbeeMessage;

/// A target for Zigbee requests
///
/// Receives commands, and processes them in a target-specific manner.
pub trait ZigbeeTarget {
    /// The result type when sending Zigbee commands. This could be a data
    /// structure, a handle, `()`, or whatever makes sense for the impl.
    type Error;
    type Output;

    fn send(&mut self, msg: ZigbeeMessage) -> Result<Self::Output, Self::Error>;
}
