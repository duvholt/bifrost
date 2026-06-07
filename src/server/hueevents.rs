use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast::{Receiver, Sender};

use hue::event::EventBlock;

#[derive(Clone, Debug)]
pub struct HueEventRecord {
    timestamp: DateTime<Utc>,
    index: u32,
    pub block: EventBlock,
}

impl HueEventRecord {
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}:{}", self.timestamp.timestamp(), self.index)
    }

    #[must_use]
    pub const fn sortable_id(&self) -> (i64, u32) {
        (self.timestamp.timestamp(), self.index)
    }
}

#[derive(Clone, Debug)]
pub struct HueEventStream {
    timestamp: DateTime<Utc>,
    index: u32,
    hue_updates: Sender<HueEventRecord>,
    buffer: VecDeque<HueEventRecord>,
}

impl HueEventStream {
    #[must_use]
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            timestamp: Utc::now(),
            index: 0,
            hue_updates: Sender::new(32),
            buffer: VecDeque::with_capacity(buffer_capacity),
        }
    }

    fn add_to_buffer(&mut self, record: HueEventRecord) {
        if self.buffer.len() == self.buffer.capacity() {
            self.buffer.pop_front();
            self.buffer.push_back(record);
            debug_assert!(self.buffer.len() == self.buffer.capacity());
        } else {
            self.buffer.push_back(record);
        }
    }

    fn generate_record(&mut self, block: EventBlock) -> HueEventRecord {
        let timestamp = Utc::now();
        if timestamp.timestamp() == self.timestamp.timestamp() {
            self.index += 1;
        } else {
            self.index = 0;
            self.timestamp = timestamp;
        }
        HueEventRecord {
            block,
            timestamp,
            index: self.index,
        }
    }

    #[must_use]
    pub fn events_sent_after_id(&self, id: &str) -> Vec<HueEventRecord> {
        let Some(parsed_id) = parse_id(id) else {
            return vec![];
        };

        self.buffer
            .iter()
            .filter(|record| record.sortable_id() > parsed_id)
            .cloned()
            .collect()
    }

    pub fn hue_event(&mut self, block: EventBlock) {
        let record = self.generate_record(block);
        self.add_to_buffer(record.clone());
        if let Err(err) = self.hue_updates.send(record) {
            log::trace!("Overflow on hue event pipe: {err}");
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> Receiver<HueEventRecord> {
        self.hue_updates.subscribe()
    }
}

fn parse_id(id: &str) -> Option<(i64, u32)> {
    let id_parts: Vec<_> = id.split(':').collect();
    if id_parts.len() < 2 {
        return None;
    }
    let timestamp: i64 = id_parts[0].parse().ok()?;
    let index: u32 = id_parts[1].parse().ok()?;
    Some((timestamp, index))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use hue::event::EventBlock;

    use crate::server::hueevents::{HueEventRecord, HueEventStream};

    #[test]
    fn no_events() {
        let event_steam = HueEventStream::new(10);

        assert_eq!(event_steam.events_sent_after_id("1780136225:0").len(), 0);
    }

    #[test]
    fn last_event_is_before_all_events() {
        let mut event_steam = HueEventStream::new(10);
        let now = Utc::now();

        event_steam.add_to_buffer(create_record(now, 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(2), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(4), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(7), 0));

        assert_eq!(
            event_steam
                .events_sent_after_id(&format!("{}:0", (now - TimeDelta::minutes(5)).timestamp()))
                .len(),
            4
        );
    }

    #[test]
    fn no_new_events() {
        let mut event_steam = HueEventStream::new(10);
        let now = Utc::now();

        event_steam.add_to_buffer(create_record(now, 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::minutes(2), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::minutes(4), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::minutes(7), 0));

        assert_eq!(
            event_steam
                .events_sent_after_id(&format!("{}:0", (now + TimeDelta::minutes(10)).timestamp()))
                .len(),
            0
        );
    }

    #[test]
    fn new_events_since_last_event() {
        let mut event_steam = HueEventStream::new(10);
        let now = Utc::now();

        event_steam.add_to_buffer(create_record(now, 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(2), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(4), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(7), 0));

        assert_eq!(
            event_steam
                .events_sent_after_id(&format!("{}:0", (now + TimeDelta::seconds(2)).timestamp()))
                .len(),
            2
        );
    }

    #[test]
    fn id_0() {
        // This is sent by the Hue Sync box 4k when (re)connecting
        let mut event_steam = HueEventStream::new(10);
        let now = Utc::now();

        event_steam.add_to_buffer(create_record(now, 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(2), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(4), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(7), 0));

        assert_eq!(event_steam.events_sent_after_id("0").len(), 0);
    }

    #[test]
    fn timestamp_index() {
        // This is sent by the Hue Sync box 4k when (re)connecting
        let mut event_steam = HueEventStream::new(10);
        let now = Utc::now();

        event_steam.add_to_buffer(create_record(now, 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(2), 0));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(2), 1));
        event_steam.add_to_buffer(create_record(now + TimeDelta::seconds(3), 0));

        assert_eq!(
            event_steam
                .events_sent_after_id(&format!("{}:1", (now + TimeDelta::seconds(2)).timestamp()))
                .len(),
            1
        );
    }

    fn create_record(now: DateTime<Utc>, index: u32) -> HueEventRecord {
        HueEventRecord {
            timestamp: now,
            index,
            block: EventBlock::add(vec![]),
        }
    }
}
