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
    pub checking: RwSignal<bool, LocalStorage>,
    pub installing: RwSignal<bool, LocalStorage>,
    pub failed: RwSignal<bool, LocalStorage>,
    pub install_problem: StoredValue<Option<String>, LocalStorage>,
    download_cancel: StoredValue<Arc<AtomicBool>, LocalStorage>,
    task: StoredValue<Option<Task>, LocalStorage>,
    owner: StoredValue<Owner, LocalStorage>,
    retry: StoredValue<Option<zgui::view::IntervalHandle>, LocalStorage>,
}

impl UpdateState {
    #[cfg(test)]
    pub fn isolated() -> Self {
        Self::create(Preferences::default(), String::new(), None)
    }

    pub fn new() -> Self {
        let (preferences, message) = match Preferences::load() {
            Ok(p) => (
                p,
                "Enable nightly builds to check for updates when Enlumi starts.".into(),
            ),
            Err(e) => (Preferences::default(), e),
        };
        let previous_error = update_install::take_error();
        Self::create(preferences, message, previous_error)
    }

    fn create(preferences: Preferences, message: String, previous_error: Option<String>) -> Self {
        let failed = previous_error.is_some();
        let message = previous_error.unwrap_or(message);
        let state = Self {
            preferences: RwSignal::new_local(preferences),
            pending: RwSignal::new_local(None),
            message: RwSignal::new_local(message),
            ready: RwSignal::new_local(None),
            busy: RwSignal::new_local(false),
            checking: RwSignal::new_local(false),
            installing: RwSignal::new_local(false),
            failed: RwSignal::new_local(failed),
            install_problem: StoredValue::new_local(update_download::installation().err()),
            download_cancel: StoredValue::new_local(Arc::new(AtomicBool::new(false))),
            task: StoredValue::new_local(None),
            owner: StoredValue::new_local(Owner::current().expect("update checker owner")),
            retry: StoredValue::new_local(None),
        };
        on_cleanup_local(move || {
            state.cancel();
            state.retry.set_value(None);
            state
                .download_cancel
                .with_value(|flag| flag.store(true, Ordering::Relaxed));
        });
        if let Some(clock) = Timers::current() {
            state.retry.set_value(Some(clock.set_interval(
                std::time::Duration::from_secs(60),
                move || {
                    if state.preferences.with_untracked(|p| p.nightly)
                        && state.pending.get_untracked().is_some()
                        && state.ready.get_untracked().is_none()
                        && !state.failed.get_untracked()
                    {
                        // The pending revision is already known. Poll only releases so a
                        // long build does not spend GitHub's rate limit checking master.
                        state.start_check(state.pending.get_untracked());
                    }
                },
            )));
        }
        if !failed && state.preferences.with_untracked(|p| p.nightly) {
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
        self.checking.set(false);
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

    pub fn check(self) {
        self.start_check(None);
    }

    fn start_check(self, known_head: Option<String>) {
        if self.busy.get_untracked() || self.checking.get_untracked() {
            return;
        }
        self.checking.set(true);
        self.message
            .set("Checking master and completed nightlies…".into());
        let task = self.owner.with_value(|owner| {
            owner.with(|| {
                spawn_local(async move {
                    let (head, release) = blocking(move || {
                        let head = known_head.map_or_else(updates::latest, Ok);
                        (head, update_release::latest())
                    })
                    .await;
                    self.receive_check(head, release);
                })
            })
        });
        self.task.set_value(Some(task));
    }

    fn receive_check(self, head: Result<String, String>, release: Result<Option<Nightly>, String>) {
        self.checking.set(false);
        self.failed.set(false);
        match &head {
            Ok(sha) => self.receive_head(sha.clone()),
            Err(error) => self.message.set(error.clone()),
        }
        match release {
            Ok(Some(release)) => {
                self.message.set(format!(
                    "Nightly {} ready to install. Update & restart closes current session.",
                    &release.commit[..8]
                ));
                self.ready.set(Some(release));
            }
            Ok(None) => {
                self.ready.set(None);
                self.failed.set(head.is_err());
            }
            Err(error) => {
                self.failed.set(true);
                self.message
                    .set(format!("Could not find installable nightly: {error}"));
            }
        }
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
        if self.busy.get_untracked() || self.checking.get_untracked() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_notice::UpdateNoticeProps;
    use zgui_testkit_view::Window;

    fn state(window: &Window) -> UpdateState {
        window.scope.with(|| {
            UpdateState::create(
                Preferences {
                    nightly: false,
                    dismissed: Some("f".repeat(40)),
                },
                String::new(),
                None,
            )
        })
    }

    fn release() -> Nightly {
        Nightly {
            tag: "nightly-test".into(),
            commit: "a".repeat(40),
            asset: "enlumi-linux-x86_64.tar.gz".into(),
            digest: "b".repeat(64),
            size: 123,
        }
    }

    #[test]
    fn published_nightly_shows_install_button_after_pending_build() {
        let window = Window::open();
        let state = state(&window);
        state.install_problem.set_value(None);
        let mut built = window.scope.with(|| {
            view! { UpdateNotice(state=state) }
                .into_view()
                .build(&mut window.cx.cx())
        });
        built.mount(&window.dom_handle, window.root, None);

        state.receive_check(Ok("a".repeat(40)), Ok(None));
        window.frame();
        let text = window.dom.tree().text_content(window.root);
        assert!(text.contains("Check for updates"), "{text}");
        assert!(!text.contains("Update & restart"), "{text}");

        state.receive_check(Ok("a".repeat(40)), Ok(Some(release())));
        window.frame();
        let text = window.dom.tree().text_content(window.root);
        assert!(text.contains("Update & restart"), "{text}");
        assert!(!state.checking.get_untracked());
    }

    #[test]
    fn release_check_succeeds_even_when_master_check_fails() {
        let window = Window::open();
        let state = state(&window);
        state.receive_check(Err("Master unavailable".into()), Ok(Some(release())));
        assert!(state.ready.get_untracked().is_some());
        assert!(!state.failed.get_untracked());
        assert!(state.message.get_untracked().contains("ready to install"));
    }

    #[test]
    fn failed_release_check_is_visible_and_recovers_on_retry() {
        let window = Window::open();
        let state = state(&window);
        state.receive_check(Ok("a".repeat(40)), Err("GitHub unavailable".into()));
        assert!(state.failed.get_untracked());
        assert!(state.message.get_untracked().contains("GitHub unavailable"));
        state.receive_check(Ok("a".repeat(40)), Ok(Some(release())));
        assert!(!state.failed.get_untracked());
        assert!(state.ready.get_untracked().is_some());
    }

    #[test]
    fn pending_build_rechecks_after_one_minute_and_stops_when_ready_or_disabled() {
        use std::time::Duration;
        let window = Window::open();
        let state = state(&window);
        state.preferences.update(|p| p.nightly = true);
        state.pending.set(Some("a".repeat(40)));
        window.host.advance(Duration::from_secs(59));
        assert!(!state.checking.get_untracked());
        window.host.advance(Duration::from_secs(1));
        assert!(state.checking.get_untracked());
        // Cancel before polling the task: this test never contacts GitHub.
        state.cancel();

        state.ready.set(Some(release()));
        window.host.advance(Duration::from_secs(60));
        assert!(!state.checking.get_untracked());
        state.ready.set(None);
        state.preferences.update(|p| p.nightly = false);
        window.host.advance(Duration::from_secs(60));
        assert!(!state.checking.get_untracked());
    }
}
