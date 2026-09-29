use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::ServiceError;

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceEvent {
    pub topic: String,
    pub project_id: Option<String>,
    pub payload: Value,
}

impl ServiceEvent {
    pub fn new(topic: impl Into<String>, project_id: Option<&str>, payload: Value) -> Self {
        Self {
            topic: topic.into(),
            project_id: project_id.map(str::to_owned),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub sequence: u64,
    pub topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub payload: Value,
}

pub trait EventSink: Send + Sync {
    fn publish(&self, event: ServiceEvent) -> Result<EventEnvelope, ServiceError>;
}

#[derive(Debug, Default)]
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn publish(&self, event: ServiceEvent) -> Result<EventEnvelope, ServiceError> {
        Ok(EventEnvelope {
            sequence: 0,
            topic: event.topic,
            project_id: event.project_id,
            payload: event.payload,
        })
    }
}

#[derive(Debug, Default)]
pub struct FakeEventSink {
    recorded: Mutex<Vec<EventEnvelope>>,
}

impl FakeEventSink {
    pub fn recorded(&self) -> Vec<EventEnvelope> {
        self.recorded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl EventSink for FakeEventSink {
    fn publish(&self, event: ServiceEvent) -> Result<EventEnvelope, ServiceError> {
        let mut recorded = self
            .recorded
            .lock()
            .map_err(|_| ServiceError::internal("event sink lock poisoned"))?;
        let envelope = EventEnvelope {
            sequence: recorded.len() as u64 + 1,
            topic: event.topic,
            project_id: event.project_id,
            payload: event.payload,
        };
        recorded.push(envelope.clone());
        Ok(envelope)
    }
}
