use std::collections::VecDeque;

use chrono::Utc;
use tokio::sync::broadcast::Sender;

use crate::hue::event::EventBlock;

#[derive(Clone, Debug)]
pub struct HueEventStream {
    prev_ts: i64,
    idx: i32,
    hue_updates: Sender<(String, EventBlock)>,
    buffer: VecDeque<(String, EventBlock)>,
}

impl HueEventStream {
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            prev_ts: Utc::now().timestamp(),
            idx: 0,
            hue_updates: Sender::new(32),
            buffer: VecDeque::with_capacity(buffer_capacity),
        }
    }

    pub fn add_to_buffer(&mut self, id: String, evt: EventBlock) {
        if self.buffer.len() == self.buffer.capacity() {
            self.buffer.pop_front();
            self.buffer.push_back((id, evt));
            debug_assert!(self.buffer.len() == self.buffer.capacity());
        } else {
            self.buffer.push_back((id, evt));
        }
    }

    pub fn events_sent_after_id(&self, id: &str) -> Vec<(String, EventBlock)> {
        let mut events = self.buffer.iter().skip_while(|(evt_id, _)| evt_id != id);
        match events.next() {
            Some(_) => events.cloned().collect(),
            // return all events if requested event is not in buffer
            None => self.buffer.iter().cloned().collect(),
        }
    }

    pub fn hue_event(&mut self, evt: EventBlock) {
        let id = self.generate_event_id();
        self.add_to_buffer(id.clone(), evt.clone());
        if let Err(err) = self.hue_updates.send((id, evt)) {
            log::trace!("Overflow on hue event pipe: {err}");
        }
    }

    fn generate_event_id(&mut self) -> String {
        let ts = Utc::now().timestamp();
        if ts == self.prev_ts {
            self.idx += 1;
        } else {
            self.idx = 0;
            self.prev_ts = ts;
        }
        format!("{}:{}", ts, self.idx)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<(String, EventBlock)> {
        self.hue_updates.subscribe()
    }
}
