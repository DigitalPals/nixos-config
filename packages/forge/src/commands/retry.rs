//! Generic retry logic with exponential backoff for network operations

use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc;

use super::CommandMessage;
use crate::constants::{DEFAULT_MAX_RETRIES, DEFAULT_RETRY_BASE_DELAY_SECS, DEFAULT_RETRY_MAX_DELAY_SECS};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay: Duration::from_secs(DEFAULT_RETRY_BASE_DELAY_SECS),
            max_delay: Duration::from_secs(DEFAULT_RETRY_MAX_DELAY_SECS),
        }
    }
}

/// Retry an async operation with exponential backoff.
///
/// The `step` parameter is used to send `StepDetail` messages showing retry progress.
/// The operation function returns `Result<T>` - `Ok` means success (stop retrying),
/// `Err` means failure (retry if attempts remain).
pub async fn retry_with_backoff<F, Fut, T>(
    tx: &mpsc::Sender<CommandMessage>,
    step: &str,
    config: &RetryConfig,
    operation: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            let delay = calculate_delay(attempt, config);
            let _ = tx
                .send(CommandMessage::StepDetail {
                    step: step.to_string(),
                    detail: format!("Retry {}/{} in {}s...", attempt, config.max_retries, delay.as_secs()),
                })
                .await;
            tokio::time::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::warn!(
                    "Attempt {}/{} for step '{}' failed: {}",
                    attempt + 1,
                    config.max_retries + 1,
                    step,
                    e
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries exhausted")))
}

/// Calculate delay with exponential backoff: base_delay * 2^(attempt-1), capped at max_delay
fn calculate_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let multiplier = 2u64.saturating_pow(attempt.saturating_sub(1));
    let delay = config.base_delay.saturating_mul(multiplier as u32);
    delay.min(config.max_delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_delay() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
        };

        assert_eq!(calculate_delay(1, &config), Duration::from_secs(2));
        assert_eq!(calculate_delay(2, &config), Duration::from_secs(4));
        assert_eq!(calculate_delay(3, &config), Duration::from_secs(8));
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig {
            max_retries: 10,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(10),
        };

        assert_eq!(calculate_delay(5, &config), Duration::from_secs(10)); // 2*16=32, capped to 10
    }

    #[tokio::test]
    async fn test_retry_succeeds_immediately() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let config = RetryConfig::default();

        let result = retry_with_backoff(&tx, "test", &config, || async {
            Ok::<_, anyhow::Error>(42)
        })
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1), // Short delay for tests
            max_delay: Duration::from_millis(10),
        };

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(&tx, "test", &config, || {
            let c = counter_clone.clone();
            async move {
                let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    Err(anyhow::anyhow!("not yet"))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
