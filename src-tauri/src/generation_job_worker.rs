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
        if !error.safe_to_retry
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
}
