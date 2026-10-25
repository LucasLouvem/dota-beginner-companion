//! Testes unitários de `src/presentation/app.rs`.

use super::*;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// Tempo máximo para o laço terminar depois que o canal fecha.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Inicia o laço numa tarefa e devolve os avisos que ele emite.
fn spawn_forward(
    state: watch::Receiver<AppSnapshot>,
    pending: Arc<PendingUpdate>,
) -> UnboundedReceiver<CoreUpdate> {
    let (update_sender, update_receiver) = unbounded_channel();
    tokio::spawn(forward_snapshots(state, pending, move |update| {
        update_sender.send(update).unwrap();
    }));
    update_receiver
}

async fn next_update(updates: &mut UnboundedReceiver<CoreUpdate>) -> Option<CoreUpdate> {
    tokio::time::timeout(FORWARD_TIMEOUT, updates.recv())
        .await
        .expect("o laço deve emitir um aviso")
}

#[tokio::test]
async fn notifies_latest_state_and_reports_stop_when_state_channel_closes() {
    let (state_sender, state) = watch::channel(AppSnapshot::default());
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 3);
    drop(state_sender);
    let mut updates = Vec::new();

    tokio::time::timeout(
        FORWARD_TIMEOUT,
        forward_snapshots(state, Arc::default(), |update| updates.push(update)),
    )
    .await
    .expect("o laço deve terminar quando o canal fecha");

    assert_eq!(updates, vec![CoreUpdate::Changed, CoreUpdate::Stopped]);
}

#[tokio::test]
async fn many_changes_before_the_window_reads_produce_a_single_notice() {
    let (state_sender, state) = watch::channel(AppSnapshot::default());
    let pending = Arc::new(PendingUpdate::default());
    let mut updates = spawn_forward(state, Arc::clone(&pending));

    assert_eq!(next_update(&mut updates).await, Some(CoreUpdate::Changed));
    for event_count in 1..=50 {
        state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = event_count);
    }
    drop(state_sender);

    // Sem `release`, as 50 mudanças não geram outro aviso; só o fechamento.
    assert_eq!(next_update(&mut updates).await, Some(CoreUpdate::Stopped));
    assert_eq!(next_update(&mut updates).await, None);
}

#[tokio::test]
async fn notifies_again_after_the_window_releases_the_pending_notice() {
    let (state_sender, state) = watch::channel(AppSnapshot::default());
    let pending = Arc::new(PendingUpdate::default());
    let mut updates = spawn_forward(state, Arc::clone(&pending));
    assert_eq!(next_update(&mut updates).await, Some(CoreUpdate::Changed));

    pending.release();
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 1);

    assert_eq!(next_update(&mut updates).await, Some(CoreUpdate::Changed));
}

#[test]
fn pending_update_is_marked_once_until_released() {
    let pending = PendingUpdate::default();
    assert!(pending.mark());
    assert!(!pending.mark());
    pending.release();
    assert!(pending.mark());
}

#[test]
fn reading_releases_the_notice_and_returns_the_unread_state() {
    let (state_sender, mut state) = watch::channel(AppSnapshot::default());
    let pending = PendingUpdate::default();
    assert!(pending.mark());
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 7);

    let snapshot = read_new_snapshot(&mut state, &pending);

    assert_eq!(snapshot.map(|snapshot| snapshot.gsi.dota_events), Some(7));
    assert!(pending.mark(), "o aviso foi liberado");
}

#[test]
fn late_notice_for_an_already_read_state_returns_nothing() {
    let (state_sender, mut state) = watch::channel(AppSnapshot::default());
    let pending = PendingUpdate::default();
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 1);
    assert!(read_new_snapshot(&mut state, &pending).is_some());

    assert!(read_new_snapshot(&mut state, &pending).is_none());
}

#[test]
fn state_published_before_the_core_stopped_is_still_read() {
    let (state_sender, mut state) = watch::channel(AppSnapshot::default());
    state_sender.send_modify(|snapshot| snapshot.gsi.dota_events = 2);
    drop(state_sender);

    let snapshot = read_new_snapshot(&mut state, &PendingUpdate::default());

    assert_eq!(snapshot.map(|snapshot| snapshot.gsi.dota_events), Some(2));
}
