use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Mutex;

use crate::app_service::error::ServiceError;
use crate::app_service::events::{EventEnvelope, EventSink, ServiceEvent};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum ResumeResult {
    Events(Vec<EventEnvelope>),
    SnapshotRequired {
        oldest_sequence: u64,
        latest_sequence: u64,
    },
}

struct State {
    next_sequence: u64,
    retained: VecDeque<EventEnvelope>,
    subscribers: Vec<SyncSender<EventEnvelope>>,
}

pub struct EventHub {
    retention: usize,
    client_capacity: usize,
    state: Mutex<State>,
}

impl EventHub {
    pub fn new(retention: usize, client_capacity: usize) -> Self {
        Self {
            retention: retention.max(1),
            client_capacity: client_capacity.max(1),
            state: Mutex::new(State {
                next_sequence: 1,
                retained: VecDeque::new(),
                subscribers: Vec::new(),
            }),
        }
    }

    pub fn publish(
        &self,
        topic: impl Into<String>,
        project_id: Option<&str>,
        payload: Value,
    ) -> EventEnvelope {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let event = EventEnvelope {
            sequence: state.next_sequence,
            topic: topic.into(),
            project_id: project_id.map(str::to_owned),
            payload,
        };
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.retained.push_back(event.clone());
        while state.retained.len() > self.retention {
            state.retained.pop_front();
        }
        state
            .subscribers
            .retain(|subscriber| match subscriber.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            });
        event
    }

    pub fn resume_after(&self, sequence: u64) -> ResumeResult {
        resume(
            &self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            sequence,
        )
    }

    pub fn subscribe(&self, after_sequence: u64) -> Result<Receiver<EventEnvelope>, ResumeResult> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let events = match resume(&state, after_sequence) {
            ResumeResult::Events(events) => events,
            snapshot @ ResumeResult::SnapshotRequired { .. } => return Err(snapshot),
        };
        let (sender, receiver) =
            mpsc::sync_channel(self.client_capacity.saturating_add(events.len()));
        for event in events {
            sender
                .try_send(event)
                .expect("subscription channel is sized for retained events");
        }
        state.subscribers.push(sender);
        Ok(receiver)
    }

    pub fn subscriber_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .subscribers
            .len()
    }
}

fn resume(state: &State, sequence: u64) -> ResumeResult {
    let latest = state.next_sequence.saturating_sub(1);
    let oldest = state
        .retained
        .front()
        .map_or(state.next_sequence, |event| event.sequence);
    if latest > 0 && sequence.saturating_add(1) < oldest {
        return ResumeResult::SnapshotRequired {
            oldest_sequence: oldest,
            latest_sequence: latest,
        };
    }
    ResumeResult::Events(
        state
            .retained
            .iter()
            .filter(|event| event.sequence > sequence)
            .cloned()
            .collect(),
    )
}

impl EventSink for EventHub {
    fn publish(&self, event: ServiceEvent) -> Result<EventEnvelope, ServiceError> {
        Ok(EventHub::publish(
            self,
            event.topic,
            event.project_id.as_deref(),
            event.payload,
        ))
    }
}
