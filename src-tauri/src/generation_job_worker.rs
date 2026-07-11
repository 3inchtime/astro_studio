use crate::api_gateway::{EngineCallError, RetryAfterHint};
use crate::models::GenerationJobStatus;
use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupAction {
    KeepQueued,
    RecoverResponse,
    AcknowledgeCancellation,
    Interrupt,
    IgnoreTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupRecoveryEvidence {
    PreProvider,
    ProviderOutcomeMayExist,
    ResponseReady,
}

pub(crate) fn startup_action(
    status: &GenerationJobStatus,
    evidence: StartupRecoveryEvidence,
    cancel_requested: bool,
) -> StartupAction {
    match status {
        GenerationJobStatus::Queued if cancel_requested => StartupAction::AcknowledgeCancellation,
        GenerationJobStatus::Queued => StartupAction::KeepQueued,
        GenerationJobStatus::Running if evidence == StartupRecoveryEvidence::ResponseReady => {
            StartupAction::RecoverResponse
        }
        GenerationJobStatus::Running
            if evidence == StartupRecoveryEvidence::PreProvider && cancel_requested =>
        {
            StartupAction::AcknowledgeCancellation
        }
        GenerationJobStatus::Running => StartupAction::Interrupt,
        GenerationJobStatus::Completed
        | GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => StartupAction::IgnoreTerminal,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutomaticRetryPolicy {
    base_delay: Duration,
    max_delay: Duration,
}

impl AutomaticRetryPolicy {
    pub(crate) fn new(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            base_delay,
            max_delay,
        }
    }

    pub(crate) fn delay(
        &self,
        error: &EngineCallError,
        auto_attempt: i32,
        max_auto_attempts: i32,
        now: DateTime<Utc>,
        jitter: Duration,
    ) -> Option<Duration> {
        let code_is_automatically_retryable = matches!(
            error.code.as_str(),
            "rate_limited" | "provider_unavailable" | "network_before_response"
        );
        if !code_is_automatically_retryable
            || !error.safe_to_retry
            || error.outcome_ambiguous
            || auto_attempt < 0
            || max_auto_attempts < 0
            || auto_attempt >= max_auto_attempts
        {
            return None;
        }

        let delay = match error.retry_after.as_ref() {
            Some(RetryAfterHint::DelaySeconds(seconds)) => Duration::from_secs(*seconds),
            Some(RetryAfterHint::HttpDate(retry_at)) if retry_at <= &now => Duration::ZERO,
            Some(RetryAfterHint::HttpDate(retry_at)) => (*retry_at - now).to_std().ok()?,
            Some(RetryAfterHint::Invalid) => return None,
            None => {
                if self.base_delay.is_zero() {
                    jitter
                } else {
                    let mut delay = self.base_delay;
                    let mut remaining = u32::try_from(auto_attempt).ok()?;
                    while remaining > 0 {
                        let step = remaining.min(31);
                        delay = delay.checked_mul(1_u32 << step)?;
                        if delay > self.max_delay {
                            return None;
                        }
                        remaining -= step;
                    }
                    delay.checked_add(jitter)?
                }
            }
        };

        (delay <= self.max_delay).then_some(delay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderFailureAction {
    RetryAfter(Duration),
    Terminal {
        status: GenerationJobStatus,
        retryable: bool,
    },
}

pub(crate) fn provider_failure_action(
    policy: &AutomaticRetryPolicy,
    error: &EngineCallError,
    auto_attempt: i32,
    max_auto_attempts: i32,
    now: DateTime<Utc>,
    jitter: Duration,
) -> ProviderFailureAction {
    if let Some(delay) = policy.delay(error, auto_attempt, max_auto_attempts, now, jitter) {
        return ProviderFailureAction::RetryAfter(delay);
    }

    let retryable = error.safe_to_retry
        || error.outcome_ambiguous
        || matches!(
            error.code.as_str(),
            "rate_limited"
                | "provider_unavailable"
                | "network_before_response"
                | "provider_outcome_unknown"
        );
    ProviderFailureAction::Terminal {
        status: if error.outcome_ambiguous {
            GenerationJobStatus::Interrupted
        } else {
            GenerationJobStatus::Failed
        },
        retryable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn retry_policy() -> AutomaticRetryPolicy {
        AutomaticRetryPolicy::new(Duration::from_secs(2), Duration::from_secs(60))
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 8, 0, 0)
            .single()
            .expect("valid fixed time")
    }

    #[test]
    fn startup_reconciliation_keeps_recovers_cancels_and_interrupts_exact_states() {
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Queued,
                StartupRecoveryEvidence::PreProvider,
                false,
            ),
            StartupAction::KeepQueued
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::ResponseReady,
                true,
            ),
            StartupAction::RecoverResponse,
            "a known provider result wins over cancellation and must finish locally"
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::PreProvider,
                true,
            ),
            StartupAction::AcknowledgeCancellation
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::ProviderOutcomeMayExist,
                true,
            ),
            StartupAction::Interrupt,
            "a stale cancel cannot discard an unknown provider outcome"
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::PreProvider,
                false,
            ),
            StartupAction::Interrupt
        );
        for terminal in [
            GenerationJobStatus::Completed,
            GenerationJobStatus::Failed,
            GenerationJobStatus::Cancelled,
            GenerationJobStatus::Interrupted,
        ] {
            assert_eq!(
                startup_action(&terminal, StartupRecoveryEvidence::ResponseReady, true),
                StartupAction::IgnoreTerminal
            );
        }
    }

    #[test]
    fn automatic_retry_policy_obeys_exact_attempt_boundaries_and_jittered_backoff() {
        let error = EngineCallError::network_before_response();
        let now = fixed_now();
        let jitter = Duration::from_millis(250);

        assert_eq!(retry_policy().delay(&error, 0, 0, now, jitter), None);
        assert_eq!(
            retry_policy().delay(&error, 0, 1, now, jitter),
            Some(Duration::from_millis(2250))
        );
        assert_eq!(retry_policy().delay(&error, 1, 1, now, jitter), None);
        assert_eq!(
            retry_policy().delay(&error, 1, 2, now, jitter),
            Some(Duration::from_millis(4250))
        );
        assert_eq!(retry_policy().delay(&error, 2, 2, now, jitter), None);
        assert_eq!(retry_policy().delay(&error, -1, 2, now, jitter), None);
    }

    #[test]
    fn retry_after_seconds_and_http_dates_are_typed_capped_and_never_jittered() {
        let now = fixed_now();
        let seconds = EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(3)));
        assert_eq!(
            retry_policy().delay(&seconds, 0, 2, now, Duration::from_secs(9)),
            Some(Duration::from_secs(3))
        );

        let future = EngineCallError::from_http_status(
            503,
            Some(RetryAfterHint::HttpDate(now + chrono::Duration::seconds(7))),
        );
        assert_eq!(
            retry_policy().delay(&future, 0, 2, now, Duration::ZERO),
            Some(Duration::from_secs(7))
        );
        let past = EngineCallError::from_http_status(
            503,
            Some(RetryAfterHint::HttpDate(now - chrono::Duration::seconds(1))),
        );
        assert_eq!(
            retry_policy().delay(&past, 0, 2, now, Duration::ZERO),
            Some(Duration::ZERO)
        );

        let too_large =
            EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(61)));
        assert_eq!(
            retry_policy().delay(&too_large, 0, 2, now, Duration::ZERO),
            None
        );
    }

    #[test]
    fn invalid_retry_after_ambiguous_and_rejected_errors_never_auto_retry() {
        let now = fixed_now();
        let invalid = EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid));
        assert!(!invalid.safe_to_retry);
        assert_eq!(
            retry_policy().delay(&invalid, 0, 2, now, Duration::ZERO),
            None
        );
        assert_eq!(
            retry_policy().delay(
                &EngineCallError::provider_outcome_unknown("closed after send"),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            None
        );
        assert_eq!(
            retry_policy().delay(
                &EngineCallError::request_rejected(),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            None
        );
        let future_safe_classification = EngineCallError {
            code: "future_safe_classification".to_string(),
            sanitized_message: "A future provider failure".to_string(),
            retry_after: None,
            safe_to_retry: true,
            outcome_ambiguous: false,
        };
        assert_eq!(
            retry_policy().delay(&future_safe_classification, 0, 2, now, Duration::ZERO,),
            None,
            "new error codes require an explicit automatic-retry policy decision"
        );
    }

    #[test]
    fn overflowing_or_over_cap_fallback_backoff_stops_automatic_retry() {
        let representable = AutomaticRetryPolicy::new(Duration::from_nanos(1), Duration::MAX);
        assert_eq!(
            representable.delay(
                &EngineCallError::network_before_response(),
                32,
                33,
                fixed_now(),
                Duration::ZERO,
            ),
            Some(Duration::from_nanos(1_u64 << 32)),
            "attempt ordinals must be limited by Duration/cap, not a u32 multiplier"
        );

        let policy = AutomaticRetryPolicy::new(Duration::MAX, Duration::MAX);
        assert_eq!(
            policy.delay(
                &EngineCallError::network_before_response(),
                1,
                i32::MAX,
                fixed_now(),
                Duration::from_nanos(1),
            ),
            None
        );
    }

    #[test]
    fn provider_failure_policy_separates_auto_retry_from_manual_retryability() {
        let now = fixed_now();
        let automatic =
            EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(3)));
        assert_eq!(
            provider_failure_action(&retry_policy(), &automatic, 0, 2, now, Duration::ZERO,),
            ProviderFailureAction::RetryAfter(Duration::from_secs(3))
        );

        let ambiguous = EngineCallError::provider_outcome_unknown("closed after send");
        assert_eq!(
            provider_failure_action(&retry_policy(), &ambiguous, 0, 2, now, Duration::ZERO,),
            ProviderFailureAction::Terminal {
                status: GenerationJobStatus::Interrupted,
                retryable: true,
            }
        );

        for manually_retryable in [
            EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid)),
            EngineCallError::from_http_status(503, Some(RetryAfterHint::DelaySeconds(61))),
            EngineCallError::network_before_response(),
        ] {
            assert_eq!(
                provider_failure_action(
                    &retry_policy(),
                    &manually_retryable,
                    2,
                    2,
                    now,
                    Duration::ZERO,
                ),
                ProviderFailureAction::Terminal {
                    status: GenerationJobStatus::Failed,
                    retryable: true,
                }
            );
        }

        assert_eq!(
            provider_failure_action(
                &retry_policy(),
                &EngineCallError::request_rejected(),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            ProviderFailureAction::Terminal {
                status: GenerationJobStatus::Failed,
                retryable: false,
            }
        );
    }
}
