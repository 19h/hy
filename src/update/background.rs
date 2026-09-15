//! Background release discovery with completion signaling and advisory caching.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::config::Env;
use crate::error::Result;

mod cache;

#[derive(Default)]
enum Outcome {
    #[default]
    Idle,
    Running,
    Complete(Option<String>),
}

type Completion = Arc<(Mutex<Outcome>, Condvar)>;

#[derive(Default)]
pub struct BackgroundUpdateChecker {
    completion: Completion,
}

impl BackgroundUpdateChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self) {
        let env = Env::global();
        self.start_with(
            cache::Cache::for_binary(&env.binary_name),
            env.version.clone(),
            env.binary_name.clone(),
            || {
                let repo = super::GitHubRepo::from_url(&env.github_url)?;
                super::compatible_version(
                    &repo,
                    &format!(">{}", env.version),
                    env.version.contains("dev"),
                )
                .map(|release| release.map(|release| release.version.to_string()))
            },
        );
    }

    fn start_with(
        &mut self,
        cache: cache::Cache,
        current: String,
        binary_name: String,
        check: impl FnOnce() -> Result<Option<String>> + Send + 'static,
    ) {
        let (state, _) = &*self.completion;
        let mut state = state.lock().expect("update completion mutex");
        if !matches!(*state, Outcome::Idle) {
            return;
        }
        if !cache.should_check(chrono::Utc::now().naive_utc()) {
            // Upstream's cached-result comparison passes a JSON string to a
            // packaging.Version comparison and returns no notification.
            *state = Outcome::Complete(None);
            return;
        }
        *state = Outcome::Running;
        drop(state);

        let completion = Arc::clone(&self.completion);
        let worker = std::thread::Builder::new().name("hy-update-check".into()).spawn(move || {
            let message = check().ok().map(|latest| {
                cache.save(latest.as_deref());
                match latest {
                    Some(latest) => format!(
                        "\nUpdate available! {current} -> {latest}\nRun {binary_name} update to update\n"
                    ),
                    None => format!("\nYou have the latest version {current}! "),
                }
            });
            finish(&completion, message);
        });
        if worker.is_err() {
            finish(&self.completion, None);
        }
        // Dropping the handle detaches the worker, like upstream's daemon thread.
    }

    pub fn get_result(&self, timeout: Duration) -> Option<String> {
        let (state, signal) = &*self.completion;
        let (state, _) = signal
            .wait_timeout_while(state.lock().expect("update completion mutex"), timeout, |state| {
                matches!(state, Outcome::Running)
            })
            .expect("update completion mutex");
        match &*state {
            Outcome::Complete(message) => message.clone(),
            _ => None,
        }
    }
}

fn finish(completion: &Completion, message: Option<String>) {
    let (state, signal) = &**completion;
    *state.lock().expect("update completion mutex") = Outcome::Complete(message);
    signal.notify_all();
}

#[cfg(test)]
mod tests;
