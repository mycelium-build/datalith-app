use cargo_packager_updater::semver::Version;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateFailure {
    Check,
    Download,
    Install,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckTrigger {
    Automatic,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    Downloading { received: u64, total: Option<u64> },
    Ready { version: Version },
    External { version: Version },
    Applying { version: Version },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePresentation {
    Hidden,
    Downloading { received: u64, total: Option<u64> },
    Ready { version: String },
    External { version: String },
    Applying,
}

impl UpdateState {
    #[must_use]
    pub fn presentation(&self) -> UpdatePresentation {
        match self {
            Self::Idle | Self::Checking => UpdatePresentation::Hidden,
            Self::Downloading { received, total } => UpdatePresentation::Downloading {
                received: *received,
                total: *total,
            },
            Self::Ready { version } => UpdatePresentation::Ready {
                version: version.to_string(),
            },
            Self::External { version } => UpdatePresentation::External {
                version: version.to_string(),
            },
            Self::Applying { .. } => UpdatePresentation::Applying,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateDecision {
    Accept,
    Ignore,
    Stale,
}

#[derive(Debug)]
pub struct UpdateMachine {
    state: UpdateState,
    previous: UpdateState,
    in_flight: Option<OperationId>,
    trigger: Option<CheckTrigger>,
    next_operation: u64,
}

impl Default for UpdateMachine {
    fn default() -> Self {
        Self {
            state: UpdateState::Idle,
            previous: UpdateState::Idle,
            in_flight: None,
            trigger: None,
            next_operation: 0,
        }
    }
}

impl UpdateMachine {
    #[must_use]
    pub const fn state(&self) -> &UpdateState {
        &self.state
    }

    #[must_use]
    pub fn presentation(&self) -> UpdatePresentation {
        self.state.presentation()
    }

    #[must_use]
    #[cfg(test)]
    pub const fn is_busy(&self) -> bool {
        self.in_flight.is_some()
    }

    #[must_use]
    pub const fn in_flight_trigger(&self) -> Option<CheckTrigger> {
        self.trigger
    }

    pub fn begin_check(&mut self, trigger: CheckTrigger) -> Option<OperationId> {
        if self.in_flight.is_some() {
            return None;
        }
        if trigger == CheckTrigger::Automatic
            && matches!(
                self.state,
                UpdateState::Ready { .. } | UpdateState::External { .. }
            )
        {
            return None;
        }

        let id = OperationId(self.next_operation);
        self.next_operation = self.next_operation.saturating_add(1);
        self.previous = self.state.clone();
        self.trigger = Some(trigger);
        self.in_flight = Some(id);
        self.state = UpdateState::Checking;
        Some(id)
    }

    pub fn decide_candidate(&self, id: OperationId, version: &Version) -> CandidateDecision {
        if self.in_flight != Some(id) {
            return CandidateDecision::Stale;
        }
        if let UpdateState::Ready { version: staged } = &self.previous
            && version <= staged
        {
            return CandidateDecision::Ignore;
        }
        CandidateDecision::Accept
    }

    pub fn begin_download(&mut self, id: OperationId) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.state = UpdateState::Downloading {
            received: 0,
            total: None,
        };
        true
    }

    pub fn progress(&mut self, id: OperationId, received: u64, total: Option<u64>) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.state = UpdateState::Downloading { received, total };
        true
    }

    pub fn staged(&mut self, id: OperationId, version: Version) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.finish();
        self.state = UpdateState::Ready { version };
        true
    }

    pub fn external(&mut self, id: OperationId, version: Version) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.finish();
        self.state = UpdateState::External { version };
        true
    }

    /// Preserve the staged offer after an unsuccessful check, download, or apply.
    pub fn restore(&mut self, id: OperationId) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.state = self.previous.clone();
        self.finish();
        true
    }

    pub fn begin_apply(&mut self) -> Option<OperationId> {
        let UpdateState::Ready { version } = &self.state else {
            return None;
        };
        if self.in_flight.is_some() {
            return None;
        }
        let version = version.clone();
        let id = OperationId(self.next_operation);
        self.next_operation = self.next_operation.saturating_add(1);
        self.previous = self.state.clone();
        self.in_flight = Some(id);
        self.state = UpdateState::Applying { version };
        Some(id)
    }

    const fn finish(&mut self) {
        self.in_flight = None;
        self.trigger = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn start(machine: &mut UpdateMachine, trigger: CheckTrigger) -> OperationId {
        machine.begin_check(trigger).expect("check starts")
    }

    fn stage(machine: &mut UpdateMachine, version: Version) {
        let id = start(machine, CheckTrigger::Automatic);
        assert_eq!(
            machine.decide_candidate(id, &version),
            CandidateDecision::Accept
        );
        assert!(machine.begin_download(id));
        assert!(machine.staged(id, version));
    }

    #[test]
    fn idle_and_checking_hide_the_control() {
        let mut machine = UpdateMachine::default();
        assert_eq!(machine.presentation(), UpdatePresentation::Hidden);
        start(&mut machine, CheckTrigger::Manual);
        assert_eq!(machine.presentation(), UpdatePresentation::Hidden);
    }

    #[test]
    fn concurrent_checks_are_coalesced() {
        let mut machine = UpdateMachine::default();
        let _ = start(&mut machine, CheckTrigger::Manual);
        assert_eq!(machine.begin_check(CheckTrigger::Automatic), None);
        assert_eq!(machine.begin_check(CheckTrigger::Manual), None);
    }

    #[test]
    fn automatic_discovery_is_skipped_while_staged() {
        let mut machine = UpdateMachine::default();
        stage(&mut machine, version(0, 2, 0));
        assert_eq!(machine.begin_check(CheckTrigger::Automatic), None);
        assert!(matches!(machine.state(), UpdateState::Ready { .. }));
    }

    #[test]
    fn manual_check_replaces_staged_only_with_a_newer_version() {
        let mut machine = UpdateMachine::default();
        stage(&mut machine, version(0, 2, 0));

        let id = start(&mut machine, CheckTrigger::Manual);
        assert_eq!(
            machine.decide_candidate(id, &version(0, 2, 0)),
            CandidateDecision::Ignore
        );
        assert!(machine.restore(id));
        assert_eq!(
            machine.state(),
            &UpdateState::Ready {
                version: version(0, 2, 0)
            }
        );

        let id = start(&mut machine, CheckTrigger::Manual);
        assert_eq!(
            machine.decide_candidate(id, &version(0, 3, 0)),
            CandidateDecision::Accept
        );
    }

    #[test]
    fn a_failed_recheck_restores_the_staged_update() {
        let mut machine = UpdateMachine::default();
        stage(&mut machine, version(0, 2, 0));

        let id = start(&mut machine, CheckTrigger::Manual);
        assert!(machine.restore(id));
        assert_eq!(
            machine.state(),
            &UpdateState::Ready {
                version: version(0, 2, 0)
            }
        );
        assert!(!machine.is_busy());
    }

    #[test]
    fn stale_completions_are_rejected() {
        let mut machine = UpdateMachine::default();
        let stale = start(&mut machine, CheckTrigger::Automatic);
        assert!(machine.restore(stale));

        let fresh = start(&mut machine, CheckTrigger::Manual);
        assert!(!machine.restore(stale));
        assert!(machine.restore(fresh));
    }

    #[test]
    fn download_progress_is_reported_until_staged() {
        let mut machine = UpdateMachine::default();
        let id = start(&mut machine, CheckTrigger::Manual);
        let candidate = version(0, 2, 0);
        assert_eq!(
            machine.decide_candidate(id, &candidate),
            CandidateDecision::Accept
        );
        assert!(machine.begin_download(id));
        assert!(machine.progress(id, 10, Some(20)));
        assert_eq!(
            machine.presentation(),
            UpdatePresentation::Downloading {
                received: 10,
                total: Some(20)
            }
        );
        assert!(machine.progress(id, 20, None));
        assert_eq!(
            machine.presentation(),
            UpdatePresentation::Downloading {
                received: 20,
                total: None
            }
        );
        assert!(machine.staged(id, candidate));
        assert_eq!(
            machine.presentation(),
            UpdatePresentation::Ready {
                version: "0.2.0".to_string()
            }
        );
    }

    #[test]
    fn externally_managed_discovery_exposes_the_release() {
        let mut machine = UpdateMachine::default();
        let id = start(&mut machine, CheckTrigger::Automatic);
        let candidate = version(0, 2, 0);
        assert_eq!(
            machine.decide_candidate(id, &candidate),
            CandidateDecision::Accept
        );
        assert!(machine.external(id, candidate));
        assert_eq!(
            machine.presentation(),
            UpdatePresentation::External {
                version: "0.2.0".to_string()
            }
        );
    }

    #[test]
    fn applying_is_only_allowed_while_staged_and_blocks_checks() {
        let mut machine = UpdateMachine::default();
        assert!(machine.begin_apply().is_none());

        stage(&mut machine, version(0, 2, 0));
        let id = machine.begin_apply().expect("apply begins");
        assert_eq!(machine.presentation(), UpdatePresentation::Applying);
        assert_eq!(machine.begin_check(CheckTrigger::Manual), None);

        assert!(machine.restore(id));
        assert!(matches!(machine.state(), UpdateState::Ready { .. }));
    }
}
