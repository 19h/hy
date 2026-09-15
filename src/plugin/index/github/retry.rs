//! GitHub catalogue request retries, before response-body consumption.

use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

mod delay;

const TRANSIENT_ATTEMPTS: u32 = 4;
const RATE_LIMIT_ATTEMPTS: u32 = 5;

pub(super) async fn send(
    client: &reqwest::Client,
    request: reqwest::Request,
) -> Result<reqwest::Response> {
    execute(
        || async {
            let request = request.try_clone().ok_or_else(|| {
                Failure::Terminal(Error::Other("GitHub request body cannot be replayed".into()))
            })?;
            client.execute(request).await.map_err(Failure::from)
        },
        tokio::time::sleep,
        timestamp,
    )
    .await
}

enum Failure {
    Transient(Error),
    Terminal(Error),
}

impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        if error.is_connect() || error.is_timeout() {
            Self::Transient(error.into())
        } else {
            Self::Terminal(error.into())
        }
    }
}

async fn execute<S, SF, W, WF, C>(mut send: S, mut wait: W, now: C) -> Result<reqwest::Response>
where
    S: FnMut() -> SF,
    SF: Future<Output = std::result::Result<reqwest::Response, Failure>>,
    W: FnMut(Duration) -> WF,
    WF: Future<Output = ()>,
    C: Fn() -> f64,
{
    for transient_attempt in 1..=TRANSIENT_ATTEMPTS {
        for rate_attempt in 1..=RATE_LIMIT_ATTEMPTS {
            match send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    if matches!(status, 403 | 429) {
                        // Tenacity evaluates its wait strategy even on the last
                        // attempt. Invalid headers must still fail at that point.
                        let delay = delay::reactive(response.headers(), rate_attempt, &now)?;
                        if rate_attempt < RATE_LIMIT_ATTEMPTS {
                            tracing::info!(seconds = delay.as_secs_f64(), "GitHub rate limit wait");
                            wait(delay).await;
                            continue;
                        }
                    } else if matches!(status, 500 | 502 | 503 | 504) {
                        if transient_attempt < TRANSIENT_ATTEMPTS {
                            break;
                        }
                    } else if response.status().is_success()
                        && let Some(delay) = delay::proactive(response.headers(), &now)?
                    {
                        tracing::warn!(
                            seconds = delay.as_secs_f64(),
                            "GitHub proactive rate limit wait"
                        );
                        wait(delay).await;
                    }
                    return Ok(response);
                }
                Err(Failure::Transient(error)) => {
                    if transient_attempt == TRANSIENT_ATTEMPTS {
                        return Err(error);
                    }
                    break;
                }
                Err(Failure::Terminal(error)) => return Err(error),
            }
        }
        let seconds = 2_u64.pow(transient_attempt);
        tracing::warn!(seconds, attempt = transient_attempt, "Retrying transient GitHub failure");
        wait(Duration::from_secs(seconds)).await;
    }
    unreachable!("the final attempt returns its response or error")
}

fn timestamp() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

#[cfg(test)]
mod tests;
