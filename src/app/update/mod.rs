mod cargo;
mod installation;
mod restart;
mod source;
mod state;

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui_kit::{App, AppContext as _, Context, Entity, Global, Subscription, Task};

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

const UPDATER_PUBKEY: &str = env!("DATALITH_UPDATER_PUBKEY");

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
}

pub struct Updater {
    source: Arc<dyn UpdateSource>,
    installation: InstallationKind,
    machine: UpdateMachine,
    staged: Option<Staged>,
    sender: mpsc::UnboundedSender<OperationMessage>,
    _listener_task: Task<()>,
    schedule_task: Task<()>,
    _quit_subscription: Subscription,
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
            _quit_subscription: cx.on_app_quit(|updater, _| {
                let staged = updater.take_install();
                async move {
                    if let Some(staged) = staged {
                        restart::install(staged);
                    }
                }
            }),
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
    pub fn get(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalUpdater>()
            .map(|global| global.0.clone())
    }

    #[must_use]
    pub fn presentation(&self) -> UpdatePresentation {
        self.machine.presentation()
    }

    pub fn check_now(&mut self, cx: &mut Context<Self>) {
        self.start_check(CheckTrigger::Manual, cx);
    }

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
        if self.staged.is_some() && self.machine.begin_apply().is_some() {
            cx.notify();
            cx.quit();
        }
    }

    const fn take_install(&mut self) -> Option<Staged> {
        if matches!(self.machine.state(), UpdateState::Applying { .. }) {
            self.staged.take()
        } else {
            None
        }
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
    let Some(endpoint) = crate::channel::Channel::current().update_endpoint() else {
        return;
    };
    let Ok(source) = CargoPackagerSource::production(endpoint, UPDATER_PUBKEY) else {
        eprintln!("Update endpoint or build version is invalid; updater disabled");
        return;
    };
    let updater = cx.new(|cx| Updater::new(Arc::new(source), InstallationKind::detect(), cx));
    updater.update(cx, Updater::start_scheduling);
    cx.set_global(GlobalUpdater(updater));
}
