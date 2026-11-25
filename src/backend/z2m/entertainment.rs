use std::collections::BTreeMap;

use serde_json::json;

use hue::stream::HueStreamLightsV2;
use hue::zigbee::{
    EntertainmentZigbeeStream, HueEntFrameLightRecord, LightRecordMode,
    PHILIPS_HUE_ZIGBEE_VENDOR_ID,
};
use z2m::request::Z2mRequest;
use zcl::attr::ZclDataType;

use crate::backend::z2m::websocket::Z2mWebSocket;
use crate::error::ApiResult;

pub struct EntStream {
    pub stream: EntertainmentZigbeeStream,
    pub target: String,
    pub addrs: BTreeMap<String, Vec<u16>>,
    pub channels: BTreeMap<u8, (u16, LightRecordMode)>,
}

/// Split `items` into the fewest slices of at most `max` elements each, with
/// the slice sizes as even as possible (e.g. 5 items with `max` 4 become 3+2,
/// not 4+1). Yields nothing for an empty slice.
fn balanced_chunks<T>(items: &[T], max: usize) -> impl Iterator<Item = &[T]> {
    let parts = items.len().div_ceil(max.max(1));
    let base = items.len().checked_div(parts).unwrap_or(0);
    let extra = items.len().checked_rem(parts).unwrap_or(0);
    let mut rest = items;

    (0..parts).map(move |i| {
        let (chunk, tail) = rest.split_at(base + usize::from(i < extra));
        rest = tail;
        chunk
    })
}

impl EntStream {
    /// Maximum number of light records sent in one Zigbee entertainment
    /// message.
    ///
    /// A message is 6 header bytes plus 7 bytes per light record, so 4 records
    /// are 34 bytes. How much a single Zigbee message can carry depends on the
    /// security level and on the route to the light (a source route takes 2
    /// bytes per hop), and is only about 50 bytes in the worst case.
    /// Messages with 6 records (48 bytes) were observed to be rejected with
    /// `MESSAGE_TOO_LONG`, while 4 records leave enough room.
    const MAX_RECORDS_PER_MESSAGE: usize = 4;

    #[must_use]
    pub fn new(
        counter: u32,
        target: &str,
        addrs: BTreeMap<String, Vec<u16>>,
        channels: BTreeMap<u8, (u16, LightRecordMode)>,
    ) -> Self {
        Self {
            stream: EntertainmentZigbeeStream::new(counter),
            target: target.to_string(),
            addrs,
            channels,
        }
    }

