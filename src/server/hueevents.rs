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
        let mut events = self.buffer.iter().skip_while(|record| record.id() != id);
        match events.next() {
            Some(_) => events.cloned().collect(),
            // return all events if requested event is not in buffer
            None => self.buffer.iter().cloned().collect(),
        }
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
