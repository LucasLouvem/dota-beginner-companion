//! Testes unitários de `src/presentation/app.rs`.

use super::*;
use std::time::Duration;

/// Tempo máximo para o laço terminar depois que o canal fecha.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn forwards_latest_snapshot_and_reports_stop_when_state_channel_closes() {
    let (state_sender, state) = watch::channel(AppSnapshot::default());
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 3);
    drop(state_sender);
    let mut updates = Vec::new();

    tokio::time::timeout(
        FORWARD_TIMEOUT,
        forward_snapshots(state, |update| updates.push(update)),
    )
    .await
    .expect("o laço deve terminar quando o canal fecha");

    assert_eq!(updates.len(), 2, "{updates:?}");
    assert!(
        matches!(&updates[0], CoreUpdate::Snapshot(snapshot) if snapshot.gsi.dota_events == 3),
        "{updates:?}"
    );
    assert!(matches!(updates[1], CoreUpdate::Stopped), "{updates:?}");
}

#[tokio::test]
async fn forwards_each_new_snapshot_while_core_is_running() {
    let (state_sender, state) = watch::channel(AppSnapshot::default());
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(forward_snapshots(state, move |update| {
        update_sender.send(update).unwrap();
    }));

    let first = tokio::time::timeout(FORWARD_TIMEOUT, update_receiver.recv())
        .await
        .unwrap();
    assert!(
        matches!(&first, Some(CoreUpdate::Snapshot(snapshot)) if snapshot.gsi.dota_events == 0),
        "{first:?}"
    );

    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 1);

    let second = tokio::time::timeout(FORWARD_TIMEOUT, update_receiver.recv())
        .await
        .unwrap();
    assert!(
        matches!(&second, Some(CoreUpdate::Snapshot(snapshot)) if snapshot.gsi.dota_events == 1),
        "{second:?}"
    );
}