    #[must_use]
    pub fn z2m_set_entertainment_brightness(brightness: u8) -> Z2mRequest<'static> {
        Z2mRequest::Write {
            cluster: EntertainmentZigbeeStream::CLUSTER,
            payload: json!({
                EntertainmentZigbeeStream::CMD_LIGHT_BALANCE.to_string(): {
                    "manufacturerCode": PHILIPS_HUE_ZIGBEE_VENDOR_ID,
                    "type": ZclDataType::ZclU8 as u8,
                    "value": brightness,
                }
            }),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[must_use]
    pub fn generate_frame(&self, frame: &HueStreamLightsV2) -> Vec<HueEntFrameLightRecord> {
        let mut blks = vec![];
        match frame {
            HueStreamLightsV2::Rgb(rgb) => {
                for light in rgb {
                    let (xy, bright) = light.rgb.to_xy();

                    let brightness = (bright / 255.0 * 2047.0).clamp(1.0, 2047.0) as u16;
                    if let Some((chan, mode)) = self.channels.get(&light.channel) {
                        let raw = xy.to_quant();
                        let lrec = HueEntFrameLightRecord::new(*chan, brightness, *mode, raw);
                        blks.push(lrec);
                    }
                }
            }
            HueStreamLightsV2::Xy(xy) => {
                for light in xy {
                    let (xy, bright) = light.xy.to_xy();

                    let brightness = (bright / 255.0 * 2047.0).clamp(1.0, 2047.0) as u16;
                    if let Some((chan, mode)) = self.channels.get(&light.channel) {
                        let raw = xy.to_quant();
                        let lrec = HueEntFrameLightRecord::new(*chan, brightness, *mode, raw);
                        blks.push(lrec);
                    }
                }
            }
        }

        blks
    }

    pub async fn start_stream(&mut self, z2mws: &mut Z2mWebSocket) -> ApiResult<()> {
        log::debug!("Entertainment addrs: {:#?}", &self.addrs);
        log::debug!("Entertainment channels: {:#?}", &self.channels);
        for (dev, segments) in &self.addrs {
            let z2mreq = Self::z2m_set_entertainment_brightness(0xFE);
            z2mws.send(dev, &z2mreq).await?;

            if segments.len() <= 1 {
                continue;
            }

            let mapping = self.stream.segment_mapping(segments)?;
            z2mws.send_zigbee_message(dev, &mapping).await?;
        }

        Ok(())
    }

    pub async fn reset_stream(&mut self, z2mws: &mut Z2mWebSocket) -> ApiResult<()> {
        let reset = self.stream.reset()?;
        for topic in self.addrs.keys() {
            log::debug!("Sending reset to {topic}");
            z2mws.send_zigbee_message(topic, &reset).await?;
        }

        Ok(())
    }

    pub async fn frame(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        frame: &HueStreamLightsV2,
    ) -> ApiResult<()> {
        let blks = self.generate_frame(frame);

        // A single Zigbee message has limited room for payload. Frames with
        // many light records (e.g. multi-segment lamps such as the Hue Signe,
        // or areas with several lights) are rejected with MESSAGE_TOO_LONG,
        // so send them as several messages, each with its own frame counter.
        //
        // A frame without any matching light records is not sent at all.
        for chunk in balanced_chunks(&blks, Self::MAX_RECORDS_PER_MESSAGE) {
            let message = self.stream.frame(chunk.to_vec())?;
            z2mws
                .send_entertainment_frame(&self.target, &message)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{EntStream, balanced_chunks};

    fn sizes(len: usize, max: usize) -> Vec<usize> {
        let items: Vec<usize> = (0..len).collect();
        balanced_chunks(&items, max).map(<[usize]>::len).collect()
    }

    #[test]
    fn empty_yields_no_chunks() {
        assert_eq!(sizes(0, 4), Vec::<usize>::new());
    }

    #[test]
    fn fits_in_one_chunk() {
        assert_eq!(sizes(1, 4), [1]);
        assert_eq!(sizes(3, 4), [3]);
        assert_eq!(sizes(4, 4), [4]);
    }

    #[test]
    fn chunks_are_as_even_as_possible() {
        assert_eq!(sizes(5, 4), [3, 2]);
        assert_eq!(sizes(6, 4), [3, 3]);
        assert_eq!(sizes(8, 4), [4, 4]);
        assert_eq!(sizes(14, 4), [4, 4, 3, 3]);
    }

    #[test]
    fn respects_the_maximum() {
        for max in 1..=8 {
            for len in 0..=40 {
                let s = sizes(len, max);
                assert_eq!(s.iter().sum::<usize>(), len);
                assert!(s.iter().all(|&n| (1..=max).contains(&n)));
                assert_eq!(s.len(), len.div_ceil(max));
            }
        }
    }

    #[test]
    fn chunks_keep_the_original_order() {
        let items: Vec<usize> = (0..14).collect();
        let flat: Vec<usize> = balanced_chunks(&items, 4).flatten().copied().collect();
        assert_eq!(flat, items);
    }

    #[test]
    fn zero_maximum_is_treated_as_one() {
        assert_eq!(sizes(3, 0), [1, 1, 1]);
    }

    #[test]
    fn two_signes_are_split_into_four_messages() {
        // two Hue Signe lamps with 7 segments each: 14 records
        assert_eq!(sizes(14, EntStream::MAX_RECORDS_PER_MESSAGE), [4, 4, 3, 3]);
    }
}
