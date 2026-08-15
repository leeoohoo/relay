use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Utc};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use ai_chat_domain::company::CompanyRealtimeSignal;
use ai_chat_shared::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct QueueChecks {
    pub(super) agents: bool,
    pub(super) plugins: bool,
    pub(super) control: bool,
}

impl QueueChecks {
    pub(super) const fn all() -> Self {
        Self {
            agents: true,
            plugins: true,
            control: true,
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.agents |= other.agents;
        self.plugins |= other.plugins;
        self.control |= other.control;
    }

    pub(super) const fn allows_blocking_maintenance(
        self,
        agent_runs_idle: bool,
        plugin_operations_idle: bool,
    ) -> bool {
        agent_runs_idle && plugin_operations_idle && !self.agents && !self.plugins
    }

    fn is_empty(self) -> bool {
        !self.agents && !self.plugins && !self.control
    }
}

pub(super) async fn next_control_wake(watcher: &mut Option<ControlFileWatcher>) -> QueueChecks {
    match watcher {
        Some(watcher) => watcher.next().await,
        None => std::future::pending().await,
    }
}

#[derive(Default)]
pub(super) struct RealtimeWakeFilter {
    last_sequence_by_company: HashMap<Uuid, i64>,
}

impl RealtimeWakeFilter {
    fn classify(&mut self, signal: CompanyRealtimeSignal) -> QueueChecks {
        let last_sequence = self
            .last_sequence_by_company
            .entry(signal.company_id)
            .or_default();
        if signal.sequence_id <= *last_sequence {
            return QueueChecks::default();
        }
        *last_sequence = signal.sequence_id;
        match signal.event_type.as_deref() {
            Some("codex.trigger.updated") => QueueChecks {
                agents: true,
                ..QueueChecks::default()
            },
            Some("codex.plugin.operation.updated") => QueueChecks {
                plugins: true,
                ..QueueChecks::default()
            },
            Some(_) => QueueChecks::default(),
            None => QueueChecks {
                agents: true,
                plugins: true,
                control: false,
            },
        }
    }
}

pub(super) async fn next_realtime_wake(
    receiver: &mut broadcast::Receiver<CompanyRealtimeSignal>,
    filter: &mut RealtimeWakeFilter,
) -> QueueChecks {
    let mut ignored = 0usize;
    loop {
        match receiver.recv().await {
            Ok(signal) => {
                let checks = filter.classify(signal);
                if !checks.is_empty() {
                    return checks;
                }
                ignored += 1;
                if ignored >= 64 {
                    ignored = 0;
                    tokio::task::yield_now().await;
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(
                    skipped,
                    "Trigger realtime listener lagged; reconciling queues"
                );
                return QueueChecks::all();
            }
            Err(broadcast::error::RecvError::Closed) => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                return QueueChecks::all();
            }
        }
    }
}

pub(super) struct ControlFileWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::UnboundedReceiver<QueueChecks>,
}

impl ControlFileWatcher {
    pub(super) fn start(control_root: &Path) -> AppResult<Self> {
        let watched_root = std::fs::canonicalize(control_root).map_err(|error| {
            AppError::Internal(format!("cannot resolve Codex control directory: {error}"))
        })?;
        let requests_dir = watched_root.join("requests");
        let preferences_path = watched_root.join("trigger-preferences.json");
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| match result {
                Ok(event) => {
                    let checks =
                        classify_control_paths(&event.paths, &requests_dir, &preferences_path);
                    if !checks.is_empty() {
                        let _ = sender.send(checks);
                    }
                }
                Err(error) => tracing::warn!(%error, "Codex control file watcher failed"),
            },
            Config::default(),
        )
        .map_err(file_watch_error)?;
        watcher
            .watch(&watched_root, RecursiveMode::Recursive)
            .map_err(file_watch_error)?;
        Ok(Self {
            _watcher: watcher,
            receiver,
        })
    }

    pub(super) async fn next(&mut self) -> QueueChecks {
        let Some(first) = self.receiver.recv().await else {
            tokio::time::sleep(Duration::from_secs(5)).await;
            return QueueChecks::all();
        };
        let mut combined = first;
        while let Ok(next) = self.receiver.try_recv() {
            combined.merge(next);
        }
        combined
    }
}

