use crate::updates::{self, Preferences};
use crate::{
    update_download, update_install,
    update_release::{self, Nightly},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zgui::{
    prelude::*,
    reactive::{Owner, Task},
};

#[derive(Clone, Copy)]
pub struct UpdateState {
    pub preferences: RwSignal<Preferences, LocalStorage>,
    pub pending: RwSignal<Option<String>, LocalStorage>,
    pub message: RwSignal<String, LocalStorage>,
    pub ready: RwSignal<Option<Nightly>, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub installing: RwSignal<bool, LocalStorage>,
    pub failed: RwSignal<bool, LocalStorage>,
    pub install_problem: StoredValue<Option<String>, LocalStorage>,
    download_cancel: StoredValue<Arc<AtomicBool>, LocalStorage>,
    task: StoredValue<Option<Task>, LocalStorage>,
    owner: StoredValue<Owner, LocalStorage>,
}

impl UpdateState {
    pub fn new() -> Self {
        let (preferences, message) = match Preferences::load() {
            Ok(p) => (
                p,
                "Enable nightly builds to check for updates when Enlumi starts.".into(),
            ),
            Err(e) => (Preferences::default(), e),
        };
        let previous_error = update_install::take_error();
        let failed = previous_error.is_some();
        let message = previous_error.unwrap_or(message);
        let state = Self {
            preferences: RwSignal::new_local(preferences),
            pending: RwSignal::new_local(None),
            message: RwSignal::new_local(message),
            ready: RwSignal::new_local(None),
            busy: RwSignal::new_local(false),
            installing: RwSignal::new_local(false),
            failed: RwSignal::new_local(failed),
            install_problem: StoredValue::new_local(update_download::installation().err()),
            download_cancel: StoredValue::new_local(Arc::new(AtomicBool::new(false))),
            task: StoredValue::new_local(None),
            owner: StoredValue::new_local(Owner::current().expect("update checker owner")),
        };
        on_cleanup_local(move || {
            state.cancel();
            state
                .download_cancel
                .with_value(|flag| flag.store(true, Ordering::Relaxed));
        });
        if !failed {
            state.check();
        }
        state
    }

    fn cancel(self) {
        self.task.with_value(|task| {
            if let Some(task) = task {
                task.cancel();
            }
        });
        self.task.set_value(None);
    }

    pub fn enable(self, enabled: bool) {
        if self.busy.get_untracked() {
            return;
        }
        let mut preferences = self.preferences.get_untracked();
        preferences.nightly = enabled;
        if let Err(error) = preferences.save() {
            self.message.set(error);
            return;
        }
        self.cancel();
        self.preferences.set(preferences);
        self.pending.set(None);
        self.ready.set(None);
        self.failed.set(false);
        if enabled {
            self.message
                .set("Nightly update checks enabled. Checks start next time Enlumi opens.".into());
        } else {
            self.message.set("Automatic update checks off.".into());
        }
    }

    fn check(self) {
        if !self.preferences.with_untracked(|p| p.nightly) {
            return;
        }
        self.message
            .set("Checking master and completed nightlies…".into());
        let task = self.owner.with_value(|owner| owner.with(|| spawn_local(async move {
            let result = blocking(|| updates::latest().map(|head| (head, update_release::latest()))).await;
            match result {
                Err(error) => self.message.set(error),
                Ok((sha, release)) => {
                    self.receive_head(sha);
                    match release {
                        Ok(release) => {
                            if let Some(release) = &release {
                                self.message.set(format!("Nightly {} ready to install. Update & restart closes current session.", &release.commit[..8]));
                            }
                            self.ready.set(release);
                        }
                        Err(error) => self.message.set(format!("Could not find installable nightly: {error}")),
                    }
                }
            }
        })));
        self.task.set_value(Some(task));
    }

    fn receive_head(self, sha: String) {
        let mut preferences = self.preferences.get_untracked();
        if sha == updates::BUILD_COMMIT {
            self.pending.set(None);
            self.message.set("This build matches master.".into());
        } else if preferences.dismissed.as_deref() == Some(&sha) {
            self.pending.set(None);
            self.message
                .set(format!("No new master activity since {}.", &sha[..8]));
        } else if updates::BUILD_COMMIT.is_empty() && preferences.dismissed.is_none() {
            preferences.dismissed = Some(sha.clone());
            match preferences.save() {
                Ok(()) => {
                    self.preferences.set(preferences);
                    self.message.set(format!(
                        "Build revision unknown. Tracking master from {}.",
                        &sha[..8]
                    ));
                }
                Err(error) => self.message.set(error),
            }
        } else {
            self.message.set(format!(
                "Changes on master: {}. Nightly may still be building.",
                &sha[..8]
            ));
            self.pending.set(Some(sha));
        }
    }

    pub fn install(self) {
        if self.busy.get_untracked() {
            return;
        }
        let Some(release) = self.ready.get_untracked() else {
            return;
        };
        if let Some(error) = self.install_problem.get_value() {
            self.message.set(error);
            self.failed.set(true);
            return;
        }
        self.busy.set(true);
        self.failed.set(false);
        self.message.set(format!(
            "Downloading and verifying nightly ({} MiB)…",
            release.size.div_ceil(1024 * 1024)
        ));
        let cancel = Arc::new(AtomicBool::new(false));
        self.download_cancel.set_value(cancel.clone());
        self.owner.with_value(|owner| {
            owner.with(|| {
                spawn_local(async move {
                    let result = blocking(move || update_download::prepare(release, &cancel)).await;
                    let result = match result {
                        Ok(prepared)
                            if !self
                                .download_cancel
                                .with_value(|flag| flag.load(Ordering::Relaxed)) =>
                        {
                            self.installing.set(true);
                            self.message.set("Installing update and restarting…".into());
                            let cancel = self.download_cancel.get_value();
                            blocking(move || update_install::start(prepared, cancel)).await
                        }
                        Ok(_) => Err("Update cancelled".into()),
                        Err(_)
                            if self
                                .download_cancel
                                .with_value(|flag| flag.load(Ordering::Relaxed)) =>
                        {
                            Err("Update cancelled".into())
                        }
                        Err(error) => Err(error),
                    };
                    match result {
                        Ok(_handoff) => std::process::exit(0),
                        Err(error) => {
                            self.failed.set(error != "Update cancelled");
                            self.message.set(error);
                            self.busy.set(false);
                            self.installing.set(false);
                        }
                    }
                })
            })
        });
    }

    pub fn cancel_download(self) {
        if self.installing.get_untracked() {
            return;
        }
        self.download_cancel
            .with_value(|flag| flag.store(true, Ordering::Relaxed));
        self.message.set("Cancelling download…".into());
    }

    pub fn dismiss(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.failed.set(false);
        let Some(sha) = self.pending.get_untracked() else {
            return;
        };
        let mut preferences = self.preferences.get_untracked();
        preferences.dismissed = Some(sha);
        match preferences.save() {
            Ok(()) => {
                self.preferences.set(preferences);
                self.pending.set(None);
            }
            Err(error) => self.message.set(error),
        }
    }

    pub fn open(self, commit: Option<String>) {
        self.owner.with_value(|owner| {
            owner.with(|| {
                spawn_local(async move {
                    if let Err(error) =
                        blocking(move || updates::open_page(commit.as_deref())).await
                    {
                        self.message.set(error);
                    }
                })
            })
        });
    }
}
