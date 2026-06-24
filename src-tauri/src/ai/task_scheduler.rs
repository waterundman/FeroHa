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
    pub last_run: Option<std::time::SystemTime>,
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
            let job_id = job.id.clone();
            let cancel_token = cancel_token.clone();
            let cb = on_tick.clone();
            let jobs = self.jobs.clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(interval));
                ticker.tick().await;
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => break,
                        _ = ticker.tick() => {
                            mark_job_run(&jobs, &job_id);
                            cb();
                        },
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
                next_run_secs: next_run_secs(j),
                last_run_at: j
                    .last_run
                    .and_then(|last_run| last_run.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs()),
                enabled: j.enabled,
            })
            .collect()
    }

    #[cfg(test)]
    fn mark_job_run_for_test(&self, job_id: &str) {
        mark_job_run(&self.jobs, job_id);
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

fn mark_job_run(jobs: &Arc<Mutex<Vec<CronJob>>>, job_id: &str) {
    if let Ok(mut jobs) = jobs.lock() {
        if let Some(job) = jobs.iter_mut().find(|job| job.id == job_id) {
            job.last_run = Some(std::time::SystemTime::now());
        }
    }
}

fn next_run_secs(job: &CronJob) -> u64 {
    let Some(last_run) = job.last_run else {
        return job.interval_secs;
    };

    let elapsed = last_run
        .elapsed()
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    job.interval_secs.saturating_sub(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn status_reports_last_run_and_remaining_interval() {
        let scheduler = TaskScheduler::new(vec![CronJob {
            id: "job-a".to_string(),
            interval_secs: 60,
            command: CliCommand::Dream,
            last_run: None,
            enabled: true,
        }]);

        let initial = scheduler.status();
        assert_eq!(initial[0].next_run_secs, 60);
        assert!(initial[0].last_run_at.is_none());

        scheduler.mark_job_run_for_test("job-a");

        let status = scheduler.status();
        assert!(status[0].last_run_at.is_some());
        assert!(status[0].next_run_secs <= 60);
    }

    #[test]
    fn start_uses_tauri_runtime_without_requiring_a_current_tokio_reactor() {
        let scheduler = TaskScheduler::new(vec![CronJob {
            id: "job-a".to_string(),
            interval_secs: 3600,
            command: CliCommand::Dream,
            last_run: None,
            enabled: true,
        }]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            scheduler.start(Arc::new(|| {}));
        }));

        scheduler.stop();
        assert!(result.is_ok());
    }
}