fn classify_control_paths(
    paths: &[PathBuf],
    requests_dir: &Path,
    preferences_path: &Path,
) -> QueueChecks {
    let mut checks = QueueChecks::default();
    for path in paths {
        if path.parent() == Some(requests_dir)
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            checks.control = true;
        }
        if path == preferences_path {
            checks.agents = true;
        }
    }
    checks
}

fn file_watch_error(error: notify::Error) -> AppError {
    AppError::Internal(format!("cannot watch Codex control files: {error}"))
}

pub(super) fn scheduled_instant(due_at: Option<DateTime<Utc>>) -> tokio::time::Instant {
    let delay = due_at
        .and_then(|due_at| (due_at - Utc::now()).to_std().ok())
        .unwrap_or(Duration::ZERO)
        .max(Duration::from_millis(100));
    tokio::time::Instant::now() + delay
}

pub(super) fn jittered_fallback_interval(base: Duration, identity: &str) -> Duration {
    let mut hasher = DefaultHasher::new();
    identity.hash(&mut hasher);
    let percent = 90 + hasher.finish() % 21;
    base.mul_f64(percent as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(sequence_id: i64, event_type: Option<&str>) -> CompanyRealtimeSignal {
        CompanyRealtimeSignal {
            sequence_id,
            company_id: Uuid::nil(),
            event_type: event_type.map(str::to_string),
            aggregate_type: None,
            aggregate_id: None,
        }
    }

    #[test]
    fn realtime_filter_only_wakes_relevant_queues() {
        let mut filter = RealtimeWakeFilter::default();
        assert!(
            filter
                .classify(signal(1, Some("codex.trigger.updated")))
                .agents
        );
        assert!(
            filter
                .classify(signal(2, Some("codex.plugin.operation.updated")))
                .plugins
        );
        assert!(filter
            .classify(signal(3, Some("codex.run.updated")))
            .is_empty());
    }

    #[test]
    fn blocking_maintenance_yields_to_execution_queues() {
        assert!(QueueChecks::default().allows_blocking_maintenance(true, true));
        assert!(!QueueChecks::all().allows_blocking_maintenance(true, true));
        assert!(!QueueChecks::default().allows_blocking_maintenance(false, true));
        assert!(!QueueChecks::default().allows_blocking_maintenance(true, false));
    }

    #[test]
    fn realtime_filter_deduplicates_company_sequences() {
        let mut filter = RealtimeWakeFilter::default();
        assert!(filter.classify(signal(5, None)).agents);
        assert!(filter.classify(signal(5, None)).is_empty());
        assert!(filter.classify(signal(4, None)).is_empty());
    }

    #[test]
    fn control_paths_ignore_runtime_status_writes() {
        let root = Path::new("control");
        assert_eq!(
            classify_control_paths(
                &[root.join("requests/1.json")],
                &root.join("requests"),
                &root.join("trigger-preferences.json")
            ),
            QueueChecks {
                control: true,
                ..QueueChecks::default()
            }
        );
        assert!(classify_control_paths(
            &[root.join("runtime.json")],
            &root.join("requests"),
            &root.join("trigger-preferences.json")
        )
        .is_empty());
        assert!(classify_control_paths(
            &[root.join("requests")],
            &root.join("requests"),
            &root.join("trigger-preferences.json")
        )
        .is_empty());
        assert!(classify_control_paths(
            &[root.join("requests/.request.json.tmp")],
            &root.join("requests"),
            &root.join("trigger-preferences.json")
        )
        .is_empty());
        assert!(classify_control_paths(
            &[root.join("requests/nested/request.json")],
            &root.join("requests"),
            &root.join("trigger-preferences.json")
        )
        .is_empty());
    }

    #[tokio::test]
    async fn control_watcher_receives_new_request_files() {
        let root = std::env::temp_dir().join(format!("relay-control-watch-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("requests")).expect("request directory");
        let mut watcher = Some(ControlFileWatcher::start(&root).expect("watcher"));
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::fs::write(root.join("requests/request.json"), b"{}").expect("request file");
        let checks = tokio::time::timeout(Duration::from_secs(5), next_control_wake(&mut watcher))
            .await
            .expect("file event");
        let _ = std::fs::remove_dir_all(root);
        assert!(checks.control);
    }
}
