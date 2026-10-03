use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};

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
    next_subscriber: u64,
    subscribers: HashMap<u64, Subscriber>,
}

enum Subscriber {
    Sync(SyncSender<EventEnvelope>),
    Async(tokio::sync::mpsc::Sender<EventEnvelope>),
}

struct SubscriptionRegistration {
    state: Weak<Mutex<State>>,
    id: u64,
}

impl Drop for SubscriptionRegistration {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .subscribers
                .remove(&self.id);
        }
    }
}

pub struct EventSubscription {
    receiver: Receiver<EventEnvelope>,
    _registration: SubscriptionRegistration,
}

impl EventSubscription {
    pub fn recv(&self) -> Result<EventEnvelope, mpsc::RecvError> {
        self.receiver.recv()
    }
}

pub struct AsyncEventSubscription {
    receiver: tokio::sync::mpsc::Receiver<EventEnvelope>,
    _registration: SubscriptionRegistration,
}

impl AsyncEventSubscription {
    pub async fn recv(&mut self) -> Option<EventEnvelope> {
        self.receiver.recv().await
    }
}

pub struct EventHub {
    retention: usize,
    client_capacity: usize,
    state: Arc<Mutex<State>>,
}

impl EventHub {
    pub fn new(retention: usize, client_capacity: usize) -> Self {
        Self {
            retention: retention.max(1),
            client_capacity: client_capacity.max(1),
            state: Arc::new(Mutex::new(State {
                next_sequence: 1,
                retained: VecDeque::new(),
                next_subscriber: 0,
                subscribers: HashMap::new(),
            })),
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
        state.subscribers.retain(|_, subscriber| match subscriber {
            Subscriber::Sync(sender) => match sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            },
            Subscriber::Async(sender) => sender.try_send(event.clone()).is_ok(),
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

    pub fn subscribe(&self, after_sequence: u64) -> Result<EventSubscription, ResumeResult> {
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
        let registration = self.register(&mut state, Subscriber::Sync(sender));
        Ok(EventSubscription {
            receiver,
            _registration: registration,
        })
    }

    pub fn subscribe_async(
        &self,
        after_sequence: u64,
    ) -> Result<AsyncEventSubscription, ResumeResult> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let events = match resume(&state, after_sequence) {
            ResumeResult::Events(events) => events,
            snapshot @ ResumeResult::SnapshotRequired { .. } => return Err(snapshot),
        };
        let (sender, receiver) =
            tokio::sync::mpsc::channel(self.client_capacity.saturating_add(events.len()));
        for event in events {
            sender
                .try_send(event)
                .expect("subscription channel is sized for retained events");
        }
        let registration = self.register(&mut state, Subscriber::Async(sender));
        Ok(AsyncEventSubscription {
            receiver,
            _registration: registration,
        })
    }

    fn register(&self, state: &mut State, subscriber: Subscriber) -> SubscriptionRegistration {
        let id = state.next_subscriber;
        state.next_subscriber += 1;
        state.subscribers.insert(id, subscriber);
        SubscriptionRegistration {
            state: Arc::downgrade(&self.state),
            id,
        }
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
