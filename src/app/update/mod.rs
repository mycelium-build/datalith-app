mod cargo;
mod installation;
mod source;
mod state;

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui_kit::{App, AppContext as _, Context, Entity, Global, Task};

use crate::ui::notifications;

use cargo::CargoPackagerSource;
use installation::{DOWNLOAD_PAGE_URL, InstallationKind};
use source::{StagedUpdate, UpdateSource};
use state::{
    CandidateDecision, CheckTrigger, OperationId, UpdateFailure, UpdateMachine, UpdateState,
};

pub use state::UpdatePresentation;

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(10);
const CHECK_INTERVAL: Duration = Duration::from_hours(1);
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// The compiled updater public key.
const UPDATER_PUBKEY: &str = match option_env!("DATALITH_UPDATER_PUBKEY") {
    Some(pubkey) => pubkey,
    None => "",
};

#[allow(dead_code, reason = "D3 connects the global updater to the UI")]
struct GlobalUpdater(Entity<Updater>);

impl Global for GlobalUpdater {}

struct Staged {
    update: Box<dyn StagedUpdate>,
    bytes: Vec<u8>,
}

enum OperationMessage {
    Checked {
        id: OperationId,
        outcome: Result<Option<Box<dyn StagedUpdate>>, UpdateFailure>,
    },
    Progress {
        id: OperationId,
        received: u64,
        total: Option<u64>,
    },
    Downloaded {
        id: OperationId,
        outcome: Result<Staged, UpdateFailure>,
    },
    Installed {
        id: OperationId,
        result: Result<(), UpdateFailure>,
        staged: Option<Staged>,
    },
}

pub struct Updater {
    source: Arc<dyn UpdateSource>,
    installation: InstallationKind,
    machine: UpdateMachine,
    staged: Option<Staged>,
    sender: mpsc::UnboundedSender<OperationMessage>,
    _listener_task: Task<()>,
    schedule_task: Task<()>,
}

