//! Typed local transition cells for the four standard flow macros.
//!
//! The admitted runtime owner must supply each cell. This module has no owner
//! registry, evaluator, callback queue, serialization or process-global state.
//! Callers mutate their existing transaction candidate before dispatching an
//! emitted execution branch. Data outputs are live cell reads, not snapshots
//! attached to an emitted branch; reentry can change them before a later read.

/// An execution output is emitted exactly when a method returns true.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DoOnceState {
    initialized: bool,
    completed: bool,
}
impl DoOnceState {
    pub(crate) const DEFAULT_START_CLOSED: bool = false;

    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn from_snapshot(initialized: bool, completed: bool) -> Result<Self, &'static str> {
        if !initialized && completed {
            return Err("Virgin DoOnce cannot already be completed");
        }
        Ok(Self {
            initialized,
            completed,
        })
    }

    /// The adapter samples Start Closed only when initialized() is false.
    /// Completion is written before the caller can dispatch Completed/reenter.
    pub(crate) fn enter(&mut self, start_closed: bool) -> bool {
        if !self.initialized {
            self.initialized = true;
            if start_closed {
                self.completed = true;
            }
        }
        if self.completed {
            return false;
        }
        self.completed = true;
        true
    }

    /// Reset before first Enter suppresses the later Start Closed initializer.
    /// Reset itself emits no execution output.
    pub(crate) fn reset(&mut self) {
        self.completed = false;
        self.initialized = true;
    }
}

