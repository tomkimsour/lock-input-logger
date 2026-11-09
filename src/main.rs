use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use evdev::Device;
use futures_util::StreamExt;
use lock_input_logger::{BurstAggregator, Category, classify, pick_session, target_user};
use logind_zbus::manager::ManagerProxy;
use logind_zbus::session::SessionProxy;
use tokio::sync::mpsc;

enum Msg {
    Activity(String, Category),
    Removed(PathBuf, String),
    Lock(bool),
}

/// Current local wall-clock time as `HH:MM:SS`.
fn now_label() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// Whether a device can produce key, button or pointer input.
fn is_interesting(device: &Device) -> bool {
    device.supported_keys().is_some()
        || device.supported_relative_axes().is_some()
        || device.supported_absolute_axes().is_some()
}

/// Streams a device's events, forwarding only coarse categories while the session is locked.
fn spawn_reader(path: PathBuf, device: Device, locked: Arc<AtomicBool>, tx: mpsc::Sender<Msg>) {
    let name = device.name().unwrap_or("unknown device").to_owned();
    let mut stream = match device.into_event_stream() {
        Ok(stream) => stream,
        Err(e) => {
            println!("cannot read {}: {e}", path.display());
            return;
        }
    };
    println!("device added: '{name}' ({})", path.display());
    tokio::spawn(async move {
        loop {
            match stream.next_event().await {
                Ok(event) => {
                    if !locked.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let Some(category) = classify(&event)
                        && tx
                            .send(Msg::Activity(name.clone(), category))
                            .await
                            .is_err()
                    {
                        return;
                    }
                }
                Err(_) => {
                    let _ = tx.send(Msg::Removed(path, name)).await;
                    return;
                }
            }
        }
    });
}

/// Opens readers for input devices that appeared since the last scan.
fn scan_devices(known: &mut HashSet<PathBuf>, locked: &Arc<AtomicBool>, tx: &mpsc::Sender<Msg>) {
    for (path, device) in evdev::enumerate() {
        if known.contains(&path) || !is_interesting(&device) {
            continue;
        }
        known.insert(path.clone());
        spawn_reader(path, device, locked.clone(), tx.clone());
    }
}

/// Follows the LockedHint of `user`'s seat0 session until that session disappears.
async fn follow_session(user: &str, tx: &mpsc::Sender<Msg>) -> zbus::Result<()> {
    let conn = zbus::Connection::system().await?;
    let manager = ManagerProxy::new(&conn).await?;
    let sessions: Vec<_> = manager
        .list_sessions()
        .await?
        .iter()
        .map(|s| (s.sid().to_owned(), s.user().to_owned(), s.seat().to_owned()))
        .collect();
    let sid = pick_session(&sessions, user)
        .ok_or_else(|| zbus::Error::Failure(format!("no seat0 session for {user}")))?;
    let path = manager.get_session(&sid).await?;
    let session = SessionProxy::builder(&conn).path(path)?.build().await?;
    println!("watching logind session {sid} of {user}");

    let mut removed = manager.receive_session_removed().await?;
    let mut changes = session.receive_locked_hint_changed().await;
    let _ = tx.send(Msg::Lock(session.locked_hint().await?)).await;
    loop {
        tokio::select! {
            Some(change) = changes.next() => {
                let _ = tx.send(Msg::Lock(change.get().await?)).await;
            }
            Some(signal) = removed.next() => {
                if *signal.args()?.session_id() == sid {
                    let _ = tx.send(Msg::Lock(false)).await;
                    return Err(zbus::Error::Failure(format!("session {sid} ended")));
                }
            }
            else => return Err(zbus::Error::Failure("logind streams closed".into())),
        }
    }
}

/// Keeps following the user's session across logouts, logind restarts and boots.
async fn watch_lock(user: String, tx: mpsc::Sender<Msg>) {
    loop {
        if let Err(e) = follow_session(&user, &tx).await {
            println!("logind: {e}; retrying in 5s");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let user = target_user(std::env::var("LOCK_LOGGER_USER").ok()).unwrap_or_else(|| {
        eprintln!("LOCK_LOGGER_USER is not set");
        std::process::exit(2);
    });
    let locked = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel(1024);
    let mut known = HashSet::new();
    let mut aggregator = BurstAggregator::new(Duration::from_secs(3));
    let mut lock_state: Option<bool> = None;

    tokio::spawn(watch_lock(user, tx.clone()));
    scan_devices(&mut known, &locked, &tx);

    let mut flush_tick = tokio::time::interval(Duration::from_secs(1));
    let mut scan_tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => match msg {
                Msg::Activity(device, category) => {
                    aggregator.record(&device, category, Instant::now(), &now_label());
                }
                Msg::Removed(path, name) => {
                    known.remove(&path);
                    println!("device removed: '{name}' ({})", path.display());
                }
                Msg::Lock(now_locked) => {
                    locked.store(now_locked, Ordering::Relaxed);
                    if lock_state != Some(now_locked) {
                        if !now_locked {
                            for line in aggregator.flush(Instant::now(), true) {
                                println!("{line}");
                            }
                        }
                        println!("{} at {}", if now_locked { "LOCKED" } else { "UNLOCKED" }, now_label());
                        lock_state = Some(now_locked);
                    }
                }
            },
            _ = flush_tick.tick() => {
                for line in aggregator.flush(Instant::now(), false) {
                    println!("{line}");
                }
            }
            _ = scan_tick.tick() => scan_devices(&mut known, &locked, &tx),
        }
    }
}
