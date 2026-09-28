use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Wrong passwords tolerated in a row before the gate closes.
const MAX_ATTEMPTS: u32 = 5;

/// How long the gate stays closed, by how many times it has closed lately. Each series of failures
/// costs more than the last: trying again in a loop gets slower, where a fixed pause would only
/// make it start again at the same pace.
const BACKOFF: [Duration; 4] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
];

/// A quiet period of this length brings the level down a step.
const LEVEL_DECAY: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct State {
    failures: u32,
    level: usize,
    last_failure: Option<Instant>,
    locked_until: Option<Instant>,
}

/// Slows down whoever guesses the access password.
///
/// There is one gate for the whole application, not one per address: behind a reverse proxy every
/// request carries the proxy's address, so a gate per address would behave the same with an
/// illusion of precision. The consequence is that anyone can lock the others out by failing on
/// purpose; the level goes down after an hour of calm, so a lock lasts as long as the attack does.
///
/// Times are passed in rather than read, which keeps the behaviour testable.
#[derive(Debug, Default)]
pub struct Gate {
    state: Mutex<State>,
}

impl Gate {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn is_locked_out(&self, now: Instant) -> bool {
        self.state().locked_until.is_some_and(|until| now < until)
    }

    /// How many seconds until the gate opens, rounded up; 0 when it is open.
    #[must_use]
    pub fn remaining_seconds(&self, now: Instant) -> u64 {
        self.state()
            .locked_until
            .and_then(|until| until.checked_duration_since(now))
            .filter(|remaining| !remaining.is_zero())
            .map_or(0, |remaining| remaining.as_secs() + 1)
    }

    pub fn register_failure(&self, now: Instant) {
        let mut state = self.state();
        decay(&mut state, now);
        state.last_failure = Some(now);

        state.failures += 1;
        if state.failures < MAX_ATTEMPTS {
            return;
        }
        state.failures = 0;
        state.locked_until = Some(now + BACKOFF[state.level.min(BACKOFF.len() - 1)]);
        state.level += 1;
    }

    /// A correct password brings the level back to zero: yesterday's failures do not penalise
    /// someone who knows it.
    pub fn register_success(&self) {
        *self.state() = State::default();
    }
}

/// Calm since the last failure, or since the end of the lock, brings the level down and forgets the
/// isolated failures.
fn decay(state: &mut State, now: Instant) {
    let quiet_since = match (state.locked_until, state.last_failure) {
        (Some(until), Some(last)) => until.max(last),
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return,
    };
    let quiet = now.saturating_duration_since(quiet_since);
    let steps = quiet.as_secs() / LEVEL_DECAY.as_secs();
    if steps == 0 {
        return;
    }
    state.level = state
        .level
        .saturating_sub(usize::try_from(steps).unwrap_or(usize::MAX));
    state.failures = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fail_times(gate: &Gate, count: u32, at: Instant) {
        for _ in 0..count {
            gate.register_failure(at);
        }
    }

    #[test]
    fn a_few_wrong_passwords_do_not_close_the_gate() {
        let gate = Gate::default();
        let now = Instant::now();

        fail_times(&gate, MAX_ATTEMPTS - 1, now);

        assert!(!gate.is_locked_out(now));
        assert_eq!(gate.remaining_seconds(now), 0);
    }

    #[test]
    fn five_wrong_passwords_close_it_for_thirty_seconds() {
        let gate = Gate::default();
        let now = Instant::now();

        fail_times(&gate, 5, now);

        assert!(gate.is_locked_out(now));
        assert_eq!(gate.remaining_seconds(now), 31);
        assert!(gate.is_locked_out(now + Duration::from_secs(29)));
        assert!(!gate.is_locked_out(now + Duration::from_secs(30)));
    }

    #[test]
    fn each_series_costs_more_than_the_last() {
        let gate = Gate::default();
        let mut now = Instant::now();
        let mut waits = Vec::new();

        for _ in 0..5 {
            fail_times(&gate, 5, now);
            waits.push(gate.remaining_seconds(now) - 1);
            now += Duration::from_secs(waits.last().copied().unwrap());
        }

        // The last one stays at the ceiling
        assert_eq!(waits, [30, 60, 300, 900, 900]);
    }

    #[test]
    fn an_hour_of_calm_brings_the_cost_down_one_step() {
        let gate = Gate::default();
        let start = Instant::now();
        fail_times(&gate, 5, start);
        fail_times(&gate, 5, start + Duration::from_secs(31));
        assert_eq!(gate.remaining_seconds(start + Duration::from_secs(31)), 61);

        // A long calm after the second lock: the level goes down by one, then the next series
        // starts from where it stood
        let later = start + Duration::from_secs(31 + 60 + 3600);
        fail_times(&gate, 5, later);

        assert_eq!(gate.remaining_seconds(later), 61);
    }

    #[test]
    fn a_long_enough_calm_forgets_everything() {
        let gate = Gate::default();
        let start = Instant::now();
        fail_times(&gate, 5, start);
        fail_times(&gate, 5, start + Duration::from_secs(31));

        let much_later = start + Duration::from_secs(10 * 3600);
        fail_times(&gate, 5, much_later);

        assert_eq!(gate.remaining_seconds(much_later), 31);
    }

    #[test]
    fn isolated_failures_are_forgotten_after_a_calm_hour() {
        let gate = Gate::default();
        let start = Instant::now();
        fail_times(&gate, 4, start);

        // Four more an hour later would be eight in a row without the calm in between
        fail_times(&gate, 4, start + Duration::from_secs(3601));

        assert!(!gate.is_locked_out(start + Duration::from_secs(3601)));
    }

    #[test]
    fn the_right_password_opens_the_gate_and_resets_the_cost() {
        let gate = Gate::default();
        let now = Instant::now();
        fail_times(&gate, 5, now);
        fail_times(&gate, 5, now + Duration::from_secs(31));

        gate.register_success();

        assert!(!gate.is_locked_out(now + Duration::from_secs(32)));
        fail_times(&gate, 5, now + Duration::from_secs(40));
        assert_eq!(gate.remaining_seconds(now + Duration::from_secs(40)), 31);
    }
}
