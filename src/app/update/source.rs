use cargo_packager_updater::semver::Version;

use super::state::UpdateFailure;

pub trait StagedUpdate: Send {
    fn version(&self) -> &Version;

    fn download(&self, on_progress: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, UpdateFailure>;

    fn install(&self, bytes: &[u8]) -> anyhow::Result<()>;
}

pub trait UpdateSource: Send + Sync + 'static {
    fn check(&self) -> Result<Option<Box<dyn StagedUpdate>>, UpdateFailure>;
}

#[cfg(test)]
pub mod scripted {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use cargo_packager_updater::semver::Version;

    use super::{StagedUpdate, UpdateFailure, UpdateSource};

    #[derive(Clone, Default)]
    pub struct ScriptedEvents {
        inner: Arc<Mutex<Vec<String>>>,
    }

    impl ScriptedEvents {
        pub fn record(&self, event: &str) {
            self.inner
                .lock()
                .expect("scripted events lock")
                .push(event.to_string());
        }

        #[must_use]
        pub fn snapshot(&self) -> Vec<String> {
            self.inner.lock().expect("scripted events lock").clone()
        }
    }

    pub struct ScriptedUpdate {
        version: Version,
        chunks: Vec<(u64, Option<u64>)>,
        bytes: Vec<u8>,
        download_failure: Option<UpdateFailure>,
        events: ScriptedEvents,
    }

    impl ScriptedUpdate {
        #[must_use]
        pub fn new(version: Version, events: &ScriptedEvents) -> Self {
            Self {
                version,
                chunks: Vec::new(),
                bytes: Vec::new(),
                download_failure: None,
                events: events.clone(),
            }
        }

        #[must_use]
        pub fn with_chunks(mut self, chunks: Vec<(u64, Option<u64>)>) -> Self {
            self.chunks = chunks;
            self
        }

        #[must_use]
        pub fn with_bytes(mut self, bytes: Vec<u8>) -> Self {
            self.bytes = bytes;
            self
        }

        #[must_use]
        pub fn failing_download(mut self, failure: UpdateFailure) -> Self {
            self.download_failure = Some(failure);
            self
        }
    }

    impl StagedUpdate for ScriptedUpdate {
        fn version(&self) -> &Version {
            &self.version
        }

        fn download(
            &self,
            on_progress: &dyn Fn(u64, Option<u64>),
        ) -> Result<Vec<u8>, UpdateFailure> {
            self.events.record("download");
            if let Some(failure) = self.download_failure {
                return Err(failure);
            }
            for (received, total) in &self.chunks {
                on_progress(*received, *total);
            }
            Ok(self.bytes.clone())
        }

        fn install(&self, _bytes: &[u8]) -> anyhow::Result<()> {
            self.events.record("install");
            Ok(())
        }
    }

    type CheckOutcome = Result<Option<Box<dyn StagedUpdate>>, UpdateFailure>;

    pub struct ScriptedSource {
        outcomes: Mutex<VecDeque<CheckOutcome>>,
        events: ScriptedEvents,
    }

    impl ScriptedSource {
        #[must_use]
        pub fn new(events: &ScriptedEvents) -> Self {
            Self {
                outcomes: Mutex::new(VecDeque::new()),
                events: events.clone(),
            }
        }

        pub fn push_check(&self, outcome: CheckOutcome) {
            self.outcomes
                .lock()
                .expect("scripted outcomes lock")
                .push_back(outcome);
        }

        pub fn push_update(&self, update: ScriptedUpdate) {
            self.push_check(Ok(Some(Box::new(update))));
        }

        pub fn push_no_update(&self) {
            self.push_check(Ok(None));
        }

        pub fn push_check_failure(&self, failure: UpdateFailure) {
            self.push_check(Err(failure));
        }
    }

    impl UpdateSource for ScriptedSource {
        fn check(&self) -> Result<Option<Box<dyn StagedUpdate>>, UpdateFailure> {
            self.events.record("check");
            self.outcomes
                .lock()
                .expect("scripted outcomes lock")
                .pop_front()
                .unwrap_or(Ok(None))
        }
    }
}