impl Updater {
    fn new(
        source: Arc<dyn UpdateSource>,
        installation: InstallationKind,
        cx: &Context<Self>,
    ) -> Self {
        let (sender, mut receiver) = mpsc::unbounded();
        let listener_task = cx.spawn(async move |this, cx| {
            while let Some(message) = receiver.next().await {
                if this
                    .update(cx, |updater, cx| updater.handle(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            source,
            installation,
            machine: UpdateMachine::default(),
            staged: None,
            sender,
            _listener_task: listener_task,
            schedule_task: Task::ready(()),
        }
    }

    #[must_use]
    #[allow(dead_code, reason = "D3 connects the global updater to the UI")]
    pub fn get(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalUpdater>()
            .map(|global| global.0.clone())
    }

    #[must_use]
    #[allow(dead_code, reason = "D3 adds the sidebar update control")]
    pub fn presentation(&self) -> UpdatePresentation {
        self.machine.presentation()
    }

    #[allow(dead_code, reason = "D3 adds the Help-menu command")]
    pub fn check_now(&mut self, cx: &mut Context<Self>) {
        self.start_check(CheckTrigger::Manual, cx);
    }

    #[allow(dead_code, reason = "D3 adds the sidebar update control")]
    pub fn activate(&mut self, cx: &mut Context<Self>) {
        match self.machine.state() {
            UpdateState::Ready { .. } => self.start_apply(cx),
            UpdateState::External { .. } => open_download_page(cx),
            _ => {}
        }
    }

    fn start_check(&mut self, trigger: CheckTrigger, cx: &mut Context<Self>) {
        if trigger == CheckTrigger::Automatic && !crate::app::settings::snapshot().automatic_updates
        {
            return;
        }
        let Some(id) = self.machine.begin_check(trigger) else {
            return;
        };
        let source = Arc::clone(&self.source);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("datalith-update-check".into())
            .spawn(move || {
                let outcome = source.check();
                let _ = sender.unbounded_send(OperationMessage::Checked { id, outcome });
            });
        if let Err(error) = spawned {
            eprintln!("Failed to start update discovery: {error}");
            self.handle_failure(id, UpdateFailure::Check, cx);
            return;
        }
        cx.notify();
    }

    fn start_download(
        &mut self,
        id: OperationId,
        update: Box<dyn StagedUpdate>,
        cx: &mut Context<Self>,
    ) {
        let sender = self.sender.clone();
        let progress_sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("datalith-update-download".into())
            .spawn(move || {
                let outcome = update
                    .download(&|received, total| {
                        let _ = progress_sender.unbounded_send(OperationMessage::Progress {
                            id,
                            received,
                            total,
                        });
                    })
                    .map(|bytes| Staged { update, bytes });
                let _ = sender.unbounded_send(OperationMessage::Downloaded { id, outcome });
            });
        if let Err(error) = spawned {
            eprintln!("Failed to start update download: {error}");
            self.handle_failure(id, UpdateFailure::Download, cx);
        }
    }

    fn start_apply(&mut self, cx: &mut Context<Self>) {
        let Some(staged) = self.staged.take() else {
            return;
        };
        let Some(id) = self.machine.begin_apply() else {
            self.staged = Some(staged);
            return;
        };
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("datalith-update-install".into())
            .spawn(move || {
                let result = staged.update.install(&staged.bytes);
                let staged = result.is_err().then_some(staged);
                let _ = sender.unbounded_send(OperationMessage::Installed { id, result, staged });
            });
        if let Err(error) = spawned {
            eprintln!("Failed to start update install: {error}");
            self.machine.abandon(id);
            notify_failure(UpdateFailure::Install, cx);
        }
        cx.notify();
    }

    fn handle(&mut self, message: OperationMessage, cx: &mut Context<Self>) {
        match message {
            OperationMessage::Checked { id, outcome } => self.handle_checked(id, outcome, cx),
            OperationMessage::Progress {
                id,
                received,
                total,
            } => {
                if self.machine.progress(id, received, total) {
                    cx.notify();
                }
            }
            OperationMessage::Downloaded { id, outcome } => match outcome {
                Ok(staged) => {
                    if self.machine.staged(id, staged.update.version().clone()) {
                        self.staged = Some(staged);
                        cx.notify();
                    }
                }
                Err(failure) => self.handle_failure(id, failure, cx),
            },
            OperationMessage::Installed { id, result, staged } => {
                if let Err(failure) = result
                    && self.machine.restore(id)
                {
                    self.staged = staged;
                    notify_failure(failure, cx);
                    cx.notify();
                }
            }
        }
    }

    fn handle_checked(
        &mut self,
        id: OperationId,
        outcome: Result<Option<Box<dyn StagedUpdate>>, UpdateFailure>,
        cx: &mut Context<Self>,
    ) {
        let manual = self.machine.in_flight_trigger() == Some(CheckTrigger::Manual);
        if !manual && !crate::app::settings::snapshot().automatic_updates {
            if self.machine.restore(id) {
                cx.notify();
            }
            return;
        }
        match outcome {
            Err(failure) => self.handle_failure(id, failure, cx),
            Ok(None) => {
                if self.machine.restore(id) {
                    report_up_to_date(manual, cx);
                }
            }
            Ok(Some(update)) => {
                let version = update.version().clone();
                match self.machine.decide_candidate(id, &version) {
                    CandidateDecision::Stale => {}
                    CandidateDecision::Ignore => {
                        if self.machine.restore(id) {
                            report_up_to_date(manual, cx);
                        }
                    }
                    CandidateDecision::Accept => {
                        if self.installation.is_self_managed() {
                            if self.machine.begin_download(id) {
                                self.start_download(id, update, cx);
                                cx.notify();
                            }
                        } else if self.machine.external(id, version) {
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn handle_failure(&mut self, id: OperationId, failure: UpdateFailure, cx: &mut Context<Self>) {
        let manual = self.machine.in_flight_trigger() == Some(CheckTrigger::Manual);
        if self.machine.restore(id) {
            if manual {
                notify_failure(failure, cx);
            }
            cx.notify();
        }
    }

    /// App-scoped automatic scheduling loop
    fn start_scheduling(&mut self, cx: &mut Context<Self>) {
        self.schedule_task = cx.spawn(async move |this, cx| {
            loop {
                let has_window = this
                    .update(cx, |_updater, cx| !cx.windows().is_empty())
                    .unwrap_or(false);
                if has_window {
                    break;
                }
                cx.background_executor().timer(WINDOW_POLL_INTERVAL).await;
            }
            cx.background_executor().timer(FIRST_CHECK_DELAY).await;
            loop {
                let scheduled = this.update(cx, |updater, cx| {
                    updater.start_check(CheckTrigger::Automatic, cx);
                });
                if scheduled.is_err() {
                    break;
                }
                cx.background_executor().timer(CHECK_INTERVAL).await;
            }
        });
    }
}

fn report_up_to_date(manual: bool, cx: &mut Context<Updater>) {
    if manual {
        notifications::push_window_notification(cx, notifications::update_up_to_date());
    }
    cx.notify();
}

fn notify_failure(failure: UpdateFailure, cx: &mut Context<Updater>) {
    let notification = match failure {
        UpdateFailure::Check => notifications::update_check_failed(),
        UpdateFailure::Download => notifications::update_download_failed(),
        UpdateFailure::Install => notifications::update_install_failed(),
    };
    notifications::push_window_notification(cx, notification);
}

fn open_download_page(cx: &mut Context<Updater>) {
    if let Err(error) = crate::app::system::open_url(DOWNLOAD_PAGE_URL) {
        notifications::push_window_notification(
            cx,
            notifications::open_url_failed(DOWNLOAD_PAGE_URL, &error),
        );
    }
}

pub fn init(cx: &mut App) {
    if !crate::app::version::is_release_build() || UPDATER_PUBKEY.is_empty() {
        return;
    }
    let Ok(source) = CargoPackagerSource::stable(UPDATER_PUBKEY) else {
        eprintln!("Update endpoint or build version is invalid; updater disabled");
        return;
    };
    let updater = cx.new(|cx| Updater::new(Arc::new(source), InstallationKind::detect(), cx));
    updater.update(cx, Updater::start_scheduling);
    cx.set_global(GlobalUpdater(updater));
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use cargo_packager_updater::semver::Version;
    use gpui_kit::{Entity, TestAppContext};

    use super::source::scripted::{ScriptedEvents, ScriptedSource, ScriptedUpdate};
    use super::*;

    fn create(
        cx: &TestAppContext,
        source: Arc<ScriptedSource>,
        installation: InstallationKind,
    ) -> Entity<Updater> {
        cx.update(|cx| cx.new(|cx| Updater::new(source, installation, cx)))
    }

    fn presentation(cx: &TestAppContext, updater: &Entity<Updater>) -> UpdatePresentation {
        cx.update(|cx| updater.read(cx).presentation())
    }

    fn wait_for(
        cx: &TestAppContext,
        updater: &Entity<Updater>,
        mut predicate: impl FnMut(&UpdatePresentation) -> bool,
    ) -> UpdatePresentation {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .expect("deadline");
        loop {
            cx.run_until_parked();
            let current = presentation(cx, updater);
            if predicate(&current) {
                return current;
            }
            assert!(
                Instant::now() < deadline,
                "updater did not reach the expected state; last was {current:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_until_idle(cx: &TestAppContext, updater: &Entity<Updater>) {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .expect("deadline");
        loop {
            cx.run_until_parked();
            let busy = cx.update(|cx| updater.read(cx).machine.is_busy());
            if !busy {
                return;
            }
            assert!(Instant::now() < deadline, "updater stayed busy");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn start_automatic(cx: &TestAppContext, updater: &Entity<Updater>) {
        cx.update(|cx| {
            updater.update(cx, |updater, cx| {
                updater.start_check(CheckTrigger::Automatic, cx);
            });
        });
    }

    fn scripted_update(events: &ScriptedEvents) -> ScriptedUpdate {
        ScriptedUpdate::new(Version::new(0, 2, 0), events)
            .with_chunks(vec![(5, Some(10)), (10, Some(10))])
            .with_bytes(b"verified update bytes".to_vec())
    }

    #[test]
    fn a_self_managed_automatic_check_stages_the_update() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_update(scripted_update(&events));
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        let presentation = wait_for(&cx, &updater, |presentation| {
            matches!(presentation, UpdatePresentation::Ready { .. })
        });

        assert_eq!(
            presentation,
            UpdatePresentation::Ready {
                version: "0.2.0".to_string()
            }
        );
        assert_eq!(events.snapshot(), vec!["check", "download"]);
    }

    #[test]
    fn an_externally_managed_check_exposes_the_release_without_downloading() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_update(scripted_update(&events));
        let updater = create(&cx, source, InstallationKind::ExternallyManaged);

        start_automatic(&cx, &updater);
        let presentation = wait_for(&cx, &updater, |presentation| {
            matches!(presentation, UpdatePresentation::External { .. })
        });

        assert_eq!(
            presentation,
            UpdatePresentation::External {
                version: "0.2.0".to_string()
            }
        );
        assert_eq!(events.snapshot(), vec!["check"]);
    }

    #[test]
    fn an_up_to_date_check_stays_hidden() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_no_update();
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        wait_until_idle(&cx, &updater);

        assert_eq!(presentation(&cx, &updater), UpdatePresentation::Hidden);
        assert_eq!(events.snapshot(), vec!["check"]);
    }

    #[test]
    fn a_check_failure_is_silent_and_retryable() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_check_failure(UpdateFailure::Check);
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        wait_until_idle(&cx, &updater);

        assert_eq!(presentation(&cx, &updater), UpdatePresentation::Hidden);
        assert_eq!(events.snapshot(), vec!["check"]);
    }

    #[test]
    fn a_failed_download_restores_the_staged_update() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_update(scripted_update(&events));
        source.push_update(
            ScriptedUpdate::new(Version::new(0, 3, 0), &events)
                .failing_download(UpdateFailure::Download),
        );
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        let staged = wait_for(&cx, &updater, |state| {
            matches!(state, UpdatePresentation::Ready { .. })
        });
        cx.update(|cx| updater.update(cx, Updater::check_now));
        wait_until_idle(&cx, &updater);

        assert_eq!(presentation(&cx, &updater), staged);
        assert_eq!(
            events.snapshot(),
            vec!["check", "download", "check", "download"]
        );
    }

    #[test]
    fn activating_a_staged_update_runs_install() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_update(scripted_update(&events));
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        wait_for(&cx, &updater, |presentation| {
            matches!(presentation, UpdatePresentation::Ready { .. })
        });

        cx.update(|cx| {
            updater.update(cx, Updater::activate);
        });
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .expect("deadline");
        while !events.snapshot().iter().any(|event| event == "install") {
            cx.run_until_parked();
            assert!(Instant::now() < deadline, "install did not run");
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(presentation(&cx, &updater), UpdatePresentation::Applying);
        assert_eq!(events.snapshot(), vec!["check", "download", "install"]);
    }

    #[test]
    fn a_failed_apply_returns_to_staged() {
        let cx = TestAppContext::single();
        let events = ScriptedEvents::default();
        let source = Arc::new(ScriptedSource::new(&events));
        source.push_update(scripted_update(&events).failing_install(UpdateFailure::Install));
        let updater = create(&cx, source, InstallationKind::SelfManaged);

        start_automatic(&cx, &updater);
        wait_for(&cx, &updater, |presentation| {
            matches!(presentation, UpdatePresentation::Ready { .. })
        });

        cx.update(|cx| {
            updater.update(cx, Updater::activate);
        });
        let presentation = wait_for(&cx, &updater, |presentation| {
            matches!(presentation, UpdatePresentation::Ready { .. })
        });

        assert_eq!(
            presentation,
            UpdatePresentation::Ready {
                version: "0.2.0".to_string()
            }
        );
        assert_eq!(events.snapshot(), vec!["check", "download", "install"]);
    }
}
