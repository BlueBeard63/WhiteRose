//! A single debounced writer: serialization and disk writes never block input,
//! and an older snapshot can never finish after a newer clear/delete.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::clipboard::{self, History};

enum Command {
    Save(PathBuf, History),
    Flush(Sender<()>),
}

pub struct Saver {
    tx: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl Default for Saver {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx: Some(tx), worker: Some(std::thread::spawn(move || run(rx))) }
    }
}

impl Saver {
    pub fn save(&self, dir: PathBuf, history: History) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Save(dir, history));
        }
    }

    /// Waits for queued writes (tests/shutdown only, never the input loop).
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.as_ref().is_some_and(|sender| sender.send(Command::Flush(tx)).is_ok()) {
            let _ = rx.recv();
        }
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn write(pending: &mut Option<(PathBuf, History)>) {
    if let Some((dir, history)) = pending.take() {
        if let Err(e) = clipboard::save(&dir, &history) {
            tracing::warn!(%e, "saving clipboard history failed");
        }
    }
}

fn run(rx: Receiver<Command>) {
    let mut pending: Option<(PathBuf, History)> = None;
    let mut deadline = Instant::now();
    loop {
        let command = if pending.is_some() {
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match command {
            Ok(Command::Save(dir, history)) => {
                if pending.as_ref().is_some_and(|(old, _)| *old != dir) {
                    write(&mut pending);
                }
                if pending.is_none() {
                    deadline = Instant::now() + Duration::from_millis(100);
                }
                pending = Some((dir, history));
            }
            Ok(Command::Flush(done)) => {
                write(&mut pending);
                let _ = done.send(());
            }
            Err(RecvTimeoutError::Timeout) => write(&mut pending),
            Err(RecvTimeoutError::Disconnected) => {
                write(&mut pending);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{clipboard::Content, scratch::Scratch};

    #[test]
    fn bursts_keep_the_latest_snapshot_including_clear() {
        let dir = Scratch::new("clipboard-save");
        let saver = Saver::default();
        let mut history = History::default();
        for n in 0..100 {
            history.add(Content::Text { text: format!("copy {n}") }, n);
            saver.save(dir.to_path_buf(), history.clone());
        }
        saver.flush();
        assert_eq!(clipboard::load(&dir), history);
        history.clear();
        saver.save(dir.to_path_buf(), history.clone());
        saver.flush();
        assert!(clipboard::load(&dir).entries.is_empty());
    }

    #[test]
    fn shutdown_flushes_the_last_copy() {
        let dir = Scratch::new("clipboard-shutdown");
        let mut history = History::default();
        history.add(Content::Text { text: "last copy".into() }, 1);
        {
            let saver = Saver::default();
            saver.save(dir.to_path_buf(), history.clone());
        }
        assert_eq!(clipboard::load(&dir), history);
    }
}