/// A checked, exact signed int32 input; RVN Int is wider and must not truncate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DoNLimit(i32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DoNLimitOutOfRange {
    pub(crate) value: i64,
}
impl From<i32> for DoNLimit {
    fn from(value: i32) -> Self {
        Self(value)
    }
}
impl TryFrom<i64> for DoNLimit {
    type Error = DoNLimitOutOfRange;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        i32::try_from(value)
            .map(Self)
            .map_err(|_| DoNLimitOutOfRange { value })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DoNState {
    initialized: bool,
    counter: i32,
}
impl DoNState {
    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }
    /// Counter is a live output. Do not freeze it in an Exit event payload.
    pub(crate) fn counter(&self) -> i32 {
        self.counter
    }
    pub(crate) fn from_snapshot(initialized: bool, counter: i32) -> Result<Self, &'static str> {
        if counter < 0 || !initialized && counter != 0 {
            return Err("DoN retained counter/init state is unreachable");
        }
        Ok(Self {
            initialized,
            counter,
        })
    }

    /// n is live for every Enter and changing it does not reset Counter.
    pub(crate) fn enter(&mut self, limit: DoNLimit) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.counter = 0;
        }
        if self.counter >= limit.0 {
            return false;
        }
        // Private state starts/resets at zero and only increments here. Since
        // Counter < n <= i32::MAX, this addition cannot overflow. A future
        // deserializer must validate these invariants before constructing it.
        self.counter += 1;
        true
    }

    /// The nested initializer remains unconsumed if Reset precedes first Enter.
    pub(crate) fn reset(&mut self) {
        self.counter = 0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlipFlopBranch {
    A,
    B,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FlipFlopState {
    is_a: bool,
}
impl FlipFlopState {
    /// This data output reads the shared updated cell, including after reentry.
    pub(crate) fn is_a(&self) -> bool {
        self.is_a
    }
    pub(crate) fn from_snapshot(is_a: bool) -> Self {
        Self { is_a }
    }
    pub(crate) fn enter(&mut self) -> FlipFlopBranch {
        self.is_a = !self.is_a;
        if self.is_a {
            FlipFlopBranch::A
        } else {
            FlipFlopBranch::B
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateInput {
    Enter,
    Open,
    Close,
    Toggle,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GateState {
    initialized: bool,
    open: bool,
}
impl GateState {
    /// This belongs to the Gate pin only, not the generic Bool schema default.
    pub(crate) const DEFAULT_START_CLOSED: bool = true;
    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
    pub(crate) fn from_snapshot(initialized: bool, open: bool) -> Result<Self, &'static str> {
        if !initialized && open {
            return Err("Virgin Gate cannot already be open");
        }
        Ok(Self { initialized, open })
    }

    /// Every input shares one initializer and one Open cell. The adapter samples
    /// Start Closed only when initialized() is false. Controls emit no Exit.
    pub(crate) fn step(&mut self, input: GateInput, start_closed: bool) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.open = !start_closed;
        }
        match input {
            GateInput::Enter => self.open,
            GateInput::Open => {
                self.open = true;
                false
            }
            GateInput::Close => {
                self.open = false;
                false
            }
            GateInput::Toggle => {
                self.open = !self.open;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn do_once_first_repeat_reset_and_reset_before_first_keep_separate_init_gate() {
        let mut state = DoOnceState::default();
        assert!(!state.initialized() && !state.completed());
        assert!(state.enter(DoOnceState::DEFAULT_START_CLOSED));
        assert!(state.initialized() && state.completed());
        assert!(!state.enter(false));
        state.reset();
        assert!(state.enter(true)); // Reset does not reapply Start Closed.
        let mut fresh = DoOnceState::default();
        fresh.reset();
        assert!(fresh.initialized() && !fresh.completed());
        assert!(fresh.enter(true));
    }

    #[test]
    fn do_once_start_closed_is_sampled_once_and_reentrant_completed_is_closed() {
        let mut state = DoOnceState::default();
        assert!(!state.enter(true));
        assert!(!state.enter(false));
        state.reset();
        if state.enter(false) {
            assert!(state.completed());
            assert!(
                !state.enter(false),
                "recipient reentry observes the write before Completed"
            );
        } else {
            panic!("Reset must permit one execution");
        }
    }

    #[test]
    fn do_n_dynamic_limit_never_resets_counter_and_reset_emits_nothing() {
        let mut state = DoNState::default();
        state.reset();
        assert!(!state.initialized());
        assert!(!state.enter(0.into()));
        assert!(state.initialized());
        assert!(!state.enter((-2).into()));
        assert_eq!(state.counter(), 0);
        assert!(state.enter(2.into()));
        assert_eq!(state.counter(), 1);
        assert!(state.enter(2.into()));
        assert_eq!(state.counter(), 2);
        assert!(!state.enter(2.into()));
        assert!(state.enter(3.into()));
        assert_eq!(state.counter(), 3);
        assert!(!state.enter(1.into()));
        assert_eq!(state.counter(), 3);
        state.reset();
        assert_eq!(state.counter(), 0);
        assert!(state.enter(1.into()));
    }

    #[test]
    fn do_n_bounded_recipient_reentry_reads_incremented_live_counter() {
        fn recipient(state: &mut DoNState, remaining: usize, exits: &mut Vec<i32>) {
            assert!(remaining > 0, "test reentry bound exhausted");
            if state.enter(3.into()) {
                exits.push(state.counter());
                recipient(state, remaining - 1, exits);
            }
        }
        let mut state = DoNState::default();
        let mut exits = Vec::new();
        recipient(&mut state, 4, &mut exits);
        assert_eq!(exits, [1, 2, 3]);
        assert_eq!(state.counter(), 3);
    }

    #[test]
    fn do_n_int32_admission_and_last_reachable_increment_do_not_wrap() {
        assert_eq!(DoNLimit::try_from(i64::from(i32::MAX)), Ok(i32::MAX.into()));
        assert_eq!(DoNLimit::try_from(i64::from(i32::MIN)), Ok(i32::MIN.into()));
        for value in [i64::from(i32::MAX) + 1, i64::from(i32::MIN) - 1] {
            assert_eq!(DoNLimit::try_from(value), Err(DoNLimitOutOfRange { value }));
        }
        // A valid reachable boundary cell avoids billions of test activations.
        let mut state = DoNState {
            initialized: true,
            counter: i32::MAX - 1,
        };
        assert!(state.enter(i32::MAX.into()));
        assert_eq!(state.counter(), i32::MAX);
        assert!(!state.enter(i32::MAX.into()));
        assert!(!state.enter((-1).into()));
        assert_eq!(state.counter(), i32::MAX);
    }

    #[test]
    fn flip_flop_alternates_before_output_and_is_a_remains_a_live_read() {
        let mut state = FlipFlopState::default();
        assert!(!state.is_a());
        let outer = state.enter();
        assert_eq!(outer, FlipFlopBranch::A);
        assert!(state.is_a());
        let reentrant = state.enter();
        assert_eq!(reentrant, FlipFlopBranch::B);
        assert!(
            !state.is_a(),
            "IsA is not a frozen true carried by the earlier A branch"
        );
        assert_eq!(state.enter(), FlipFlopBranch::A);
        assert!(state.is_a());
    }

    #[test]
    fn gate_default_closed_and_first_open_close_toggle_share_initialization() {
        let mut state = GateState::default();
        assert!(!state.step(GateInput::Enter, GateState::DEFAULT_START_CLOSED));
        assert!(state.initialized());
        assert!(!state.step(GateInput::Enter, false));
        assert!(!state.step(GateInput::Open, true));
        assert!(state.is_open());
        assert!(state.step(GateInput::Enter, true));
        assert!(!state.step(GateInput::Close, false));
        assert!(!state.is_open());
        assert!(!state.step(GateInput::Enter, false));
        assert!(!state.step(GateInput::Toggle, true));
        assert!(state.is_open());
        assert!(state.step(GateInput::Enter, true));
        for (input, start_closed, expected) in [
            (GateInput::Open, true, true),
            (GateInput::Close, false, false),
            (GateInput::Toggle, true, true),
            (GateInput::Toggle, false, false),
        ] {
            let mut first = GateState::default();
            assert!(!first.step(input, start_closed));
            assert_eq!(first.is_open(), expected);
            assert_eq!(first.step(GateInput::Enter, !start_closed), expected);
        }
    }

    #[test]
    fn gate_exit_recipient_can_close_before_immediate_reentrant_enter() {
        let mut state = GateState::default();
        if state.step(GateInput::Enter, false) {
            assert!(!state.step(GateInput::Close, true));
            assert!(!state.step(GateInput::Enter, false));
        } else {
            panic!("Explicit Start Closed=false must permit Enter");
        }
    }

    #[test]
    fn caller_supplied_cells_are_independent_and_candidate_copy_does_not_publish_itself() {
        let original = DoOnceState::default();
        let mut candidate = original;
        assert!(candidate.enter(false));
        assert_ne!(candidate, original);
        let mut independent = original;
        assert!(independent.enter(false));
        let mut a = FlipFlopState::default();
        let mut b = FlipFlopState::default();
        assert_eq!(a.enter(), FlipFlopBranch::A);
        assert_eq!(a.enter(), FlipFlopBranch::B);
        assert_eq!(b.enter(), FlipFlopBranch::A);
        // These are ordinary local cells, not evidence of Engine owner/save admission.
        assert!(!original.initialized());
    }
}
