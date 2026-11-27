//! Pausing Platform API status polls after Govee says there were too many
//! requests.
//!
//! A failed poll is otherwise attempted again on every tick of the poll loop,
//! which keeps an account over its limit.
//! <https://github.com/wez/govee2mqtt/issues/542>

use std::time::{Duration, Instant};

const FIRST_PAUSE: Duration = Duration::from_secs(60);
/// The longest pause chosen here, matching the normal poll interval
const LONGEST_PAUSE: Duration = Duration::from_secs(900);
/// The longest pause honored when Govee asks for one
const LONGEST_REQUESTED_PAUSE: Duration = Duration::from_secs(3600);

#[derive(Debug, Default)]
pub struct PollBackoff {
    until: Option<Instant>,
    last_pause: Option<Duration>,
}

impl PollBackoff {
    /// How much longer polling is paused for
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.until?
            .checked_duration_since(now)
            .filter(|wait| !wait.is_zero())
    }

    /// Record that Govee refused a poll. Returns the length of the pause that
    /// this starts, or `None` if polling was paused already.
    pub fn rate_limited(
        &mut self,
        now: Instant,
        retry_after: Option<Duration>,
    ) -> Option<Duration> {
        if self.remaining(now).is_some() {
            return None;
        }

        let doubled = self
            .last_pause
            .map(|pause| pause.saturating_mul(2).min(LONGEST_PAUSE))
            .unwrap_or(FIRST_PAUSE);
        let pause = doubled.max(
            retry_after
                .unwrap_or(Duration::ZERO)
                .min(LONGEST_REQUESTED_PAUSE),
        );

        self.last_pause = Some(doubled);
        self.until = now.checked_add(pause);
        Some(pause)
    }

    /// A poll succeeded, so the next refusal starts from the shortest pause
    pub fn succeeded(&mut self) {
        self.until = None;
        self.last_pause = None;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn pauses_grow_until_a_poll_succeeds() {
        let mut backoff = PollBackoff::default();
        let mut now = Instant::now();
        assert_eq!(backoff.remaining(now), None);

        for expected in [60, 120, 240, 480, 900, 900] {
            assert_eq!(backoff.rate_limited(now, None), Some(secs(expected)));
            assert_eq!(backoff.remaining(now), Some(secs(expected)));
            // Further refusals during the pause do not lengthen it
            assert_eq!(backoff.rate_limited(now, None), None);
            now += secs(expected);
            assert_eq!(backoff.remaining(now), None);
        }

        backoff.succeeded();
        assert_eq!(backoff.rate_limited(now, None), Some(secs(60)));
    }

    #[test]
    fn the_wait_govee_asks_for_is_honored() {
        let now = Instant::now();

        let mut backoff = PollBackoff::default();
        assert_eq!(
            backoff.rate_limited(now, Some(secs(1800))),
            Some(secs(1800))
        );

        let mut backoff = PollBackoff::default();
        assert_eq!(backoff.rate_limited(now, Some(secs(5))), Some(secs(60)));

        let mut backoff = PollBackoff::default();
        assert_eq!(
            backoff.rate_limited(now, Some(secs(86400))),
            Some(secs(3600))
        );
    }
}
