//! Keep source reads, disk writes and bounded Herdr probes off the input thread.
use crate::{
    herdr::{self, Binding},
    live::{Feed, Snapshot},
};
use anyhow::{Result, anyhow};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Update {
    pub snapshot: Snapshot,
    pub connection: Option<String>,
    pub error: Option<String>,
    pub storage_error: Option<String>,
}

pub struct Runner {
    pub updates: Receiver<Update>,
    cancel: Arc<AtomicBool>,
    wake: Sender<()>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl Runner {
    pub fn start(
        mut feed: Feed,
        cache: PathBuf,
        binding: Option<Binding>,
        project: Option<crate::project::Project>,
        initial: Snapshot,
    ) -> Self {
        let (tx, updates) = mpsc::sync_channel(1);
        let (wake, wake_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        feed.set_cancel(stop.clone());
        let handle = thread::spawn(move || {
            let mut check_at = Instant::now() - Duration::from_secs(2);
            let mut saved_at = Instant::now() - Duration::from_secs(2);
            let mut connection = None;
            let mut disconnected = false;
            let mut error = None;
            let mut storage_error = None;
            let mut dirty = true;
            let mut projected = initial;
            while !stop.load(Ordering::Relaxed) {
                if let Some(binding) = &binding
                    && check_at.elapsed() >= Duration::from_secs(2)
                {
                    match herdr::check_cancellable(binding, Some(&stop)) {
                        Ok(state) => {
                            connection = Some(state);
                            disconnected = false;
                        }
                        Err(_) => {
                            disconnected = true;
                            error = Some("에이전트 연결 끊김 · 마지막 계획 보존".into());
                        }
                    }
                    check_at = Instant::now();
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if !disconnected {
                    let position = feed.position();
                    match feed.refresh() {
                        Ok(changed) => {
                            dirty |= changed;
                            error = None;
                        }
                        Err(_) => error = Some("세션 읽기 실패 · 마지막 정상 계획 보존".into()),
                    }
                    dirty |= feed.position() != position;
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if dirty
                    && (storage_error.is_none() || saved_at.elapsed() >= Duration::from_secs(1))
                {
                    match feed.save_checkpoint(&cache) {
                        Ok(()) => {
                            dirty = false;
                            storage_error = None;
                        }
                        Err(_) => storage_error = Some("진행 기록 저장 실패 · 재시도 중".into()),
                    }
                    saved_at = Instant::now();
                }
                if let Some(project) = &project {
                    match project.project(&feed.snapshot) {
                        Ok(snapshot) => projected = snapshot,
                        Err(_) => {
                            error = Some("제품 계획 연결 오류 · 마지막 제품 진행 유지".into())
                        }
                    }
                } else {
                    projected = feed.snapshot.clone();
                }
                let update = Update {
                    snapshot: projected.clone(),
                    connection: connection.clone(),
                    error: error.clone(),
                    storage_error: storage_error.clone(),
                };
                if matches!(
                    tx.try_send(update),
                    Err(mpsc::TrySendError::Disconnected(_))
                ) {
                    break;
                }
                if wake_rx.recv_timeout(Duration::from_millis(300)).is_ok() {
                    break;
                }
            }
            if dirty {
                feed.save_checkpoint(&cache)?;
            }
            Ok(())
        });
        Self {
            updates,
            cancel,
            wake,
            handle: Some(handle),
        }
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow!("progress reader stopped unexpectedly"))??;
        }
        Ok(())
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
