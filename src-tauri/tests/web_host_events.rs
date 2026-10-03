use serde_json::json;
use video_creater_lib::web_host::event_hub::{EventHub, ResumeResult};

#[test]
fn event_hub_orders_events_and_requires_snapshot_after_retention_gap() {
    let hub = EventHub::new(3, 2);
    for number in 1..=4 {
        let event = hub.publish("job.progress", Some("project-1"), json!({"number": number}));
        assert_eq!(event.sequence, number);
    }

    assert_eq!(
        hub.resume_after(0),
        ResumeResult::SnapshotRequired {
            oldest_sequence: 2,
            latest_sequence: 4,
        }
    );
    let ResumeResult::Events(events) = hub.resume_after(2) else {
        panic!("retained resume must return events")
    };
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![3, 4]
    );
}

#[test]
fn slow_subscriber_is_disconnected_without_blocking_publishers() {
    let hub = EventHub::new(8, 1);
    let subscriber = hub.subscribe(0).expect("subscriber");
    hub.publish("one", None, json!({}));
    hub.publish("two", None, json!({}));
    assert_eq!(hub.subscriber_count(), 0);
    assert_eq!(subscriber.recv().unwrap().topic, "one");
}

#[test]
fn subscription_replays_retained_events_before_live_events() {
    let hub = EventHub::new(8, 2);
    hub.publish("before.one", None, json!({}));
    hub.publish("before.two", None, json!({}));

    let subscriber = hub.subscribe(0).expect("subscriber");
    hub.publish("after", None, json!({}));

    assert_eq!(subscriber.recv().unwrap().topic, "before.one");
    assert_eq!(subscriber.recv().unwrap().topic, "before.two");
    assert_eq!(subscriber.recv().unwrap().topic, "after");
}

#[test]
fn dropping_an_idle_subscription_unregisters_without_another_publish() {
    let hub = EventHub::new(8, 2);
    for _ in 0..100 {
        let subscriber = hub.subscribe(0).unwrap();
        assert_eq!(hub.subscriber_count(), 1);
        drop(subscriber);
        assert_eq!(hub.subscriber_count(), 0);
    }
}

#[tokio::test]
async fn async_subscriptions_replay_ordered_events_and_evict_slow_consumers() {
    let hub = EventHub::new(8, 1);
    hub.publish("retained", None, json!({}));
    let mut subscriber = hub.subscribe_async(0).unwrap();
    hub.publish("live", None, json!({}));
    hub.publish("overflow", None, json!({}));
    assert_eq!(hub.subscriber_count(), 0);
    assert_eq!(subscriber.recv().await.unwrap().topic, "retained");
    assert_eq!(subscriber.recv().await.unwrap().topic, "live");
    assert!(subscriber.recv().await.is_none());
}

#[tokio::test]
async fn cancelling_an_idle_async_consumer_unregisters_without_a_publish() {
    let hub = EventHub::new(8, 2);
    let mut subscriber = hub.subscribe_async(0).unwrap();
    let consumer = tokio::spawn(async move { subscriber.recv().await });
    assert_eq!(hub.subscriber_count(), 1);
    consumer.abort();
    assert!(consumer.await.unwrap_err().is_cancelled());
    assert_eq!(hub.subscriber_count(), 0);
}

#[test]
fn bounded_resume_window_is_monotonic_for_every_sequence() {
    let hub = EventHub::new(32, 4);
    for number in 0..256 {
        hub.publish("event", None, json!({"number":number}));
    }
    for after in 0..=300_u64 {
        match hub.resume_after(after) {
            ResumeResult::Events(events) => {
                assert!(events.iter().all(|event| event.sequence > after));
                assert!(events
                    .windows(2)
                    .all(|pair| pair[0].sequence < pair[1].sequence));
            }
            ResumeResult::SnapshotRequired {
                oldest_sequence,
                latest_sequence,
            } => {
                assert!(oldest_sequence > after.saturating_add(1));
                assert!(latest_sequence >= oldest_sequence);
            }
        }
    }
}
