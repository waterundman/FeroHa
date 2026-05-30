use crate::cli::parser::CliCommand;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub interval_secs: u64,
    pub command: CliCommand,
    #[serde(skip)]
    pub last_run: Option<std::time::Instant>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CronJobStatus {
    pub id: String,
    pub interval_secs: u64,
    pub next_run_secs: u64,
    pub last_run_at: Option<u64>,
    pub enabled: bool,
}

pub struct TaskScheduler {
    jobs: Arc<Mutex<Vec<CronJob>>>,
    cancel_token: CancellationToken,
}

impl TaskScheduler {
    pub fn new(jobs: Vec<CronJob>) -> Self {
        TaskScheduler {
            jobs: Arc::new(Mutex::new(jobs)),
            cancel_token: CancellationToken::new(),
        }
    }

    pub fn start(&self, on_tick: Arc<dyn Fn() + Send + Sync + 'static>) {
        let cancel_token = self.cancel_token.clone();
        let jobs = self.jobs.lock().unwrap();
        for job in jobs.iter() {
            if !job.enabled {
                continue;
            }
            let interval = job.interval_secs;
            let cancel_token = cancel_token.clone();
            let cb = on_tick.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(interval));
                ticker.tick().await;
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => break,
                        _ = ticker.tick() => cb(),
                    }
                }
            });
        }
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
    }

    pub fn status(&self) -> Vec<CronJobStatus> {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter()
            .map(|j| CronJobStatus {
                id: j.id.clone(),
                interval_secs: j.interval_secs,
                next_run_secs: j.interval_secs,
                last_run_at: None,
                enabled: j.enabled,
            })
            .collect()
    }

    pub fn default_jobs() -> Vec<CronJob> {
        vec![CronJob {
            id: "dream-auto".to_string(),
            interval_secs: 6 * 3600,
            command: CliCommand::Dream,
            last_run: None,
            enabled: true,
        }]
    }
}
